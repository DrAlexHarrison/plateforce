//! An id that resolves to nothing is answered with the entries filed beside it.
//!
//! A misspelling is the commonest way a reader meets this, and the dead end it used to reach
//! named nothing to try next. The rules for a landmark were already listed when one was passed
//! to `--onset`, and a mistyped subcommand already earns a suggestion, so the registry lookup
//! was the one place on that path that stopped.
//!
//! Answered here rather than on each surface because the terminal and Python had already
//! diverged on this condition, and four spellings of one answer is the conflation this project
//! exists to refuse.

use plateforce_registry::Registry;

fn registry() -> Registry {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../registry")
        .canonicalize()
        .expect("the registry directory is committed");
    Registry::load(root).expect("the committed registry loads")
}

/// One character off a real id, which is what a reader actually types.
#[test]
fn a_misspelling_reaches_the_id_it_was_reaching_for() {
    let registry = registry();
    let beside = registry.filed_beside("onset.threshold.noise_relatve");

    assert!(
        beside.contains(&"onset.threshold.noise_relative"),
        "the id being reached for is among them: {beside:?}"
    );
    assert!(
        beside
            .iter()
            .all(|id| registry.methods.contains_key(*id) || registry.protocols.contains_key(*id)),
        "every id offered resolves: {beside:?}"
    );
}

/// The answer is bounded by how the registry is filed rather than by a number chosen in the
/// code, so this reports what the data actually holds instead of asserting a cap.
#[test]
fn the_answer_is_the_namespace_rather_than_the_registry() {
    let registry = registry();
    let entries = registry.methods.len() + registry.protocols.len();

    let mut widest = 0;
    for id in registry.methods.keys().chain(registry.protocols.keys()) {
        let mut typed: Vec<&str> = id.split('.').collect();
        // A misspelling of the last segment, which is where one lands.
        let last = typed.len() - 1;
        typed[last] = "misspelled_leaf";
        widest = widest.max(registry.filed_beside(&typed.join(".")).len());
    }

    println!("entries: {entries}, widest set filed beside a misspelling: {widest}");
    assert!(
        widest < entries,
        "a misspelling reached the whole registry: {widest} of {entries}"
    );
}

/// A word from another vocabulary shares no segment with anything, and listing the registry at
/// somebody who typed one is worse than saying nothing.
#[test]
fn a_word_that_shares_no_segment_reaches_nothing() {
    assert!(registry().filed_beside("banana").is_empty());
    assert!(registry().filed_beside("crossfit.metcon.amrap").is_empty());
}

/// A real id is found rather than described, so this never fires on the path that works.
#[test]
fn an_id_that_resolves_is_not_a_near_miss() {
    let registry = registry();
    assert!(registry
        .methods
        .contains_key("onset.threshold.noise_relative"));
}
