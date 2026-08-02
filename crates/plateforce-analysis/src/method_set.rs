//! The small file a researcher hands a colleague so the colleague reproduces the analysis.
//!
//! The document is the request rather than the result. A binding present in it was stated
//! by somebody; a slot the software resolved for itself is absent. Presence is statement,
//! so no per-binding source field is needed and none exists: a document that recorded
//! "assumed" beside a binding would be claiming somebody chose it.
//!
//! It carries the three things needed to know which software produced a number and which
//! registry it read: the schema it is written to, the `plateforce` version, and the
//! registry's own identity. Nothing here is a promise about what a rule does; that lives
//! in the registry the digest names.
//!
//! The type owns its shape and the caller owns its bytes. Every surface that writes one of
//! these already holds a JSON serialiser, so reading and writing stay where the file
//! handle is rather than pulling one into the crate the maths lives in.

use std::collections::BTreeMap;

use plateforce_core::Refusal;
use serde::{Deserialize, Serialize};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT};
use crate::request::{AnalysisRequest, MethodChoice, WeighingChoice};

/// Written into every document and checked on every read. It gets committed to strangers'
/// repositories, so it is a permanent string from the first release.
pub const METHOD_SET_SCHEMA: &str = "plateforce.method-set/1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodSetBinding {
    pub construct: String,
    pub method_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodSet {
    pub schema: String,
    pub plateforce_version: String,
    /// Taken over every file under the registry root. It identifies which registry a
    /// number came from whether or not that registry declares a version.
    pub registry_digest: String,
    /// The revision the registry declares for itself, when it declares one. A registry
    /// with no declared revision reads as unnamed rather than as an empty name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_version: Option<String>,
    /// Present when a preset produced this document. The bindings are still written out in
    /// full, so the document reads the same whether a preset produced it or a person did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    pub bindings: Vec<MethodSetBinding>,
}

/// What a document names, and what this build knows how to run.
///
/// The three constructs come from the binding layer rather than from a second table of
/// strings here. A construct with no slot is refused by name, so the day a fourth slot is
/// declared and not mapped, the failure says so instead of silently dropping a binding.
fn slot_for(construct: &str) -> Option<&'static str> {
    match construct {
        WEIGHING_CONSTRUCT => Some("weighing"),
        ONSET_CONSTRUCT => Some("onset"),
        TAKEOFF_CONSTRUCT => Some("takeoff"),
        _ => None,
    }
}

fn declared_constructs() -> Vec<String> {
    [WEIGHING_CONSTRUCT, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]
        .iter()
        .map(|construct| (*construct).to_string())
        .collect()
}

impl MethodSet {
    /// The document a run would need to reproduce itself.
    ///
    /// A slot bound to nothing is left out rather than written with an empty id, because an
    /// absent binding says the software resolved it and an empty one says nothing at all.
    pub fn of(
        request: &AnalysisRequest,
        plateforce_version: impl Into<String>,
        registry_digest: impl Into<String>,
        registry_version: Option<String>,
    ) -> Self {
        let mut bindings = Vec::new();
        if !request.weighing.method_id.is_empty() {
            bindings.push(MethodSetBinding {
                construct: WEIGHING_CONSTRUCT.to_string(),
                method_id: request.weighing.method_id.clone(),
                parameters: request.weighing.parameters.clone(),
                options: request.weighing.options.clone(),
            });
        }
        for (construct, choice) in [
            (ONSET_CONSTRUCT, &request.onset),
            (TAKEOFF_CONSTRUCT, &request.takeoff),
        ] {
            if choice.method_id.is_empty() {
                continue;
            }
            bindings.push(MethodSetBinding {
                construct: construct.to_string(),
                method_id: choice.method_id.clone(),
                parameters: choice.parameters.clone(),
                options: choice.options.clone(),
            });
        }
        Self {
            schema: METHOD_SET_SCHEMA.to_string(),
            plateforce_version: plateforce_version.into(),
            registry_digest: registry_digest.into(),
            registry_version,
            preset: None,
            bindings,
        }
    }

    /// The request this document asks for.
    ///
    /// A construct this build runs no slot for is refused naming the construct and what is
    /// declared, rather than dropped, because a binding silently ignored is a stated choice
    /// the run did not make.
    pub fn resolve(&self) -> Result<AnalysisRequest, Refusal> {
        let mut request = AnalysisRequest {
            weighing: WeighingChoice::default(),
            onset: MethodChoice::default(),
            takeoff: MethodChoice::default(),
            touchdown_index: None,
            gravity_meters_per_second_squared:
                plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            registry_backed_ids: Vec::new(),
        };

        for binding in &self.bindings {
            let Some(slot) = slot_for(&binding.construct) else {
                return Err(Refusal::construct_not_on_the_path(
                    binding.construct.clone(),
                    declared_constructs(),
                ));
            };
            match slot {
                "weighing" => {
                    request.weighing = WeighingChoice {
                        method_id: binding.method_id.clone(),
                        start_index: None,
                        parameters: binding.parameters.clone(),
                        options: binding.options.clone(),
                    }
                }
                "onset" => {
                    request.onset = MethodChoice {
                        method_id: binding.method_id.clone(),
                        parameters: binding.parameters.clone(),
                        options: binding.options.clone(),
                        manual_index: None,
                    }
                }
                _ => {
                    request.takeoff = MethodChoice {
                        method_id: binding.method_id.clone(),
                        parameters: binding.parameters.clone(),
                        options: binding.options.clone(),
                        manual_index: None,
                    }
                }
            }
        }
        Ok(request)
    }
}
