//! A published pipeline, named, so `--preset owen2014` is the pipeline that paper ran.
//!
//! A preset is a set of bindings and a citation. It has no rule field and no surfacing
//! field, for the same reason a protocol has neither: it describes which rules to use, not
//! how any of them works.
//!
//! A preset binds only the slots its source states. A slot the source is silent about is
//! left to the software's normal resolution and is never attributed to the preset. A
//! preset that filled in the rest would manufacture provenance: a settings screen naming a
//! method and a citation over a code path computing something else.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::schema::Citation;
use crate::validate::{Violation, ViolationKind};
use crate::Registry;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default, rename = "binding")]
    pub bindings: Vec<PresetBinding>,
    #[serde(default, rename = "citation")]
    pub citations: Vec<Citation>,
    /// What the source is silent about, as a fact about the source rather than about this
    /// software.
    #[serde(default)]
    pub states_nothing_about: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresetBinding {
    pub construct: String,
    pub method_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// The entry this rule composes, when the id names a composition rather than an entry.
    ///
    /// A composition is an entry with an operator bound onto it and carries that entry's
    /// citations, so it has no row of its own to be checked against. Stating the entry here
    /// is what lets a preset name one and still be checked: without it, an id that composes
    /// and an id that is a typo are the same thing to a validator.
    #[serde(default)]
    pub composed_from: Option<String>,
    /// What the source states about this binding that the ids do not carry.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetFile {
    #[serde(default, rename = "preset")]
    pub presets: Vec<Preset>,
}

impl Preset {
    /// False when any citation rests on an abstract or a secondary source. A preset is only
    /// as citable as the weakest source behind it.
    pub fn every_source_obtained(&self) -> bool {
        !self.citations.is_empty() && self.citations.iter().all(|citation| citation.obtained)
    }
}

/// The preset population's rules, checked against the registry the preset names into.
///
/// A preset naming a method the registry does not carry is a load-time violation. A preset
/// naming a method that exists but has no rule behind it is not: that would make the
/// registry unloadable the moment a preset cites an entry whose rule has not landed,
/// freezing breadth behind preset maintenance. That case is a refusal where the user asked
/// for it, carrying the same three names a load failure would.
pub fn validate(registry: &Registry) -> Vec<Violation> {
    let mut violations = Vec::new();
    for preset in registry.presets.values() {
        if preset.citations.is_empty() {
            violations.push(Violation {
                entry: preset.id.clone(),
                kind: ViolationKind::PresetWithoutCitation {
                    preset: preset.id.clone(),
                },
            });
        }

        // Against the declared constructs rather than the slots that run today, because a
        // source may legitimately be silent about a construct nothing executes yet.
        for construct in &preset.states_nothing_about {
            if !registry.constructs.contains_key(construct) {
                violations.push(Violation {
                    entry: preset.id.clone(),
                    kind: ViolationKind::PresetSilentAboutUnknownConstruct {
                        preset: preset.id.clone(),
                        construct: construct.clone(),
                    },
                });
            }
        }

        let mut seen_constructs: Vec<&str> = Vec::new();
        for binding in &preset.bindings {
            if seen_constructs.contains(&binding.construct.as_str()) {
                violations.push(Violation {
                    entry: preset.id.clone(),
                    kind: ViolationKind::PresetBindsOneConstructTwice {
                        preset: preset.id.clone(),
                        construct: binding.construct.clone(),
                    },
                });
            }
            seen_constructs.push(&binding.construct);

            // A composition is checked through the entry it composes, which is the row
            // carrying the citations it inherits. The id itself has no row.
            let checked_id = binding
                .composed_from
                .as_ref()
                .unwrap_or(&binding.method_id)
                .clone();
            match registry.methods.get(&checked_id) {
                None => violations.push(Violation {
                    entry: preset.id.clone(),
                    kind: ViolationKind::PresetBindsUnknownMethod {
                        preset: preset.id.clone(),
                        method_id: checked_id.clone(),
                    },
                }),
                Some(method) if method.construct != binding.construct => {
                    violations.push(Violation {
                        entry: preset.id.clone(),
                        kind: ViolationKind::PresetBindingConstructMismatch {
                            preset: preset.id.clone(),
                            method_id: checked_id.clone(),
                            declared: binding.construct.clone(),
                            actual: method.construct.clone(),
                        },
                    })
                }
                Some(_) => {}
            }
        }
    }
    violations
}
