//! `rfd.exponential_model.padulles`: the rate read off a mono-exponential rise fitted from
//! onset.
//!
//! The only variant here whose value can exceed anything the athlete produced, because it is
//! read off a model rather than off the trace. The model's slope decays monotonically from the
//! instant it is pinned at, so its maximum rate is its rate at onset, which is the one reading
//! of the fit that needs no value the entry does not publish.
//!
//! Real force rise is sigmoidal, with a low-rate foot before the steep phase, so the model
//! overestimates early rate and underestimates it later. The mean absolute residual is exactly
//! what that costs, and the source that ships this rule prints it in red above 5 percent. It
//! goes to the reader beside the number, every time, with the time constant that produced it.

use crate::binding::ONSET_CONSTRUCT;
use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::{Resolution, RuleRefusal};
use crate::response::Quantity;
use crate::slots::analysis_window;

pub const ID: &str = "rfd.exponential_model.padulles";

/// Where the source that ships this rule stops trusting the fit.
pub const FLAGGED_RESIDUAL_PERCENT: f64 = 5.0;

pub const QUANTITIES: &[Quantity] = &[Quantity {
    key: super::KEY,
    label: "Rate of force development",
    unit: "newtons_per_second",
    computed_by: Some(ID),
}];

pub const RULE: DerivedRule = compute;

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> DerivedOutcome {
    let resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let bound = resolved.finish();

    let onset = context.onset_index();
    let window = analysis_window::span(context);
    let (Some(onset), Some((_, end))) = (onset, window) else {
        let mut missing = Vec::new();
        if onset.is_none() {
            missing.push(ONSET_CONSTRUCT);
        }
        if window.is_none() {
            missing.push(analysis_window::CONSTRUCT);
        }
        return DerivedOutcome::declined(bound, context.unavailable(ID, &missing));
    };

    match plateforce_core::exponential_rise::fit_exponential_rise(
        context.trial.force(),
        onset,
        end,
        context.trial.sample_interval_seconds(),
    ) {
        Ok(fitted) => {
            warnings.push(format!(
                "{ID} fitted a time constant of {:.4} s, leaving a mean absolute residual of \
                 {:.1} percent of the measured force{}",
                fitted.time_constant_seconds,
                fitted.mean_absolute_residual_percent,
                if fitted.mean_absolute_residual_percent > FLAGGED_RESIDUAL_PERCENT {
                    format!(
                        ", above the {FLAGGED_RESIDUAL_PERCENT} percent this rule's source flags"
                    )
                } else {
                    String::new()
                }
            ));
            DerivedOutcome {
                values: vec![(super::KEY, Some(fitted.maximum_rate_newtons_per_second()))],
                placed: Vec::new(),
                bound,
                refusal: None,
            }
        }
        Err(plateforce_core::exponential_rise::FitError::SpanTooShort { start, end }) => {
            DerivedOutcome::declined(
                bound,
                RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
                    ID, start, end,
                ))),
            )
        }
        // The stretch holds samples and no rise can be told from a flat line through them,
        // which is the recording answering rather than a span nobody could search.
        Err(plateforce_core::exponential_rise::FitError::NothingToFit) => DerivedOutcome::declined(
            bound,
            RuleRefusal::Refused(Box::new(plateforce_core::Refusal::nothing_qualified(
                ID,
                end.saturating_sub(onset),
                std::collections::BTreeMap::from([(
                    "search_bound_seconds".to_string(),
                    context.trial.time_at(end.saturating_sub(1)),
                )]),
            ))),
        ),
    }
}
