//! `qc.countermovement_contamination.chavda2020`: an isometric trial that dipped before it
//! pulled.
//!
//! The threshold is the weighing window's own mean less `k` of its own standard deviations,
//! so it follows how still the athlete actually stood rather than a fixed force. That is also
//! why the gate is tighter on a hand-placed flattest window than on a fixed one, by the same
//! mechanism that makes manual window placement lower a noise-relative onset threshold.
//!
//! The entry keeps its protocol boundary. It judges an isometric trial, and on a
//! countermovement jump the dip it looks for is the movement rather than a contamination of
//! it, so the number it reports is real on any recording and the verdict means something only
//! on the recording the operator has still to make.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "qc.countermovement_contamination.chavda2020";

/// The entry's own name for the multiplier, and the value it publishes.
pub const DEVIATIONS_PARAMETER: &str = "k";
pub const DEVIATIONS_DEFAULT: f64 = 5.0;

pub const DIP_KEY: &str = "countermovement_dip_newtons";
pub const THRESHOLD_KEY: &str = "countermovement_threshold_newtons";
pub const KEY: &str = "trial_admitted_by_the_countermovement_gate";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: DIP_KEY,
        label: "Lowest force between standing and the start of the effort",
        unit: "newtons",
        computed_by: Some(ID),
    },
    Quantity {
        key: THRESHOLD_KEY,
        label: "Force the dip had to stay above",
        unit: "newtons",
        computed_by: Some(ID),
    },
    Quantity {
        key: KEY,
        label: "Admitted by the countermovement gate",
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
    let standard_deviations = resolved.number(DEVIATIONS_PARAMETER, DEVIATIONS_DEFAULT);
    let dispersion = resolved.dispersion();

    let epoch = context.epoch();
    let onset = context.onset_index();
    let bound = resolved.finish();

    let dispersion = match dispersion {
        Ok(estimator) => estimator,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };
    let Some(onset) = onset else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[crate::binding::ONSET_CONSTRUCT]),
        );
    };
    let force = context.trial.force();
    let (Some(baseline), Some(between)) = (
        force.get(epoch.start_index..epoch.end_index),
        force.get(epoch.end_index..onset),
    ) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[crate::binding::ONSET_CONSTRUCT]),
        );
    };

    let Some(finding) = plateforce_core::validity::countermovement_contamination(
        baseline,
        between,
        standard_deviations,
        dispersion,
    ) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::nothing_qualified(
                    ID,
                    between.len(),
                    std::collections::BTreeMap::from([
                        ("weighing_samples".to_string(), baseline.len() as f64),
                        ("onset_sample".to_string(), onset as f64),
                    ]),
                ),
            )),
        );
    };
    DerivedOutcome {
        values: vec![
            (DIP_KEY, Some(finding.observed)),
            (THRESHOLD_KEY, Some(finding.criterion)),
            (KEY, super::admitted(finding.fired)),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
