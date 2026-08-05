//! `qc.transient_peak_count.pedley2023`: braking that arrives as a spike train rather than as
//! a curve.
//!
//! The only automated data-quality rule found anywhere in the sweep that inspects the shape of
//! the trace rather than a single value, and shape is where impact artefacts and plate ringing
//! show up. It counts the samples force rose into and did not rise out of, over the braking
//! period the phase rules bounded.
//!
//! Nothing is smoothed before the count. The rule's own two remedies are to discard the trial
//! or to lower the filter cutoff and re-run, and counting on a pre-smoothed signal would apply
//! the second remedy before the reader chose between them. The greatest force in the period is
//! reported beside the count because the rule falls back to it when it finds no peak at all.

use crate::derived::{DerivedContext, DerivedOutcome, DerivedRule};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::response::Quantity;
use crate::slots::{braking_phase_start, propulsion_phase_start};

pub const ID: &str = "qc.transient_peak_count.pedley2023";

/// The entry's own name for the ceiling, and the value it publishes.
pub const MAX_PEAKS_PARAMETER: &str = "max_peaks";
pub const MAX_PEAKS_DEFAULT: f64 = 3.0;

pub const COUNT_KEY: &str = "braking_transient_peak_count";
pub const CEILING_KEY: &str = "braking_transient_peak_ceiling_count";
pub const GREATEST_KEY: &str = "braking_greatest_force_newtons";
pub const KEY: &str = "trial_validity_transient_peaks_admitted";

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: COUNT_KEY,
        label: "Force peaks during braking",
        unit: "count",
        computed_by: Some(ID),
    },
    Quantity {
        key: CEILING_KEY,
        label: "Peaks the trial was allowed",
        unit: "count",
        computed_by: Some(ID),
    },
    Quantity {
        key: GREATEST_KEY,
        label: "Greatest force during braking",
        unit: "newtons",
        computed_by: Some(ID),
    },
    Quantity {
        key: KEY,
        label: "Admitted by the transient peak gate",
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
    let max_peaks = resolved.number(MAX_PEAKS_PARAMETER, MAX_PEAKS_DEFAULT);

    let start = braking_phase_start::placed(context);
    let end = propulsion_phase_start::placed(context);
    let bound = resolved.finish();

    let (Some(start), Some(end)) = (start, end) else {
        return DerivedOutcome::declined(
            bound,
            context.unavailable(
                ID,
                &[
                    braking_phase_start::CONSTRUCT,
                    propulsion_phase_start::CONSTRUCT,
                ],
            ),
        );
    };
    // A negative ceiling would fire on a period with no peak at all, which is the rule
    // rejecting every trial rather than the caller choosing a strict one.
    if !max_peaks.is_finite() || max_peaks < 0.0 {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::value_not_accepted(
                    ID,
                    MAX_PEAKS_PARAMETER,
                    max_peaks,
                    vec!["a count of zero or more".to_string()],
                ),
            )),
        );
    }
    let Some(report) = plateforce_core::validity::transient_peak_count(
        context.trial.force(),
        start,
        end,
        max_peaks.floor() as usize,
    ) else {
        return DerivedOutcome::declined(
            bound,
            crate::resolution::RuleRefusal::Refused(Box::new(
                plateforce_core::Refusal::nothing_qualified(
                    ID,
                    end.saturating_sub(start),
                    std::collections::BTreeMap::from([
                        ("braking_start_sample".to_string(), start as f64),
                        ("braking_end_sample".to_string(), end as f64),
                    ]),
                ),
            )),
        );
    };
    DerivedOutcome {
        values: vec![
            (COUNT_KEY, Some(report.finding.observed)),
            (CEILING_KEY, Some(report.finding.criterion)),
            (GREATEST_KEY, Some(report.greatest_force_newtons)),
            (KEY, super::admitted(report.finding.fired)),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
