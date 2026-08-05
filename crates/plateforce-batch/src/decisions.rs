//! Which choices on a requested path nobody has made yet.
//!
//! A decision is per construct, not per entry: a construct carrying six forced entries is
//! one decision, and a construct carrying none is not a decision at all however many entries
//! it holds. Batch refuses while any such choice is open, so no artifact leaves the machine
//! carrying a rule the user never picked.
//!
//! Naming the rule does not always close the choice. A rule that requires a number the
//! literature publishes several ways leaves the number open, and `open_parameters` is that
//! second question. Both live here rather than beside the surface that asks them, because a
//! folder run and a single trial answering one request two ways is the defect this module
//! exists to stop, and it is the one that was here: the terminal refused an unnamed `onset.k`
//! and the folder ran every trial at the code's own value, recording that nobody was asked.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_registry::{Method, Registry, Surfacing};
use serde::Serialize;

/// One construct on the requested path whose choice is still open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnresolvedDecision {
    pub construct: String,
    /// The entries on this construct that force a decision.
    pub forcing_entries: Vec<String>,
    /// What could be bound instead, so a caller prints candidates without a second query.
    pub published_alternatives: Vec<String>,
}

impl UnresolvedDecision {
    pub fn message(&self) -> String {
        format!(
            "{} is still to be chosen, and {} of {} published rules for it force the choice",
            self.construct,
            self.forcing_entries.len(),
            self.published_alternatives.len()
        )
    }
}

/// Every construct on `path` carrying a `force_a_decision` entry that `resolved` does not
/// name. Ordered as `path` gives them, so a caller prints the pipeline in the order it runs.
pub fn unresolved(
    registry: &Registry,
    path: &[&str],
    resolved: &BTreeSet<String>,
) -> Vec<UnresolvedDecision> {
    let mut open = Vec::new();
    for construct in path {
        if resolved.contains(*construct) {
            continue;
        }
        let mut forcing_entries = Vec::new();
        let mut published_alternatives = Vec::new();
        for method in registry.methods.values() {
            if method.construct != *construct {
                continue;
            }
            published_alternatives.push(method.id.clone());
            if method
                .gui
                .as_ref()
                .is_some_and(|gui| gui.surfacing == Surfacing::ForceADecision)
            {
                forcing_entries.push(method.id.clone());
            }
        }
        if !forcing_entries.is_empty() {
            forcing_entries.sort();
            published_alternatives.sort();
            open.push(UnresolvedDecision {
                construct: (*construct).to_string(),
                forcing_entries,
                published_alternatives,
            });
        }
    }
    open
}

/// A value a bound rule requires, that the literature publishes more than one way, and that
/// the request did not name.
///
/// Carried out of a refusal rather than flattened into its sentence, so a browser and a
/// notebook lay the choice out their own way and a terminal is not the only surface that can
/// show what is being chosen between.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnresolvedValue {
    pub construct: String,
    /// The rule that requires it. The choice belongs to the rule, so naming a different one
    /// for the same construct asks a different question.
    pub method_id: String,
    pub name: String,
    /// What the literature publishes, as the registry lists it.
    pub published_values: Vec<f64>,
}

impl UnresolvedValue {
    pub fn message(&self) -> String {
        format!(
            "{}.{} is still to be chosen, and {} was published {} ways",
            self.construct,
            self.name,
            self.method_id,
            self.published_values.len()
        )
    }
}

/// A parameter is unresolved when its construct forces the decision, the rule requires it,
/// the literature publishes more than one value for it, and the request named none.
///
/// Every one of the four tests earns its place. Dropping the requirement test makes
/// `takeoff.threshold.absolute_force`'s persistence an unmade decision on a construct
/// carrying no forced entry, so a fully specified run would refuse.
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

/// The parameters on a bound rule that the literature does not agree on, which is a
/// different question from the parameters it reads.
pub fn published_choices(method: &Method) -> Vec<(String, Vec<f64>)> {
    method
        .parameters
        .iter()
        .filter(|parameter| parameter.required && parameter.published_values.len() > 1)
        .map(|parameter| (parameter.name.clone(), parameter.published_values.clone()))
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

/// Every value on the path the literature publishes more than one way, whether or not the
/// request named it. The denominator `unresolved_values` is counted against.
///
/// Reached by asking the same question of a request that stated nothing, so the count and the
/// open set cannot be taken over two different rules.
pub fn values_forcing_a_choice(
    registry: &Registry,
    bound: &[(&str, &str, &BTreeMap<String, f64>)],
) -> usize {
    let nothing_stated = BTreeMap::new();
    let named_none: Vec<(&str, &str, &BTreeMap<String, f64>)> = bound
        .iter()
        .map(|(construct, method_id, _)| (*construct, *method_id, &nothing_stated))
        .collect();
    unresolved_values(registry, &named_none).len()
}

/// Every value still open across a whole requested path, in the order the path runs.
///
/// `bound` gives each construct the rule it holds and the values the request stated against
/// it. A construct with no rule is a decision rather than a value and `unresolved` above has
/// already spoken for it.
pub fn unresolved_values(
    registry: &Registry,
    bound: &[(&str, &str, &BTreeMap<String, f64>)],
) -> Vec<UnresolvedValue> {
    let mut open = Vec::new();
    for (construct, method_id, stated) in bound {
        if method_id.is_empty() {
            continue;
        }
        for (name, published_values) in open_parameters(registry, construct, method_id, stated) {
            open.push(UnresolvedValue {
                construct: (*construct).to_string(),
                method_id: (*method_id).to_string(),
                name,
                published_values,
            });
        }
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
            .expect("the committed registry loads")
    }

    #[test]
    fn a_jump_height_path_with_nothing_resolved_is_two_decisions_and_not_nine() {
        let registry = registry();
        let path = ["system_weight", "movement_onset", "takeoff"];
        let open = unresolved(&registry, &path, &BTreeSet::new());

        let forcing: usize = open.iter().map(|item| item.forcing_entries.len()).sum();
        println!(
            "unresolved: {} of {} constructs on this path, over {} forcing entries",
            open.len(),
            path.len(),
            forcing
        );
        assert_eq!(open.len(), 2, "a decision is per construct");
        assert_eq!(forcing, 9, "nine entries force, across two constructs");
        assert_eq!(open[0].construct, "system_weight");
        assert_eq!(open[0].forcing_entries.len(), 6);
        assert_eq!(open[1].construct, "movement_onset");
        assert_eq!(open[1].forcing_entries.len(), 3);
    }

    #[test]
    fn takeoff_carries_entries_and_no_decision() {
        let registry = registry();
        let takeoff: Vec<&str> = registry
            .methods
            .values()
            .filter(|method| method.construct == "takeoff")
            .map(|method| method.id.as_str())
            .collect();
        println!("takeoff entries: {} of 252", takeoff.len());
        assert!(!takeoff.is_empty(), "takeoff carries entries");
        assert!(unresolved(&registry, &["takeoff"], &BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_resolved_construct_is_no_longer_a_decision() {
        let registry = registry();
        let resolved = BTreeSet::from(["system_weight".to_string()]);
        let open = unresolved(
            &registry,
            &["system_weight", "movement_onset", "takeoff"],
            &resolved,
        );
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].construct, "movement_onset");
    }
}
