//! Every id this build writes into a record can be looked up, as a number.
//!
//! A fingerprint carrying an id that resolves nowhere reaches a stranger's methods section and
//! stays there, and the only way they find out is to try the lookup and fail. Python refuses to
//! run an id the registry does not carry, so an unresolvable id is also a rule Python cannot
//! reach however well it runs in the browser.
//!
//! This used to excuse two ids through a list of compositions, on the ground that a composition
//! is an entry plus an operator and the registry files those separately. That was true and it
//! was still an escape hatch: the two ids went on being written into records nobody could look
//! up. They are no longer written. Selecting and recording are different acts, and a caller may
//! still select a compound name; what travels with the number is the entry it composes, with the
//! operator recorded beside it. So the set under test is what this build records, not what it
//! accepts, and it has no exceptions.

use plateforce_analysis::TAKEOFF_OPERATOR_IDS;
use plateforce_analysis::{records_under, Binding, BINDINGS, ONSET_OPERATOR_IDS};
use plateforce_wasm::registry_embed;

/// Every id this build can write into a record: the entry each rule is recorded under, and
/// every operator entry a rule can compose onto it.
fn recorded_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = BINDINGS
        .iter()
        .map(|binding| records_under(binding.id))
        .chain(ONSET_OPERATOR_IDS.iter().copied())
        .chain(TAKEOFF_OPERATOR_IDS.iter().copied())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[test]
fn every_id_this_build_records_resolves_in_the_registry() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let recorded = recorded_ids();

    // A scan that matches nothing reads exactly like a scan that found nothing wrong.
    assert!(
        recorded.len() >= BINDINGS.len(),
        "the recorded set is smaller than the binding table, so this is reading the wrong thing"
    );

    let unreachable: Vec<&str> = recorded
        .iter()
        .copied()
        .filter(|id| !loaded.registry.methods.contains_key(*id))
        .collect();

    assert!(
        unreachable.is_empty(),
        "{} of the {} ids this build records cannot be looked up, so a reader holding one of \
         these results cannot reach the rule that produced it: {unreachable:?}",
        unreachable.len(),
        recorded.len()
    );
}

/// The counterpart, and the reason this cannot quietly become the old exception list.
///
/// A redirect is only defensible while the id it redirects has no entry of its own. Given one it
/// would be a second spelling of a rule the registry already names, which is the duplicate
/// vocabulary the redirect exists to prevent, so the field has to come off in the same edit that
/// adds the row. Without this, a row could be added and the redirect left behind, and every other
/// assertion here would go on passing.
#[test]
fn an_id_is_redirected_only_while_the_registry_carries_no_row_for_it() {
    let loaded = registry_embed::load().expect("a registry file did not parse");

    let redirected: Vec<&Binding> = BINDINGS
        .iter()
        .filter(|binding| binding.records_under.is_some())
        .collect();
    assert!(
        !redirected.is_empty(),
        "no binding is redirected, so this is asserting nothing"
    );

    for binding in redirected {
        assert!(
            !loaded.registry.methods.contains_key(binding.id),
            "{} now has a registry entry of its own, so it must record under itself: drop its \
             records_under and let the entry carry the rule",
            binding.id
        );
    }
}

/// Composing does not make an id a redirect. `takeoff.threshold.descending_crossing` composes
/// `takeoff.threshold.absolute_force` and has an entry of its own, because no operator's
/// parameter enumerates a confirmed descending crossing as one of its values. That is the test
/// separating the two cases, and the reason this one is worth stating.
#[test]
fn a_composition_with_an_entry_of_its_own_records_under_itself() {
    let loaded = registry_embed::load().expect("a registry file did not parse");

    let composing: Vec<&Binding> = BINDINGS
        .iter()
        .filter(|binding| binding.composed_from.is_some())
        .collect();
    assert!(
        !composing.is_empty(),
        "no binding composes, so this is asserting nothing"
    );

    for binding in composing {
        if loaded.registry.methods.contains_key(binding.id) {
            assert_eq!(
                records_under(binding.id),
                binding.id,
                "{} is an entry and must record under itself",
                binding.id
            );
        }
        let base = binding.composed_from.expect("filtered on being present");
        assert!(
            loaded.registry.methods.contains_key(base),
            "{} composes {base}, which is not in the registry, so the citations it inherits \
             cannot be reached",
            binding.id
        );
    }
}

/// The operators a rule composes are entries too, and a request that states one has to be able
/// to name it. The takeoff family existed nowhere until the onset family had been there from the
/// start, so a takeoff composition recorded its selection against a movement_onset row.
#[test]
fn both_operator_families_sit_on_the_construct_they_operate_on() {
    let loaded = registry_embed::load().expect("a registry file did not parse");

    for id in ONSET_OPERATOR_IDS.iter().chain(TAKEOFF_OPERATOR_IDS) {
        let entry = loaded
            .registry
            .methods
            .get(*id)
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
