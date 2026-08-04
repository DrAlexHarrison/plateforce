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
use plateforce_registry::Preset;

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
    /// The revision the caller pinned, when they pinned one. Absent on a run whose caller
    /// pinned nothing, which is a fact about the request rather than about the registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_version: Option<String>,
    /// The revision the registry declares for itself, when it declares one. A registry
    /// with no declared revision reads as unnamed rather than as an empty name.
    ///
    /// Separate from the pin above. This document is what a colleague reads to reproduce a
    /// run, and one field standing for both would tell them the author cited a revision the
    /// data named itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_declared_version: Option<String>,
    /// Present when a preset produced this document. The bindings are still written out in
    /// full, so the document reads the same whether a preset produced it or a person did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    pub bindings: Vec<MethodSetBinding>,
}

/// The constructs this build runs a step for, named from the binding layer rather than
/// from a second table of strings here.
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
        registry: &plateforce_core::provenance::RegistryStamp,
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
            ..Self::stamped(registry, None, bindings)
        }
    }

    /// The three registry fields and the bindings, filled once for both constructors so the
    /// document says the same thing about its registry whichever one wrote it.
    ///
    /// An unread registry writes an empty digest, which is what this document has always
    /// meant by one: the field is a name rather than an option, and there is nothing to name.
    fn stamped(
        registry: &plateforce_core::provenance::RegistryStamp,
        preset: Option<String>,
        bindings: Vec<MethodSetBinding>,
    ) -> Self {
        let plateforce_core::provenance::RegistryStamp {
            version,
            declared_version,
            digest,
        } = registry.clone();
        Self {
            schema: METHOD_SET_SCHEMA.to_string(),
            plateforce_version: String::new(),
            registry_digest: digest.unwrap_or_default(),
            registry_version: version,
            registry_declared_version: declared_version,
            preset,
            bindings,
        }
    }

    /// The document a named published pipeline resolves to.
    ///
    /// Every binding the source states is written out in full, so the file reads the same
    /// whether a preset produced it or somebody typed it, and a reviewer sees the pipeline
    /// rather than a name they have to look up.
    ///
    /// A slot the source is silent about is absent, and is left to the software's normal
    /// resolution. That is a fact about the source rather than about this build, so it is
    /// not a refusal and the preset is never credited with the choice.
    ///
    /// A bound method the registry carries and no rule here runs refuses, naming the
    /// method and the step, rather than the registry declining to load.
    ///
    /// The preset is taken already looked up. The shared refusal has no sentence for an id
    /// that names no preset, and the surface that reads the flag is the one holding the
    /// list of ids it could suggest instead.
    pub fn from_preset(
        preset: &Preset,
        plateforce_version: impl Into<String>,
        registry: &plateforce_core::provenance::RegistryStamp,
    ) -> Result<Self, Box<Refusal>> {
        let mut bindings = Vec::new();
        for binding in &preset.bindings {
            if !crate::binding::bindings_for_construct(&binding.construct)
                .any(|runnable| runnable.id == binding.method_id)
            {
                return Err(Box::new(Refusal::method_not_implemented(
                    binding.method_id.clone(),
                    binding.construct.clone(),
                    crate::binding::bindings_for_construct(&binding.construct)
                        .map(|runnable| runnable.id.to_string())
                        .collect(),
                )));
            }
            bindings.push(MethodSetBinding {
                construct: binding.construct.clone(),
                method_id: binding.method_id.clone(),
                parameters: binding.parameters.clone(),
                options: binding.options.clone(),
            });
        }

        Ok(Self {
            plateforce_version: plateforce_version.into(),
            ..Self::stamped(registry, Some(preset.id.clone()), bindings)
        })
    }

    /// The schema this document declares, checked before anything else is read from it.
    ///
    /// A file written by a later `plateforce` is refused as a version rather than as a
    /// corrupt file, because the two have different answers: there is nothing else the
    /// reader could have asked for, and the remedy is a newer build.
    pub fn readable(&self) -> Result<(), Box<Refusal>> {
        if self.schema == METHOD_SET_SCHEMA {
            return Ok(());
        }
        Err(Box::new(Refusal::schema_unsupported(
            self.schema.clone(),
            METHOD_SET_SCHEMA,
        )))
    }

    /// The request this document asks for.
    ///
    /// A construct this build runs no slot for is refused naming the construct and what is
    /// declared, rather than dropped, because a binding silently ignored is a stated choice
    /// the run did not make.
    pub fn resolve(&self) -> Result<AnalysisRequest, Box<Refusal>> {
        let mut request = AnalysisRequest::default();

        self.readable()?;
        // A construct with no step is refused by name rather than dropped, so the day a
        // fourth construct is declared and not mapped here, the failure says so instead of
        // silently discarding a binding somebody stated.
        for binding in &self.bindings {
            let choice = MethodChoice {
                method_id: binding.method_id.clone(),
                parameters: binding.parameters.clone(),
                options: binding.options.clone(),
                manual_index: None,
                ..Default::default()
            };
            match binding.construct.as_str() {
                WEIGHING_CONSTRUCT => {
                    request.weighing = WeighingChoice {
                        method_id: binding.method_id.clone(),
                        start_index: None,
                        parameters: binding.parameters.clone(),
                        options: binding.options.clone(),
                        ..Default::default()
                    }
                }
                ONSET_CONSTRUCT => request.onset = choice,
                TAKEOFF_CONSTRUCT => request.takeoff = choice,
                _ => {
                    return Err(Box::new(Refusal::construct_not_on_the_path(
                        binding.construct.clone(),
                        declared_constructs(),
                    )))
                }
            }
        }
        Ok(request)
    }
}
