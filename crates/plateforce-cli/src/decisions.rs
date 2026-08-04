//! Which choices on the path to a number nobody has made yet, and what can be passed instead.
//!
//! A decision is per construct: a construct carrying six rules that force is one decision,
//! and a construct carrying none is not a decision at all however many rules it holds. On the
//! path to a jump height that is two, not nine and not a hundred and nineteen.
//!
//! The set that triggers a decision and the set a reader may choose from are different sets.
//! A rule with no code behind it still makes the construct contested, and it is never printed
//! as something to pass to a flag.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::bindings_for;
use plateforce_registry::{Method, Registry, Status, Surfacing};

use crate::render::{Renderer, Role};

/// A construct whose choice is open, and what this build can run for it.
pub struct OpenDecision {
    pub construct: String,
    pub flag: String,
    pub label: String,
    pub note: Option<String>,
    pub candidates: Vec<Candidate>,
}

pub struct Candidate {
    pub id: String,
    /// Where the field stands on this rule, in the registry's own word.
    pub standing: String,
    pub current: bool,
    /// Why a rule with no row of its own is still on offer.
    pub derivation: Option<String>,
    /// Parameters the literature publishes more than one value for, so a row shows what is
    /// being chosen between rather than only that a choice exists.
    pub published: Vec<(String, Vec<f64>)>,
}

impl Candidate {
    fn emphasis(&self) -> Role {
        if self.current {
            Role::Heading
        } else {
            Role::NotCurrent
        }
    }
}

/// Every construct on `path` that forces a choice and that `chosen` does not name.
pub fn open(
    registry: &Registry,
    path: &[&str],
    chosen: &BTreeMap<String, String>,
) -> Vec<OpenDecision> {
    let resolved: BTreeSet<String> = chosen.keys().cloned().collect();
    plateforce_batch::unresolved(registry, path, &resolved)
        .into_iter()
        .map(|open| {
            let slot = slot_of(&open.construct);
            OpenDecision {
                flag: format!("--{slot}"),
                label: label_of(registry, &open.construct),
                note: registry
                    .constructs
                    .get(&open.construct)
                    .and_then(|construct| construct.notes.clone()),
                candidates: candidates_for(registry, slot),
                construct: open.construct,
            }
        })
        .collect()
}

/// The rules this build can run for a slot, minus the ones the registry says are never a
/// user's to pick. An entry that forces the decision need not be runnable, and a runnable
/// rule the registry has not filed is still offered, flagged rather than hidden.
fn candidates_for(registry: &Registry, slot: &str) -> Vec<Candidate> {
    bindings_for(slot)
        .filter_map(|binding| {
            let entry = registry.methods.get(binding.id);
            if entry.is_some_and(|method| {
                matches!(
                    method.gui.as_ref().map(|gui| gui.surfacing),
                    Some(Surfacing::NeverAUserChoice) | Some(Surfacing::Refuse)
                )
            }) {
                return None;
            }
            let (standing, derivation) = match (entry, binding.composed_from) {
                (Some(method), _) => (method.status.to_string(), None),
                (None, Some(base)) => (
                    "composition".to_string(),
                    Some(format!(
                        "an operator bound onto {base}, whose citations it carries"
                    )),
                ),
                (None, None) => (
                    "unfiled".to_string(),
                    Some(format!(
                        "{}, which the registry files under another id",
                        binding.title
                    )),
                ),
            };
            Some(Candidate {
                id: binding.id.to_string(),
                current: entry.is_none_or(|method| {
                    !matches!(method.status, Status::Legacy | Status::Deprecated)
                }),
                standing,
                derivation,
                published: entry.map(published_choices).unwrap_or_default(),
            })
        })
        .collect()
}

/// The parameters on a bound rule that the literature does not agree on, which is a
/// different question from the parameters it reads.
fn published_choices(method: &Method) -> Vec<(String, Vec<f64>)> {
    method
        .parameters
        .iter()
        .filter(|parameter| parameter.required && parameter.published_values.len() > 1)
        .map(|parameter| (parameter.name.clone(), parameter.published_values.clone()))
        .collect()
}

/// A parameter is unresolved when its construct forces the decision, the rule requires it,
/// the literature publishes more than one value for it, and the request named none. Dropping
/// the requirement test makes `takeoff.threshold.absolute_force`'s persistence an unmade
/// decision on a construct carrying no forced entry, so a fully specified run would refuse.
pub fn open_parameters(
    registry: &Registry,
    construct: &str,
    method_id: &str,
    stated: &BTreeMap<String, f64>,
) -> Vec<(String, Vec<f64>)> {
    if !construct_forces(registry, construct) {
        return Vec::new();
    }
    let Some(method) = registry.methods.get(method_id) else {
        return Vec::new();
    };
    published_choices(method)
        .into_iter()
        .filter(|(name, _)| !stated.contains_key(name))
        .collect()
}

fn construct_forces(registry: &Registry, construct: &str) -> bool {
    registry.methods.values().any(|method| {
        method.construct == construct
            && method
                .gui
                .as_ref()
                .is_some_and(|gui| gui.surfacing == Surfacing::ForceADecision)
    })
}

/// The flag a construct is chosen with, read off the binding table rather than held as a
/// second list that would disagree with it.
pub fn slot_of(construct: &str) -> &'static str {
    plateforce_analysis::BINDINGS
        .iter()
        .find(|binding| binding.construct == construct)
        .map(|binding| binding.slot)
        .unwrap_or("")
}

/// The field's spoken words for a construct, which the registry carries. `onset` appears in
/// none of six course documents and "start of the jump" appears in nine places across them.
pub fn label_of(registry: &Registry, construct: &str) -> String {
    registry
        .constructs
        .get(construct)
        .and_then(|entry| entry.label.clone())
        .unwrap_or_else(|| construct.to_string())
}

/// The refusal, as a reader meets it: the count of open choices, then one block per
/// construct naming what it is in the field's words, what the choice is worth, and every
/// rule that can be passed with what the literature publishes for it.
pub fn describe(
    open: &[OpenDecision],
    constructs_on_the_path: usize,
    renderer: &Renderer,
) -> String {
    let mut lines = vec![format!(
        "{} of {constructs_on_the_path} choices on the path to a jump height have no default.",
        open.len(),
    )];
    for decision in open {
        lines.push(String::new());
        // The field's spoken words name the choice and the registry's id identifies it, on
        // one row.
        lines.push(format!(
            "  {} <METHOD>   {}   {}",
            decision.flag, decision.label, decision.construct
        ));
        if let Some(note) = &decision.note {
            lines.extend(renderer.wrap(note, 6));
        }
        for candidate in &decision.candidates {
            lines.push(format!(
                "      {:<52}{}",
                candidate.id,
                renderer.paint(candidate.emphasis(), &candidate.standing)
            ));
            if let Some(derivation) = &candidate.derivation {
                lines.extend(renderer.wrap(derivation, 10));
            }
            for (name, values) in &candidate.published {
                lines.extend(
                    renderer.wrap(&format!("{name} published at {}", join_numbers(values)), 10),
                );
            }
        }
    }
    lines.join("\n")
}

/// TOML floats carry a decimal point and `f64`'s Display drops it on a whole number, so
/// `20` on screen would not match the `20.0` in the file a reader goes on to search.
fn join_numbers(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}
