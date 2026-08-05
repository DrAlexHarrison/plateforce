//! `trial.gate.pretension_ceiling`: the athlete leaned into the plate before they were asked
//! to pull.
//!
//! The two criteria are not two strictnesses of one rule. A hundred newtons is 22 percent of
//! a 45 kg athlete's weight and 7 percent of a 150 kg athlete's, so a squad screened under
//! the absolute form is not the squad screened under the percentage one, and the entry files
//! that disagreement as genuine on the same principle the registry uses for absolute against
//! mass-relative onset thresholds.
//!
//! Both verdicts are reported and the stated criterion decides admission. The disagreement
//! between them is the entry's whole recorded finding, and on one trial it is a fact a reader
//! can see rather than an argument they have to take.

use plateforce_core::validity::PretensionCriterion;

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;

pub const ID: &str = "trial.gate.pretension_ceiling";

/// The entry's own names, and the values it publishes.
pub const CRITERION_PARAMETER: &str = "criterion";
pub const CRITERIA: &[(&str, PretensionCriterion)] = &[
    (
        "absolute_newtons_above_bodyweight",
        PretensionCriterion::AbsoluteNewtonsAboveBodyweight,
    ),
    (
        "percent_of_bodyweight",
        PretensionCriterion::PercentOfBodyweight,
    ),
];
pub const CEILING_PARAMETER: &str = "ceiling";
pub const CEILING_DEFAULT_NEWTONS: f64 = 100.0;
pub const BAND_PARAMETER: &str = "band_pct_bodyweight";
pub const BAND_DEFAULT_PERCENT: f64 = 10.0;

/// The one name this rule cannot run without.
pub const REQUIRED_OPTIONS: &[(&str, &str)] =
    &[(CRITERION_PARAMETER, "absolute_newtons_above_bodyweight")];

pub const EXCURSION_KEY: &str = "pretension_excursion_newtons";
pub const ABSOLUTE_VERDICT_KEY: &str = "trial_validity_pretension_admitted_at_the_absolute_ceiling";
pub const PERCENT_VERDICT_KEY: &str =
    "trial_validity_pretension_admitted_inside_the_percentage_band";
pub const KEY: &str = "trial_validity_pretension_admitted";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: EXCURSION_KEY,
        label: "How far force stood from standing weight before the effort",
        unit: "newtons",
        computed_by: Some(ID),
    },
    Quantity {
        key: ABSOLUTE_VERDICT_KEY,
        label: "Admitted against the newton ceiling",
        unit: "boolean",
        computed_by: Some(ID),
    },
    Quantity {
        key: PERCENT_VERDICT_KEY,
        label: "Admitted inside the percentage band",
        unit: "boolean",
        computed_by: Some(ID),
    },
    Quantity {
        key: KEY,
        label: "Admitted by the pretension gate",
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
    let mut resolved = Resolution::over(
        &choice.parameters,
        &choice.options,
        choice.declared.of_entry(ID),
        choice.claims(),
    );
    let criterion = resolved.required_enumerated(ID, CRITERION_PARAMETER, CRITERIA);
    let ceiling_newtons = resolved.number(CEILING_PARAMETER, CEILING_DEFAULT_NEWTONS);
    let band_percent = resolved.number(BAND_PARAMETER, BAND_DEFAULT_PERCENT);

    let epoch = context.epoch();
    let baseline_newtons = epoch.system_weight_newtons;
    let onset = context.onset_index();
    let bound = resolved.finish();

    let criterion = match criterion {
        Ok(chosen) => chosen,
        Err(refusal) => return DerivedOutcome::declined(bound, refusal),
    };
    // The stretch between the end of the weighing window and the start of the effort, which
    // is where a pre-load sits: inside the weighing window it moves standing weight itself,
    // and after onset it is the effort.
    let Some(onset) = onset else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[crate::binding::ONSET_CONSTRUCT]),
        );
    };
    let Some(before_effort) = context.trial.force().get(epoch.end_index..onset) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[crate::binding::ONSET_CONSTRUCT]),
        );
    };

    let judge = |criterion, against| {
        plateforce_core::validity::pretension_ceiling(
            before_effort,
            baseline_newtons,
            criterion,
            against,
        )
    };
    let (Some(against_ceiling), Some(inside_band), Some(departure_newtons)) = (
        judge(
            PretensionCriterion::AbsoluteNewtonsAboveBodyweight,
            ceiling_newtons,
        ),
        judge(PretensionCriterion::PercentOfBodyweight, band_percent),
        plateforce_core::validity::baseline_departure_newtons(before_effort, baseline_newtons),
    ) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::nothing_qualified(
                    ID,
                    before_effort.len(),
                    std::collections::BTreeMap::from([
                        ("weighing_end_sample".to_string(), epoch.end_index as f64),
                        ("onset_sample".to_string(), onset as f64),
                    ]),
                ),
            )),
        );
    };

    let deciding = match criterion {
        PretensionCriterion::AbsoluteNewtonsAboveBodyweight => against_ceiling,
        PretensionCriterion::PercentOfBodyweight => inside_band,
    };
    DerivedOutcome {
        values: vec![
            (EXCURSION_KEY, Some(departure_newtons)),
            (ABSOLUTE_VERDICT_KEY, super::admitted(against_ceiling.fired)),
            (PERCENT_VERDICT_KEY, super::admitted(inside_band.fired)),
            (KEY, super::admitted(deciding.fired)),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
