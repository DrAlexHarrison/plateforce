//! Which choices on a requested path nobody has made yet.
//!
//! A decision is per construct, not per entry: a construct carrying six forced entries is
//! one decision, and a construct carrying none is not a decision at all however many entries
//! it holds. Batch refuses while any such choice is open, so no artifact leaves the machine
//! carrying a rule the user never picked.

use std::collections::BTreeSet;

use plateforce_registry::{Registry, Surfacing};
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
