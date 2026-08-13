//! `tts.band_and_dwell.hawkin`: force back inside a band around system weight, and staying.
//!
//! The interval runs from the landing a landing rule placed to the instant the dwell
//! completed, which is what the published rule reports. It therefore overstates the settling
//! by one whole dwell, and the registry carries that as a bias against the entry rather than
//! this rule reporting a second number no source publishes.
//!
//! Both parameters are exposed because no consensus exists to hide behind, and the number
//! turns on both: the band is defined relative to system weight, so the weighing rule reaches
//! this answer too, and no value shorter than the dwell can ever come back.
//!
//! A recording that ends before force settles is a fact about the recording. It comes back as
//! a refusal naming how long the longest quiet run managed, so a reader can see whether the
//! trace nearly answered or never came close.

use std::collections::BTreeMap;

use plateforce_core::stabilisation::{first_sustained_band_entry, StabilisationOutcome};
use plateforce_core::Refusal;

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::landing;

pub const ID: &str = "tts.band_and_dwell.hawkin";

/// Half-width of the band, as a percentage of system weight. The entry declares 5.
pub const BAND_PARAMETER: &str = "band_pct";
pub const BAND_DEFAULT_PCT: f64 = 5.0;

/// How long force has to hold inside the band. The entry declares one second.
pub const DWELL_PARAMETER: &str = "dwell_seconds";
pub const DWELL_DEFAULT_SECONDS: f64 = 1.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Time to stabilisation",
    unit: "seconds",
    computed_by: Some(ID),
    produced_by_construct: None,
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    _warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let band_pct = resolved.number(BAND_PARAMETER, BAND_DEFAULT_PCT);
    let dwell_samples = resolved.seconds_as_samples(
        DWELL_PARAMETER,
        DWELL_DEFAULT_SECONDS,
        context.trial.sample_rate_hz(),
    );

    let Some(landing_index) = landing::placed(context) else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(ID, &[landing::CONSTRUCT]),
        );
    };

    let system_weight_newtons = context.epoch().system_weight_newtons;
    let outcome = first_sustained_band_entry(
        context.trial.force(),
        landing_index,
        system_weight_newtons,
        band_pct,
        dwell_samples,
    );
    let bound = resolved.finish();

    let interval_seconds = context.trial.sample_interval_seconds();
    match outcome {
        StabilisationOutcome::Stabilised(found) => DerivedOutcome {
            values: vec![(
                super::KEY,
                Some(
                    (found.dwell_completed_index as f64 - landing_index as f64) * interval_seconds,
                ),
            )],
            placed: Vec::new(),
            bound,
            refusal: None,
        },
        // The three that are not a settling, each naming the evidence a reader would ask for
        // next. Seconds rather than samples, because every other number beside them is.
        StabilisationOutcome::TraceShorterThanDwell {
            available_samples,
            dwell_samples,
        } => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::nothing_qualified(
                ID,
                0,
                BTreeMap::from([
                    (
                        "seconds_recorded_after_landing".to_string(),
                        available_samples as f64 * interval_seconds,
                    ),
                    (
                        "dwell_seconds".to_string(),
                        dwell_samples as f64 * interval_seconds,
                    ),
                ]),
            ))),
        ),
        StabilisationOutcome::NeverSustained {
            longest_run_samples,
            dwell_samples,
        } => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::nothing_qualified(
                ID,
                1,
                BTreeMap::from([
                    (
                        "longest_quiet_run_seconds".to_string(),
                        longest_run_samples as f64 * interval_seconds,
                    ),
                    (
                        "dwell_seconds".to_string(),
                        dwell_samples as f64 * interval_seconds,
                    ),
                    ("system_weight_newtons".to_string(), system_weight_newtons),
                ]),
            ))),
        ),
        StabilisationOutcome::Unsearchable => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(Refusal::value_not_accepted(
                ID,
                BAND_PARAMETER,
                band_pct,
                vec![
                    "a band at or above zero percent of a positive system weight".to_string(),
                    "a dwell of at least one sample".to_string(),
                ],
            ))),
        ),
    }
}
