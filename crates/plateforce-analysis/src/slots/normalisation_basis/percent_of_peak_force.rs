//! `norm.pct_peak_force`: early force as a percentage of the peak.
//!
//! It separates the ability to express force rapidly from maximum force capacity, which an
//! absolute early-force value conflates, and that separation is the source paper's actual
//! contribution.
//!
//! It requires the net convention and is incompatible with the gross one, because a ratio
//! whose numerator and denominator both carry a bodyweight offset is not a fraction of
//! anything. So the peak it divides by is the one `net_peak_force` reported and the force it
//! divides is net at the stated instant. The entry names the time point and publishes no
//! value for it, so the caller states one and the rule declines by name until they do.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::net_peak_force;

pub const ID: &str = "norm.pct_peak_force";

/// The entry's own name for the instant, which it publishes no value for.
pub const TIME_PARAMETER: &str = "time_after_onset_seconds";

/// The one number the guards state to reach this rule. Not a default and not published.
pub const REQUIRED_NUMBERS: &[(&str, f64)] = &[(TIME_PARAMETER, 0.1)];

pub const FORCE_KEY: &str = "early_net_force_newtons";
pub const KEY: &str = "early_net_force_share_of_peak_percent";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: FORCE_KEY,
        label: "Force above standing weight at the stated instant",
        unit: "newtons",
        computed_by: Some(ID),
    },
    Quantity {
        key: KEY,
        label: "Early force as a share of the peak",
        unit: "percent",
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
    let seconds = resolved.required_number(ID, TIME_PARAMETER);
    let epoch = context.epoch();
    let system_weight_newtons = epoch.system_weight_newtons;
    let onset = context.onset_index();
    let peak = super::measured(
        context,
        ID,
        net_peak_force::CONSTRUCT,
        net_peak_force::KEY,
    );
    let bound = resolved.finish();

    let (seconds, (net_peak_newtons, produced_by)) = match (seconds, peak) {
        (Ok(seconds), Ok(peak)) => (seconds, peak),
        (Err(refusal), _) | (_, Err(refusal)) => return DerivedOutcome::declined(bound, refusal),
    };
    let Some(onset) = onset else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(ID, &[crate::binding::ONSET_CONSTRUCT]),
        );
    };
    let offset = (seconds * context.trial.sample_rate_hz()).round();
    let index = (offset >= 0.0)
        .then(|| onset.checked_add(offset as usize))
        .flatten()
        .filter(|index| *index < context.trial.len());
    let (Some(index), true) = (index, net_peak_newtons.abs() > 0.0) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::value_not_accepted(
                    ID,
                    TIME_PARAMETER,
                    seconds,
                    vec![format!(
                        "an instant the recording reaches after onset, at most {:.4} s here",
                        (context.trial.len() - 1 - onset) as f64 / context.trial.sample_rate_hz()
                    )],
                ),
            )),
        );
    };
    let early_net_newtons = context.trial.force()[index] - system_weight_newtons;
    super::rests_on(context, KEY, &produced_by);
    super::rests_on(context, FORCE_KEY, &produced_by);
    DerivedOutcome {
        values: vec![
            (FORCE_KEY, Some(early_net_newtons)),
            (KEY, Some(early_net_newtons / net_peak_newtons * 100.0)),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
