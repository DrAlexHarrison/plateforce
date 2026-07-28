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

#[cfg(test)]
mod tests {
    use super::*;

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
}
