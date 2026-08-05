//! What this registry says can be computed, and what stands in the way of the rest.
//!
//! A construct a practitioner reports, then either the rules that reach it or the barrier
//! between them and a recording. The barrier is a fact about the operator's movements and
//! instruments, so a row naming one names something they can act on: a drop jump they have
//! not recorded, a second plate they do not own, or a rule nobody can obtain.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use plateforce_core::Refusal;
use plateforce_registry::{Boundary, Registry};
use serde_json::json;

use crate::exit::{Declined, Outcome};
use crate::out::Format;
use crate::registry_cmd::canonical;
use crate::render::{Renderer, Role};

/// The four the report names, from the five the registry files. `Both` is a movement and an
/// instrument at once, so it names both rather than being rounded to either. How many entries
/// that costs is a query, and the test below runs it against the registry this build carries.
fn barriers_of(boundary: Boundary) -> &'static [&'static str] {
    match boundary {
        Boundary::Protocol => &["movement"],
        Boundary::Equipment => &["instrument"],
        Boundary::Both => &["movement", "instrument"],
        Boundary::Source => &["rule"],
        Boundary::Undetermined => &["undetermined"],
    }
}

struct ConstructReach {
    id: String,
    label: String,
    reachable: bool,
    /// Whether this build has a rule bound for it, which is a different question from whether
    /// a recording could support one.
    computed: bool,
    barriers: BTreeSet<&'static str>,
    /// What would settle an undetermined barrier, carried rather than replaced by a guess.
    queries: Vec<String>,
}

/// A construct is reachable when one of its rules is, because a practitioner who can compute
/// it by one published rule can compute it. One that is not carries every barrier its rules
/// meet, so a construct walled by different things names all of them.
fn reach_of(registry: &Registry) -> Vec<ConstructReach> {
    let mut rules_by_construct: BTreeMap<&str, Vec<&plateforce_registry::Method>> = BTreeMap::new();
    for method in registry.methods.values() {
        rules_by_construct
            .entry(method.construct.as_str())
            .or_default()
            .push(method);
    }

    registry
        .constructs
        .values()
        .map(|construct| {
            let rules = rules_by_construct
                .get(construct.id.as_str())
                .cloned()
                .unwrap_or_default();
            let reachable = rules.iter().any(|method| method.reach.is_none());
            let computed = plateforce_analysis::BINDINGS
                .iter()
                .any(|binding| binding.construct == construct.id);
            let mut barriers = BTreeSet::new();
            let mut queries = Vec::new();
            if !reachable {
                for method in &rules {
                    let Some(reach) = &method.reach else { continue };
                    barriers.extend(barriers_of(reach.boundary));
                    if let Some(query) = &reach.query {
                        queries.push(format!("{}: {query}", method.id));
                    }
                }
            }
            ConstructReach {
                id: construct.id.clone(),
                label: construct
                    .label
                    .clone()
                    .unwrap_or_else(|| construct.id.clone()),
                reachable,
                computed,
                barriers,
                queries,
            }
        })
        .collect()
}

pub fn run(registry_directory: Option<&Path>, format: Format, renderer: &Renderer) -> Outcome {
    let registry = match crate::registry_source::load(registry_directory) {
        Ok(registry) => registry,
        Err(error) => {
            return Outcome::declined(Declined::recorded(Refusal::registry_invalid(format!(
                "{error}"
            ))))
        }
    };

    let rows = reach_of(&registry);
    let reachable = rows.iter().filter(|row| row.reachable).count();
    let computed = rows.iter().filter(|row| row.computed).count();
    // The denominator behind every row. A registry whose entries declare no barrier reports
    // every construct reachable, and the two numbers together are what tell a reader which
    // of the two they are looking at.
    let declared = registry
        .methods
        .values()
        .filter(|method| method.reach.is_some())
        .count();

    match format {
        Format::Json => Outcome::complete(canonical(&json!({
            "construct_count": rows.len(),
            "reachable_count": reachable,
            "computed_count": computed,
            "entries_declaring_a_boundary": declared,
            "computation_entry_count": registry.methods.len(),
            "constructs": rows.iter().map(|row| json!({
                "id": row.id,
                "reachable": row.reachable,
                "computed": row.computed,
                "boundary": row.barriers.iter().collect::<Vec<_>>(),
                "query": query_of(row),
            })).collect::<Vec<_>>(),
        }))),
        Format::Text => Outcome::complete(text_body(
            &rows, computed, reachable, declared, &registry, renderer,
        )),
    }
}

fn query_of(row: &ConstructReach) -> Option<String> {
    if row.queries.is_empty() {
        None
    } else {
        Some(row.queries.join("; "))
    }
}

fn text_body(
    rows: &[ConstructReach],
    computed: usize,
    reachable: usize,
    declared: usize,
    registry: &Registry,
    renderer: &Renderer,
) -> String {
    let mut document = String::new();
    let widest = rows.iter().map(|row| row.id.len()).max().unwrap_or(0);

    let _ = writeln!(
        document,
        "{}",
        renderer.paint(Role::Heading, "Within reach")
    );
    for row in rows.iter().filter(|row| row.reachable) {
        let _ = writeln!(document, "  {:<widest$}  {}", row.id, row.label);
    }

    let walled: Vec<&ConstructReach> = rows.iter().filter(|row| !row.reachable).collect();
    if !walled.is_empty() {
        let _ = writeln!(document);
        let _ = writeln!(
            document,
            "{}",
            renderer.paint(Role::Heading, "What stands in the way")
        );
        for row in walled {
            let _ = writeln!(
                document,
                "  {:<widest$}  {}",
                row.id,
                row.barriers.iter().copied().collect::<Vec<_>>().join(", ")
            );
            for line in renderer.wrap(&row.label, 4) {
                let _ = writeln!(document, "{line}");
            }
            if let Some(query) = query_of(row) {
                for line in renderer.wrap(&query, 4) {
                    let _ = writeln!(document, "{line}");
                }
            }
        }
    }

    // Three populations, each against its own denominator and never added together. The first
    // is what this build computes and is the one a floor is held to. The second counts
    // constructs nothing stands in the way of, and it falls as barriers are classified.
    let _ = writeln!(document);
    let _ = write!(
        document,
        "{computed} of {} constructs compute. {reachable} of {} reachable, from {declared} of {} computation entries declaring a boundary",
        rows.len(),
        rows.len(),
        registry.methods.len()
    );
    document
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five field values, four report words, and `both` is the one that carries two.
    #[test]
    fn every_boundary_the_registry_files_names_a_barrier_a_reader_can_act_on() {
        let filed = [
            Boundary::Protocol,
            Boundary::Equipment,
            Boundary::Both,
            Boundary::Source,
            Boundary::Undetermined,
        ];
        let named: BTreeSet<&str> = filed
            .iter()
            .flat_map(|boundary| barriers_of(*boundary).iter().copied())
            .collect();
        assert_eq!(
            named,
            BTreeSet::from(["movement", "instrument", "rule", "undetermined"])
        );
        assert_eq!(barriers_of(Boundary::Both).len(), 2);
        println!(
            "boundaries the registry files: {}, barriers the report names: {}",
            filed.len(),
            named.len()
        );
    }

    /// What collapsing `Both` onto one word would cost, asked of this build's registry rather
    /// than written in prose beside the mapping.
    ///
    /// A count in a comment is checked by nobody and goes stale on a registry data edit that
    /// compiles. This runs the count, prints it against the denominator it is taken over, and
    /// holds the mapping to the population that reaches it: entries carrying `both` exist, so
    /// the word `both` has to name two barriers or those entries lose one on the report.
    #[test]
    fn the_boundary_naming_two_barriers_names_them_for_the_entries_that_carry_it() {
        let registry = crate::registry_source::load(None).expect("this build carries a registry");
        let declaring: Vec<&plateforce_registry::Method> = registry
            .methods
            .values()
            .filter(|method| method.reach.is_some())
            .collect();
        let carrying_both = declaring
            .iter()
            .filter(|method| {
                method
                    .reach
                    .as_ref()
                    .is_some_and(|reach| reach.boundary == Boundary::Both)
            })
            .count();

        println!(
            "computation entries: {}, declaring a boundary: {}, carrying both: {carrying_both}",
            registry.methods.len(),
            declaring.len()
        );
        // The control. With no entry carrying `both`, the assertion under it is about a
        // population of zero and passes whatever the mapping says.
        assert!(
            carrying_both > 0,
            "no entry of the {} declaring a boundary carries both, so the mapping below is held \
             against nothing",
            declaring.len()
        );
        assert_eq!(
            barriers_of(Boundary::Both).len(),
            2,
            "{carrying_both} entries carry both, and the report names {:?} for them",
            barriers_of(Boundary::Both)
        );
    }
}
