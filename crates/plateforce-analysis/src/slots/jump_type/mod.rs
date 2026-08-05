//! Which jump the trial contains, decided from how far the athlete unloaded the plate.
//!
//! Misclassification is categorical rather than numeric: a squat jump has no unweighting and
//! no braking phase to find, so a countermovement pipeline pointed at one searches for
//! boundaries that are not there. The two rules here differ in one place, the threshold the
//! unweighting is held against, and they differ there on principle: a constant newton value
//! is 56.7 percent of a 45 kg athlete's weight and 17.0 percent of a 150 kg athlete's.
//!
//! Both report the same three quantities and the choice moves the threshold, which is what
//! `rules_answer = "one_question"` on the construct says about them.

pub mod fixed_threshold;
pub mod mass_scaled;

use plateforce_core::validity::{JumpType, JumpTypeFinding};

use crate::derived::{DerivedContext, DerivedOutcome};
use crate::resolution::{BoundValues, RuleRefusal};

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "jump_type";

/// The classification, as one and zero. The construct's unit is boolean and a metric carries
/// a number, so the two named types are the two values a boolean takes rather than a code a
/// reader has to look up.
pub const KEY: &str = "jump_type_is_countermovement";

/// What the classification was decided from, and what it was decided against. A type shown
/// without both is a guess presented as a fact, and the gap between them is the margin.
pub const UNWEIGHTING_KEY: &str = "jump_type_unweighting_newtons";
pub const THRESHOLD_KEY: &str = "jump_type_threshold_newtons";

/// The labels both rules report the three keys under. Spelled once here so the two rules
/// cannot describe one quantity two ways, and each rule declares its own array so
/// `computed_by` names the rule that ran.
pub const CLASSIFICATION_LABEL: &str = "Countermovement jump";
pub const UNWEIGHTING_LABEL: &str = "How far the plate was unloaded";
pub const THRESHOLD_LABEL: &str = "Unloading the classification needed";

/// How far below system weight the trace fell before the athlete left the plate, and the
/// system weight it fell from.
///
/// Searched to takeoff rather than over the whole recording, because flight reads near zero
/// and a search that ran through it would report the whole of system weight as unweighting on
/// every trial, countermovement or not. A recording whose takeoff rule placed nothing has no
/// end to search to, so this asks for one and declines by name without it.
pub(crate) fn unweighting_newtons(
    context: &DerivedContext,
    method_id: &'static str,
) -> Result<(f64, f64), RuleRefusal> {
    let Some(takeoff) = context.takeoff_index() else {
        return Err(context.unavailable(method_id, &[crate::binding::TAKEOFF_CONSTRUCT]));
    };
    let system_weight_newtons = context.epoch().system_weight_newtons;
    let before_takeoff = &context.trial.force()[..takeoff];
    let Some(minimum) = plateforce_core::statistics::index_of_minimum(before_takeoff) else {
        return Err(RuleRefusal::Refused(Box::new(
            plateforce_core::Refusal::nothing_qualified(
                method_id,
                before_takeoff.len(),
                std::collections::BTreeMap::from([("takeoff_sample".to_string(), takeoff as f64)]),
            ),
        )));
    };
    Ok((system_weight_newtons, before_takeoff[minimum]))
}

/// One classification as the three numbers the construct reports.
pub(crate) fn reported(finding: JumpTypeFinding, bound: BoundValues) -> DerivedOutcome {
    DerivedOutcome {
        values: vec![
            (
                KEY,
                Some(f64::from(u8::from(
                    finding.jump_type == JumpType::Countermovement,
                ))),
            ),
            (UNWEIGHTING_KEY, Some(finding.unweighting_newtons)),
            (THRESHOLD_KEY, Some(finding.threshold_newtons)),
        ],
        placed: Vec::new(),
        bound,
        refusal: None,
    }
}
