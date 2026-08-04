//! The trace itself, and the sentinel handling that must happen before anything reads it.

#[derive(Debug, Clone, thiserror::Error)]
pub enum TrialError {
    #[error("trace is empty")]
    Empty,
    #[error("sample rate must be positive, got {0}")]
    BadSampleRate(f64),
    #[error(
        "{method_id}({parameter} = {value}) found no crossing within the search bound of {search_bound_seconds} s"
    )]
    NoCrossing {
        method_id: String,
        parameter: String,
        value: f64,
        search_bound_seconds: f64,
    },
    #[error(
        "{method_id}({parameter} = {value}) has no band to search: dispersion is {dispersion_newtons} N and the threshold falls at {threshold_newtons} N"
    )]
    CollapsedBand {
        method_id: String,
        parameter: String,
        value: f64,
        dispersion_newtons: f64,
        threshold_newtons: f64,
    },
    #[error("weighing epoch of {requested_seconds} s starting at {start_seconds} s does not fit in a trace of {available_seconds} s")]
    EpochTooLong {
        requested_seconds: f64,
        start_seconds: f64,
        available_seconds: f64,
    },
}

/// The one mapping from `TrialError` onto the refusal every surface reads.
impl From<TrialError> for crate::Refusal {
    fn from(error: TrialError) -> Self {
        match error {
            TrialError::Empty => crate::Refusal::empty_trace(""),
            TrialError::BadSampleRate(value) => crate::Refusal::value_not_accepted(
                "",
                "sample_rate_hz",
                value,
                // A rate of -1200 is a finite number, so the code for a number that is not a
                // number would be reporting the wrong fault about it.
                vec!["a positive number of samples per second".to_string()],
            ),
            TrialError::NoCrossing {
                method_id,
                parameter,
                value,
                search_bound_seconds,
            } => crate::Refusal::no_crossing(method_id, parameter, value, search_bound_seconds),
            TrialError::CollapsedBand {
                method_id,
                parameter,
                value,
                dispersion_newtons,
                threshold_newtons,
            } => crate::Refusal::collapsed_band(
                method_id,
                parameter,
                value,
                dispersion_newtons,
                threshold_newtons,
            ),
            TrialError::EpochTooLong {
                requested_seconds,
                start_seconds,
                available_seconds,
            } => crate::Refusal::epoch_does_not_fit(
                "",
                requested_seconds,
                start_seconds,
                available_seconds,
            ),
        }
    }
}

/// A single trial. Force is vertical ground reaction force in newtons, already in the
/// sign convention where standing quietly reads positive and equal to system weight.
#[derive(Debug, Clone)]
pub struct Trial {
    vertical_ground_reaction_force_newtons: Vec<f64>,
    sample_rate_hz: f64,
}

impl Trial {
    pub fn new(
        vertical_ground_reaction_force_newtons: Vec<f64>,
        sample_rate_hz: f64,
    ) -> Result<Self, TrialError> {
        if vertical_ground_reaction_force_newtons.is_empty() {
            return Err(TrialError::Empty);
        }
        if !(sample_rate_hz.is_finite() && sample_rate_hz > 0.0) {
            return Err(TrialError::BadSampleRate(sample_rate_hz));
        }
        Ok(Self {
            vertical_ground_reaction_force_newtons,
            sample_rate_hz,
        })
    }

    pub fn force(&self) -> &[f64] {
        &self.vertical_ground_reaction_force_newtons
    }

    pub fn sample_rate_hz(&self) -> f64 {
        self.sample_rate_hz
    }

    pub fn sample_interval_seconds(&self) -> f64 {
        1.0 / self.sample_rate_hz
    }

    pub fn len(&self) -> usize {
        self.vertical_ground_reaction_force_newtons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertical_ground_reaction_force_newtons.is_empty()
    }

    pub fn duration_seconds(&self) -> f64 {
        self.len() as f64 * self.sample_interval_seconds()
    }

    pub fn time_at(&self, index: usize) -> f64 {
        index as f64 * self.sample_interval_seconds()
    }

    /// Trapezoidal integration over a half-open sample range, in newton seconds.
    ///
    /// Trapezoid rather than rectangle because the registry records the choice as a
    /// named variant with a measured difference, and this is the one it names.
    pub fn integrate_newton_seconds(&self, start: usize, end: usize) -> f64 {
        self.integrate_offset_newton_seconds(start, end, 0.0)
    }

    /// Trapezoidal integration of the force less a constant offset.
    ///
    /// Subtracting inside the integral rather than afterwards is the whole of it. A
    /// trapezoid over n samples spans n-1 intervals, so an offset removed afterwards
    /// over n intervals leaves one sample of weight behind, which is 8.2 mm/s of
    /// takeoff velocity at 1200 Hz and biases every jump height the same way.
    pub fn integrate_offset_newton_seconds(&self, start: usize, end: usize, offset: f64) -> f64 {
        let force = &self.vertical_ground_reaction_force_newtons;
        let end = end.min(force.len());
        if start >= end || end - start < 2 {
            return 0.0;
        }
        let dt = self.sample_interval_seconds();
        let mut accumulated = crate::statistics::CompensatedAccumulator::default();
        for index in start..end - 1 {
            accumulated.add((force[index] + force[index + 1]) * 0.5 - offset);
        }
        accumulated.total() * dt
    }
}

/// A value a vendor export writes to mean "no measurement". Reading one as a real
/// number is how three rows in 244 moved a published correlation by 0.16.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sentinel {
    Zero,
    NegativeOne,
    Value(f64),
}

impl Sentinel {
    pub fn matches(&self, candidate: f64) -> bool {
        match self {
            Sentinel::Zero => candidate == 0.0,
            Sentinel::NegativeOne => candidate == -1.0,
            Sentinel::Value(v) => candidate == *v,
        }
    }
}

/// Split values into real measurements and sentinels, reporting which convention was
/// applied. Callers must declare a convention rather than inheriting a default,
/// because a silent choice here is indistinguishable from a measurement.
pub fn partition_sentinels(values: &[f64], sentinel: Sentinel) -> (Vec<f64>, Vec<usize>) {
    let mut kept = Vec::with_capacity(values.len());
    let mut dropped = Vec::new();
    for (index, &value) in values.iter().enumerate() {
        if sentinel.matches(value) || !value.is_finite() {
            dropped.push(index);
        } else {
            kept.push(value);
        }
    }
    (kept, dropped)
}

/// Samples matching the declared convention against samples carrying no number at all.
///
/// `partition_sentinels` reports one total over both, and one number cannot say which. The
/// distinction is not academic on a force plate: the zero convention a vendor writes for a
/// measurement it does not have is also the correct reading of a plate with nothing on it,
/// so on a jump trace it matches the whole flight phase. A reader told only the total cannot
/// tell a gap in the recording from the athlete being in the air.
///
/// Measured on `subject01_trial1_interrupted` under the zero convention, the total is 160:
/// 157 samples of an athlete in the air and 3 samples the recording lost, in one number
/// nobody can take apart. Under no convention at all the same total is 0, and the 3 are
/// still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReportedSamples {
    /// Samples reading the value the declared convention writes for a measurement that was
    /// not taken. Zero when the caller declared no convention, because a reader who declared
    /// nothing matched nothing.
    pub matched_the_convention: usize,
    /// Samples carrying no number, which is a gap in the recording rather than a convention
    /// a reader could have declared differently. Counted whether or not a convention was
    /// declared, because the recording is the same either way.
    pub carried_no_number: usize,
}

impl ReportedSamples {
    /// The two counts together, so a caller that wants the total does not add them up itself
    /// and the split can be checked against it.
    pub fn total(&self) -> usize {
        self.matched_the_convention + self.carried_no_number
    }
}

/// Count the two reasons a sample is reported, apart.
///
/// A value that is not finite is counted as carrying no number and nowhere else, so the two
/// counts are disjoint and add up to the length of what `partition_sentinels` drops under the
/// same convention. `the_two_counts_add_up_to_what_the_partition_drops` holds that.
pub fn reported_samples(values: &[f64], sentinel: Option<Sentinel>) -> ReportedSamples {
    let mut reported = ReportedSamples::default();
    for &value in values {
        if !value.is_finite() {
            reported.carried_no_number += 1;
        } else if sentinel.is_some_and(|convention| convention.matches(value)) {
            reported.matched_the_convention += 1;
        }
    }
    reported
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant reaches a refusal carrying the same fields, and four of the five carry
    /// the same sentence.
    #[test]
    fn every_trial_error_becomes_a_refusal_that_says_the_same_thing() {
        use crate::{Refusal, RefusalCode};

        let refused: Refusal = TrialError::NoCrossing {
            method_id: "onset.threshold.noise_relative".to_string(),
            parameter: "k".to_string(),
            value: 5.0,
            search_bound_seconds: 2.5,
        }
        .into();
        assert_eq!(refused.code, RefusalCode::NoCrossing);
        assert_eq!(
            refused.message(),
            "onset.threshold.noise_relative(k = 5) found no crossing within the search bound of 2.5 s"
        );
        assert_eq!(refused.detail["search_bound_seconds"], 2.5);

        let collapsed: Refusal = TrialError::CollapsedBand {
            method_id: "onset.threshold.noise_relative".to_string(),
            parameter: "k".to_string(),
            value: 5.0,
            dispersion_newtons: 0.4,
            threshold_newtons: 812.1,
        }
        .into();
        assert_eq!(collapsed.code, RefusalCode::CollapsedBand);
        assert_eq!(
            collapsed.message(),
            TrialError::CollapsedBand {
                method_id: "onset.threshold.noise_relative".to_string(),
                parameter: "k".to_string(),
                value: 5.0,
                dispersion_newtons: 0.4,
                threshold_newtons: 812.1,
            }
            .to_string()
        );

        let empty: Refusal = TrialError::Empty.into();
        assert_eq!(empty.code, RefusalCode::TraceTooShort);
        assert_eq!(empty.message(), TrialError::Empty.to_string());

        let epoch: Refusal = TrialError::EpochTooLong {
            requested_seconds: 2.0,
            start_seconds: 1.5,
            available_seconds: 3.0,
        }
        .into();
        assert_eq!(epoch.code, RefusalCode::TraceTooShort);
        assert_eq!(epoch.detail["available_seconds"], 3.0);

        // The refusal names the parameter as a caller spells it, which is the name they would
        // set. A rate of 0 is a finite number, so the code says the value is one this
        // parameter does not take rather than that it is not a number.
        let rate: Refusal = TrialError::BadSampleRate(0.0).into();
        assert_eq!(rate.code, RefusalCode::ValueNotAccepted);
        assert_eq!(rate.parameter.as_deref(), Some("sample_rate_hz"));
        assert_eq!(
            rate.message(),
            "sample_rate_hz does not accept 0: it takes a positive number of samples per second"
        );
    }

    #[test]
    fn integrating_a_constant_force_gives_force_times_duration() {
        let trial = Trial::new(vec![100.0; 1201], 1200.0).unwrap();
        let impulse = trial.integrate_newton_seconds(0, 1201);
        assert!((impulse - 100.0).abs() < 1e-9, "got {impulse}");
    }

    #[test]
    fn a_zero_length_range_integrates_to_zero() {
        let trial = Trial::new(vec![100.0; 10], 1000.0).unwrap();
        assert_eq!(trial.integrate_newton_seconds(5, 5), 0.0);
    }

    #[test]
    fn sentinels_are_separated_from_measurements_and_counted() {
        let (kept, dropped) = partition_sentinels(&[45.0, 0.0, 51.0, 0.0], Sentinel::Zero);
        assert_eq!(kept, vec![45.0, 51.0]);
        assert_eq!(dropped, vec![1, 3]);
    }

    /// A sentinel convention matches one value, not a neighbourhood. Reactive strength
    /// index runs from about 0.2 to 1.5 and flight times from about 0.4 s, so a
    /// convention that swallowed anything small would drop real measurements as
    /// missing data, which is the same defect as reading a sentinel as a measurement.
    #[test]
    fn a_small_real_measurement_is_not_mistaken_for_the_sentinel() {
        let (kept, dropped) = partition_sentinels(&[0.51, 0.0, 0.74, 0.001], Sentinel::Zero);
        assert_eq!(kept, vec![0.51, 0.74, 0.001]);
        assert_eq!(dropped, vec![1]);
    }

    #[test]
    fn a_non_finite_value_is_never_read_as_a_measurement() {
        let (kept, dropped) =
            partition_sentinels(&[45.0, f64::NAN, f64::INFINITY], Sentinel::NegativeOne);
        assert_eq!(kept, vec![45.0]);
        assert_eq!(dropped, vec![1, 2]);
    }

    #[test]
    fn an_empty_trace_is_rejected_rather_than_returning_a_number() {
        assert!(Trial::new(Vec::new(), 1200.0).is_err());
    }

    /// The two reasons are counted apart, and the trace that carries both is the one that
    /// shows why the total cannot stand in for either.
    #[test]
    fn the_two_reasons_a_sample_is_reported_are_counted_apart() {
        let values = [45.0, 0.0, f64::NAN, 0.0, 51.0];

        let declared = reported_samples(&values, Some(Sentinel::Zero));
        assert_eq!(declared.matched_the_convention, 2);
        assert_eq!(declared.carried_no_number, 1);

        // The same recording with nothing declared. The convention count falls to zero
        // because the reader declared nothing; the gap is still there, because the gap
        // belongs to the recording rather than to the reader.
        let undeclared = reported_samples(&values, None);
        assert_eq!(undeclared.matched_the_convention, 0);
        assert_eq!(undeclared.carried_no_number, 1);
    }

    /// The disjointness the doc comment claims, held rather than asserted in prose.
    ///
    /// A count that double-counted a non-finite sample matching the convention, or missed
    /// one, would still look like a plausible pair of numbers beside a trace. Held against
    /// `partition_sentinels`, so the split cannot silently stop adding up to its total.
    #[test]
    fn the_two_counts_add_up_to_what_the_partition_drops() {
        for convention in [
            Sentinel::Zero,
            Sentinel::NegativeOne,
            Sentinel::Value(9999.0),
        ] {
            let values = [
                45.0,
                0.0,
                -1.0,
                9999.0,
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
                51.0,
            ];
            let dropped = partition_sentinels(&values, convention).1.len();
            assert_eq!(reported_samples(&values, Some(convention)).total(), dropped);
        }
    }

    /// A trace with nothing to report reads zero for both, so a caller cannot read one of
    /// these counts as evidence that the counter ran.
    #[test]
    fn a_clean_trace_reports_neither_reason() {
        let reported = reported_samples(&[45.0, 51.0, 47.0], Some(Sentinel::Zero));
        assert_eq!(reported.matched_the_convention, 0);
        assert_eq!(reported.carried_no_number, 0);
        assert_eq!(reported.total(), 0);
    }
}
