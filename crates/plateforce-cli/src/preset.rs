//! The published pipeline a command line named, laid onto the request it built.
//!
//! One home for both commands that take trials, so a folder run and a single trial adopt a
//! pipeline the same way and refuse an unknown name with the same record.

use std::collections::BTreeMap;

use plateforce_analysis::request::preset_named;
use plateforce_analysis::{
    AnalysisRequest, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
use plateforce_registry::Registry;

use crate::decisions::slot_of;
use crate::exit::Declined;

/// Lays the named pipeline onto the request, or declines with the record.
///
/// Nothing happens when no pipeline was named, so a caller passes the option straight
/// through rather than branching around this.
pub(crate) fn adopt(
    request: &mut AnalysisRequest,
    registry: &Registry,
    named: Option<&String>,
) -> Result<(), Declined> {
    let Some(id) = named else { return Ok(()) };
    let preset = preset_named(registry, id).map_err(|refusal| Declined::recorded(*refusal))?;
    request
        .adopt(preset)
        .map_err(|refusal| Declined::recorded(*refusal))
}

/// The rule bound to each landmark, read back off the request.
///
/// Read back rather than tracked alongside, because a pipeline answers choices the command
/// line did not, and a decision rail run against the command line alone would report a
/// choice as open that the pipeline's source has already published.
pub(crate) fn methods_in(request: &AnalysisRequest) -> BTreeMap<String, String> {
    let mut chosen = BTreeMap::new();
    for (construct, method_id) in [
        (WEIGHING_CONSTRUCT, &request.weighing.method_id),
        (ONSET_CONSTRUCT, &request.onset.method_id),
        (TAKEOFF_CONSTRUCT, &request.takeoff.method_id),
    ] {
        if !method_id.is_empty() {
            chosen.insert(construct.to_string(), method_id.clone());
        }
    }
    chosen
}

/// Every value bound to a landmark, keyed by the slot word the command line writes.
pub(crate) fn parameters_in(request: &AnalysisRequest) -> BTreeMap<String, BTreeMap<String, f64>> {
    let mut stated = BTreeMap::new();
    for (construct, parameters) in [
        (WEIGHING_CONSTRUCT, &request.weighing.parameters),
        (ONSET_CONSTRUCT, &request.onset.parameters),
        (TAKEOFF_CONSTRUCT, &request.takeoff.parameters),
    ] {
        if !parameters.is_empty() {
            stated.insert(slot_of(construct).to_string(), parameters.clone());
        }
    }
    for (construct, choice) in &request.derived {
        if !choice.parameters.is_empty() {
            stated.insert(construct.clone(), choice.parameters.clone());
        }
    }
    stated
}
