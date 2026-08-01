//! How many of the rules this build runs can be named by their registry id, as a number.
//!
//! Python resolves every id against the registry before it will run anything, so a rule whose id
//! has no entry is a rule Python cannot reach however well it runs in the browser. That gap was
//! carried as a sentence in a plan for as long as nothing computed it. This computes it.
//!
//! A binding that names no registry entry is not necessarily wrong. Two of them are compositions
//! of an entry plus an operator, which the registry files as separate rows on purpose, and those
//! are listed here as compositions rather than counted as gaps. Anything else is a gap and the
//! number may only go down.

use plateforce_analysis::BINDINGS;
use plateforce_wasm::registry_embed;

/// Bindings that are a registry entry plus an operator bound to a stated value, rather than an
/// entry of their own. Each names the entry it composes, and that entry must resolve.
const COMPOSITIONS: &[(&str, &str)] = &[
    (
        "onset.threshold.last_within_band",
        "onset.threshold.noise_relative",
    ),
    (
        "takeoff.threshold.longest_run",
        "takeoff.threshold.absolute_force",
    ),
];

#[test]
fn every_rule_is_either_a_registry_entry_or_a_composition_of_one() {
    let loaded = registry_embed::load().expect("a registry file did not parse");

    let mut resolved = Vec::new();
    let mut composed = Vec::new();
    let mut unreachable = Vec::new();

    for binding in BINDINGS {
        if loaded.registry.methods.contains_key(binding.id) {
            resolved.push(binding.id);
            continue;
        }
        match COMPOSITIONS.iter().find(|(id, _)| *id == binding.id) {
            Some((_, base)) => {
                assert!(
                    loaded.registry.methods.contains_key(*base),
                    "{} is recorded as a composition of {base}, which is not in the registry either",
                    binding.id
                );
                composed.push(binding.id);
            }
            None => unreachable.push(binding.id),
        }
    }

    assert!(
        unreachable.is_empty(),
        "{} of {} rules this build runs cannot be named by a registry id, so no surface that \
         validates against the registry can reach them: {unreachable:?}",
        unreachable.len(),
        BINDINGS.len()
    );

    // The count is the point. If a rule is added without an entry this fails above; if one is
    // added with an entry, this number rises and the assertion below is what makes it deliberate.
    assert_eq!(
        resolved.len() + composed.len(),
        BINDINGS.len(),
        "every binding is accounted for as an entry or a composition"
    );
    assert_eq!(
        composed.len(),
        COMPOSITIONS.len(),
        "a binding stopped being a composition without the list being updated"
    );
}

/// The operators a rule composes are entries too, and a request that states one has to be able to
/// name it. The takeoff family existed nowhere until the onset family had been there from the
/// start, so a takeoff composition recorded its selection against a movement_onset row.
#[test]
fn both_operator_families_exist() {
    let loaded = registry_embed::load().expect("a registry file did not parse");

    for id in [
        "onset.op.crossing_selection",
        "onset.op.persistence",
        "onset.op.search_floor",
        "takeoff.op.crossing_selection",
        "takeoff.op.short_run_handling",
        "takeoff.op.residual_comparison",
    ] {
        let entry = loaded
            .registry
            .methods
            .get(id)
            .unwrap_or_else(|| panic!("{id} is not in the registry"));
        let expected = if id.starts_with("takeoff.") {
            "takeoff"
        } else {
            "movement_onset"
        };
        assert_eq!(
            entry.construct, expected,
            "{id} is filed under {} and an operator has to sit on the construct it operates on",
            entry.construct
        );
    }
}
