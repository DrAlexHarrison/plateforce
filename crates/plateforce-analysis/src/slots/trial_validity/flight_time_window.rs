//! `qc.flight_time_acceptance_window`: a flight phase too short or too long to be a jump.
//!
//! Silent data exclusion is the exact failure a provenance registry exists to prevent, and
//! this is the clearest instance of it in the registry: three shipped tools apply a window and
//! none of them documents the choice. One of the published windows is a jump-height window of
//! roughly 5 to 100 cm, which discards a genuine 4 cm hop and a 105 cm elite jump without
//! saying so. So every candidate is reported with its duration and its verdict, and the count
//! of rejected candidates travels with the count it was taken over.
//!
//! The direction of the resulting bias depends on the cohort and is therefore not correctable:
//! truncating the low end inflates a squad mean, truncating the high end deflates it, and both
//! bounds bite in a mixed cohort.
//!
//! A candidate is a stretch the plate read less force over than the caller's stated flight
//! threshold. The entry publishes no value for it and none of its three sources states one, so
//! the caller states it and the rule declines by name until they do.

use plateforce_core::takeoff::ResidualComparison;
use plateforce_core::validity::FlightSelection;

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "qc.flight_time_acceptance_window";

/// The entry's own names, and the values it publishes.
pub const LOWER_PARAMETER: &str = "lower_seconds";
pub const LOWER_DEFAULT_SECONDS: f64 = 0.1;
pub const UPPER_PARAMETER: &str = "upper_seconds";
pub const UPPER_DEFAULT_SECONDS: f64 = 2.0;
pub const SELECTION_PARAMETER: &str = "selection";
pub const SELECTIONS: &[(&str, FlightSelection)] = &[
    ("first_qualifying", FlightSelection::FirstQualifying),
    (
        "longest_qualifying_batch",
        FlightSelection::LongestQualifying,
    ),
];
pub const THRESHOLD_PARAMETER: &str = "flight_threshold_n";

/// The two names the guards state to reach this rule. Neither is a default and neither is
/// published: the entry states both required and publishes a value for neither.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[(SELECTION_PARAMETER, "first_qualifying")];
pub const REQUIRED_NUMBERS: &[(&str, f64)] = &[(THRESHOLD_PARAMETER, 10.0)];

pub const DURATION_KEY: &str = "accepted_flight_seconds";
pub const CANDIDATE_KEY: &str = "flight_candidates_read_count";
pub const REJECTED_KEY: &str = "flight_candidates_rejected_count";
pub const KEY: &str = "trial_validity_flight_window_admitted";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: DURATION_KEY,
        label: "Duration of the flight phase the window accepted",
        unit: "seconds",
        computed_by: Some(ID),
    },
    Quantity {
        key: CANDIDATE_KEY,
        label: "Flight phases the recording offered",
        unit: "count",
        computed_by: Some(ID),
    },
    Quantity {
        key: REJECTED_KEY,
        label: "Flight phases the window turned down",
        unit: "count",
        computed_by: Some(ID),
    },
    Quantity {
        key: KEY,
        label: "Admitted by the flight window",
        unit: "boolean",
        computed_by: Some(ID),
    },
];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let lower_seconds = resolved.number(LOWER_PARAMETER, LOWER_DEFAULT_SECONDS);
    let upper_seconds = resolved.number(UPPER_PARAMETER, UPPER_DEFAULT_SECONDS);
    let selection = resolved.required_enumerated(ID, SELECTION_PARAMETER, SELECTIONS);
    let threshold_newtons = resolved.required_number(ID, THRESHOLD_PARAMETER);
    // Asked for so the rules that placed the flight this analysis works from are in the chain
    // behind the verdict, whether or not the candidate the window accepted is that one.
    let placed_takeoff = context.takeoff_index();
    let bound = resolved.finish();

    let (selection, threshold_newtons) = match (selection, threshold_newtons) {
        (Ok(selection), Ok(threshold)) => (selection, threshold),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };
    if placed_takeoff.is_none() {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[crate::binding::TAKEOFF_CONSTRUCT]),
        );
    }

    let sample_rate_hz = context.trial.sample_rate_hz();
    let candidates: Vec<(usize, usize, f64)> = plateforce_core::takeoff::low_force_runs(
        context.trial.force(),
        threshold_newtons,
        ResidualComparison::SignedValue,
    )
    .into_iter()
    .map(|run| {
        (
            run.start_index,
            run.end_index,
            run.length() as f64 / sample_rate_hz,
        )
    })
    .collect();

    let Some(report) = plateforce_core::validity::flight_time_acceptance_window(
        &candidates,
        lower_seconds,
        upper_seconds,
        selection,
    ) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::nothing_qualified(
                    ID,
                    candidates.len(),
                    std::collections::BTreeMap::from([
                        ("flight_threshold_n".to_string(), threshold_newtons),
                        ("lower_seconds".to_string(), lower_seconds),
                        ("upper_seconds".to_string(), upper_seconds),
                    ]),
                ),
            )),
        );
    };
    let accepted = report
        .selected
        .map(|index| report.candidates[index].duration_seconds);
    DerivedOutcome {
        values: vec![
            (DURATION_KEY, accepted),
            (CANDIDATE_KEY, Some(report.population.considered() as f64)),
            (REJECTED_KEY, Some(report.population.rejected() as f64)),
            (KEY, super::admitted(report.selected.is_none())),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
