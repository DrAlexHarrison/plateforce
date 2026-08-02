//! Trial validity gates, which report and never remove.
//!
//! A gate that quietly drops a trial is the failure this registry exists to document, so
//! nothing here returns a shortened list. Each gate returns what it observed, what it
//! compared that against, and the count it was taken over. Whether a firing gate excludes
//! anything is a decision made above this crate, with the report in hand.
//!
//! Nothing here decides a method. A caller passes the thresholds a bound rule resolved.

use crate::statistics::{index_of_minimum, mean_and_standard_deviation, DispersionEstimator};

/// A count and the count it was taken over.
///
/// The two are one value because a numerator that travels without its denominator is the
/// defect this project publishes about, and a type that cannot express one without the
/// other is a stronger guarantee than a convention that says to include it. There is
/// deliberately no proportion on this type: a bare percentage is what hides a moving
/// denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counted {
    rejected: usize,
    considered: usize,
}

impl Counted {
    /// Nothing when more were rejected than were looked at, which is an arithmetic error
    /// upstream rather than a finding.
    pub fn of(rejected: usize, considered: usize) -> Option<Self> {
        (rejected <= considered).then_some(Self {
            rejected,
            considered,
        })
    }

    pub fn rejected(&self) -> usize {
        self.rejected
    }

    pub fn considered(&self) -> usize {
        self.considered
    }

    pub fn any_rejected(&self) -> bool {
        self.rejected > 0
    }
}

impl std::fmt::Display for Counted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} of {}", self.rejected, self.considered)
    }
}

/// What a gate saw, what it compared that against, and over how many.
///
/// Both sides of the comparison travel because a reader deciding whether to trust a
/// rejection needs to see how close it was, and a gate that fired by a hair and one that
/// fired by a mile are different facts about the trial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateFinding {
    pub fired: bool,
    pub observed: f64,
    pub criterion: f64,
    /// As the registry spells it, so no second vocabulary of units exists.
    pub unit: &'static str,
    pub population: Counted,
}

impl GateFinding {
    /// How far the observation sat from the criterion, in the criterion's own unit. The
    /// sign follows the observation, so a reader reads it beside the two values rather
    /// than having to remember which way the gate points.
    pub fn margin(&self) -> f64 {
        self.observed - self.criterion
    }
}

/// Which side of the baseline a pre-tension criterion is stated on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PretensionCriterion {
    /// Newtons above the athlete's weight. Stricter for a light athlete than a heavy one.
    AbsoluteNewtonsAboveBodyweight,
    /// A percentage of the athlete's weight, which is the same strictness at every mass.
    PercentOfBodyweight,
}

/// Force departing the baseline before the effort begins.
///
/// The two criteria are not two strictnesses of one rule. A fixed newton ceiling is a
/// larger fraction of a light athlete's weight than of a heavy one's, so a squad screened
/// under one is not the squad screened under the other.
pub fn pretension_ceiling(
    force_before_effort: &[f64],
    baseline_newtons: f64,
    criterion: PretensionCriterion,
    ceiling: f64,
) -> Option<GateFinding> {
    let departure = force_before_effort
        .iter()
        .map(|sample| (sample - baseline_newtons).abs())
        .fold(f64::NEG_INFINITY, f64::max);
    if !departure.is_finite() {
        return None;
    }
    let (observed, criterion_value, unit) = match criterion {
        PretensionCriterion::AbsoluteNewtonsAboveBodyweight => (departure, ceiling, "newtons"),
        PretensionCriterion::PercentOfBodyweight => (
            departure / baseline_newtons * 100.0,
            ceiling,
            "percent_of_bodyweight",
        ),
    };
    Some(GateFinding {
        fired: observed > criterion_value,
        observed,
        criterion: criterion_value,
        unit,
        population: Counted::of(usize::from(observed > criterion_value), 1)?,
    })
}

/// A countermovement inside a trial that was meant not to contain one.
///
/// The threshold is the baseline mean less `k` of its own standard deviations, so it
/// adapts to how still the athlete actually stood rather than to a fixed force.
pub fn countermovement_contamination(
    baseline_window: &[f64],
    between_baseline_and_onset: &[f64],
    standard_deviations: f64,
    dispersion: DispersionEstimator,
) -> Option<GateFinding> {
    let (mean, spread) = mean_and_standard_deviation(baseline_window, dispersion)?;
    let threshold = mean - standard_deviations * spread;
    let dip_index = index_of_minimum(between_baseline_and_onset)?;
    let dip = between_baseline_and_onset[dip_index];
    Some(GateFinding {
        fired: dip < threshold,
        observed: dip,
        criterion: threshold,
        unit: "newtons",
        population: Counted::of(usize::from(dip < threshold), 1)?,
    })
}

/// Which of several qualifying flight phases a rule takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightSelection {
    FirstQualifying,
    LongestQualifying,
}

/// A candidate flight phase and whether the window admitted it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlightCandidate {
    pub start_index: usize,
    pub end_index: usize,
    pub duration_seconds: f64,
    pub accepted: bool,
}

/// Every candidate flight phase judged against the window, and the one selected.
///
/// The rejected candidates are returned rather than dropped. A window that discards a
/// genuine 4 cm hop and a 105 cm elite jump does so silently in every implementation
/// examined, and the count with its denominator is the whole remedy.
#[derive(Debug, Clone)]
pub struct FlightWindowReport {
    pub candidates: Vec<FlightCandidate>,
    pub selected: Option<usize>,
    pub population: Counted,
}

pub fn flight_time_acceptance_window(
    durations_seconds: &[(usize, usize, f64)],
    lower_seconds: f64,
    upper_seconds: f64,
    selection: FlightSelection,
) -> Option<FlightWindowReport> {
    let candidates: Vec<FlightCandidate> = durations_seconds
        .iter()
        .map(
            |&(start_index, end_index, duration_seconds)| FlightCandidate {
                start_index,
                end_index,
                duration_seconds,
                accepted: (lower_seconds..=upper_seconds).contains(&duration_seconds),
            },
        )
        .collect();

    let qualifying: Vec<usize> = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.accepted)
        .map(|(index, _)| index)
        .collect();
    let selected = match selection {
        FlightSelection::FirstQualifying => qualifying.first().copied(),
        FlightSelection::LongestQualifying => qualifying.iter().copied().max_by(|left, right| {
            candidates[*left]
                .duration_seconds
                .total_cmp(&candidates[*right].duration_seconds)
        }),
    };

    let population = Counted::of(candidates.len() - qualifying.len(), candidates.len())?;
    Some(FlightWindowReport {
        candidates,
        selected,
        population,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpType {
    Countermovement,
    Squat,
}

/// A classification, the threshold it was decided against, and how far from it.
///
/// A type shown without its margin is a guess presented as a fact, so the threshold
/// travels with the answer rather than being recoverable only by rerunning the rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpTypeFinding {
    pub jump_type: JumpType,
    pub unweighting_newtons: f64,
    pub threshold_newtons: f64,
}

impl JumpTypeFinding {
    pub fn margin_newtons(&self) -> f64 {
        self.unweighting_newtons - self.threshold_newtons
    }
}

fn classify(unweighting_newtons: f64, threshold_newtons: f64) -> JumpTypeFinding {
    JumpTypeFinding {
        jump_type: if unweighting_newtons > threshold_newtons {
            JumpType::Countermovement
        } else {
            JumpType::Squat
        },
        unweighting_newtons,
        threshold_newtons,
    }
}

/// The fixed-threshold classifier, at whatever constant the caller was given.
pub fn jump_type_fixed_threshold(
    system_weight_newtons: f64,
    minimum_force_newtons: f64,
    threshold_newtons: f64,
) -> JumpTypeFinding {
    classify(
        system_weight_newtons - minimum_force_newtons,
        threshold_newtons,
    )
}

/// The same classifier with the threshold scaled by body mass against an anchor mass.
///
/// An exponent of zero returns the fixed threshold exactly and an exponent of one makes it
/// a constant fraction of bodyweight, so the two published endpoints are values of one
/// parameter rather than a third position invented between them.
pub fn jump_type_mass_scaled(
    system_weight_newtons: f64,
    minimum_force_newtons: f64,
    body_mass_kilograms: f64,
    anchor_mass_kilograms: f64,
    exponent: f64,
    threshold_at_anchor_newtons: f64,
) -> JumpTypeFinding {
    let scaled =
        threshold_at_anchor_newtons * (body_mass_kilograms / anchor_mass_kilograms).powf(exponent);
    classify(system_weight_newtons - minimum_force_newtons, scaled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_cannot_exceed_the_count_it_was_taken_over() {
        assert!(Counted::of(3, 244).is_some());
        assert!(Counted::of(245, 244).is_none());
        assert_eq!(Counted::of(3, 244).unwrap().to_string(), "3 of 244");
    }

    /// The absolute criterion is stricter on a light athlete and more permissive on a
    /// heavy one, which is the entry's own finding and the reason the two forms are not
    /// two strictnesses of one rule.
    #[test]
    fn the_two_pretension_criteria_disagree_across_athlete_mass() {
        let departure = 90.0f64;
        for (name, bodyweight, absolute_fires, relative_fires) in [
            ("a light athlete", 450.0f64, false, true),
            ("a heavy athlete", 1100.0f64, false, false),
        ] {
            let trace = vec![bodyweight + departure, bodyweight];
            let absolute = pretension_ceiling(
                &trace,
                bodyweight,
                PretensionCriterion::AbsoluteNewtonsAboveBodyweight,
                100.0,
            )
            .unwrap();
            let relative = pretension_ceiling(
                &trace,
                bodyweight,
                PretensionCriterion::PercentOfBodyweight,
                10.0,
            )
            .unwrap();
            assert_eq!(absolute.fired, absolute_fires, "{name}, absolute");
            assert_eq!(relative.fired, relative_fires, "{name}, relative");
        }
    }

    #[test]
    fn a_gate_reports_its_margin_on_both_sides_of_the_criterion() {
        let trace = vec![700.0, 820.0, 700.0];
        let finding = pretension_ceiling(
            &trace,
            700.0,
            PretensionCriterion::AbsoluteNewtonsAboveBodyweight,
            100.0,
        )
        .unwrap();
        assert!(finding.fired);
        assert!((finding.observed - 120.0).abs() < 1e-9);
        assert!((finding.criterion - 100.0).abs() < 1e-9);
        assert!((finding.margin() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn the_contamination_threshold_follows_how_still_the_athlete_stood() {
        let quiet: Vec<f64> = (0..600)
            .map(|index| 700.0 + ((index % 7) as f64 - 3.0) * 0.2)
            .collect();
        let restless: Vec<f64> = (0..600)
            .map(|index| 700.0 + ((index % 7) as f64 - 3.0) * 8.0)
            .collect();
        let dip = vec![700.0, 690.0, 700.0];

        let on_quiet =
            countermovement_contamination(&quiet, &dip, 5.0, DispersionEstimator::Sample).unwrap();
        let on_restless =
            countermovement_contamination(&restless, &dip, 5.0, DispersionEstimator::Sample)
                .unwrap();
        assert!(on_quiet.fired, "a 10 N dip off a still baseline is a dip");
        assert!(
            !on_restless.fired,
            "a 10 N dip off a restless baseline is inside the noise"
        );
    }

    /// The rejection count and the count it was taken over, which is the whole remedy for
    /// the silent discard this entry documents.
    #[test]
    fn the_flight_window_reports_the_rejected_count_with_its_denominator() {
        let candidates = [(100, 160, 0.05), (400, 900, 0.42), (1200, 1260, 0.05)];
        let report =
            flight_time_acceptance_window(&candidates, 0.1, 2.0, FlightSelection::FirstQualifying)
                .unwrap();
        assert_eq!(report.population.to_string(), "2 of 3");
        assert_eq!(report.selected, Some(1));
        assert!(report.candidates.iter().filter(|c| !c.accepted).count() == 2);
    }

    /// A rejected candidate stays in the report. A shortened list is the silent exclusion
    /// the entry exists to prevent.
    #[test]
    fn rejected_candidates_are_reported_rather_than_removed() {
        let candidates = [(100, 160, 0.05), (400, 900, 0.42)];
        let report =
            flight_time_acceptance_window(&candidates, 0.1, 2.0, FlightSelection::FirstQualifying)
                .unwrap();
        assert_eq!(report.candidates.len(), 2);
        assert!((report.candidates[0].duration_seconds - 0.05).abs() < 1e-12);
    }

    #[test]
    fn the_selection_rule_changes_which_flight_phase_is_taken() {
        let candidates = [(100, 400, 0.25), (800, 1400, 0.50)];
        let first =
            flight_time_acceptance_window(&candidates, 0.1, 2.0, FlightSelection::FirstQualifying)
                .unwrap();
        let longest = flight_time_acceptance_window(
            &candidates,
            0.1,
            2.0,
            FlightSelection::LongestQualifying,
        )
        .unwrap();
        assert_eq!(first.selected, Some(0));
        assert_eq!(longest.selected, Some(1));
    }

    /// The two published endpoints are values of the exponent, so the fixed rule is
    /// recovered exactly at zero rather than approximated.
    #[test]
    fn a_zero_exponent_recovers_the_fixed_threshold_exactly() {
        for body_mass in [52.0f64, 87.5, 118.0] {
            let fixed = jump_type_fixed_threshold(700.0, 400.0, 250.0);
            let scaled = jump_type_mass_scaled(700.0, 400.0, body_mass, 87.5, 0.0, 250.0);
            assert_eq!(scaled.threshold_newtons, fixed.threshold_newtons);
            assert_eq!(scaled.jump_type, fixed.jump_type);
        }
    }

    /// At the anchor mass the two agree whatever the exponent, which is what the anchor is
    /// for: the scaled rule reproduces the incumbent at the midpoint of the mass range its
    /// author states it works over.
    #[test]
    fn the_anchor_mass_is_where_the_two_rules_meet() {
        for exponent in [0.0f64, 0.667, 1.0] {
            let scaled = jump_type_mass_scaled(700.0, 400.0, 87.5, 87.5, exponent, 250.0);
            assert!(
                (scaled.threshold_newtons - 250.0).abs() < 1e-9,
                "{exponent}"
            );
        }
    }

    /// The exponent has to move the threshold away from the anchor or the entry is a
    /// second name for the incumbent, and the direction is the one the entry argues for:
    /// a lighter athlete is held to a lower threshold.
    #[test]
    fn the_exponent_moves_the_threshold_for_an_athlete_off_the_anchor() {
        let light = jump_type_mass_scaled(500.0, 300.0, 52.0, 87.5, 0.667, 250.0);
        let heavy = jump_type_mass_scaled(1150.0, 800.0, 118.0, 87.5, 0.667, 250.0);
        assert!(light.threshold_newtons < 250.0, "{light:?}");
        assert!(heavy.threshold_newtons > 250.0, "{heavy:?}");
    }

    #[test]
    fn a_classification_carries_the_margin_it_was_decided_by() {
        let finding = jump_type_fixed_threshold(700.0, 420.0, 250.0);
        assert_eq!(finding.jump_type, JumpType::Countermovement);
        assert!((finding.unweighting_newtons - 280.0).abs() < 1e-9);
        assert!((finding.margin_newtons() - 30.0).abs() < 1e-9);
    }
}
