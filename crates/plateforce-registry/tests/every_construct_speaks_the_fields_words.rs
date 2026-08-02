//! A construct a surface can reach carries the words a practitioner uses for it.
//!
//! The identifier stays visible wherever the label is shown, so this adds a name rather than
//! replacing one. Measured across six course documents, `takeoff` appears in 6 of 6 and
//! `onset`, `threshold` and `epoch` in 0 of 6, so an interface titled with identifiers alone
//! is titled in words two thirds of its audience has not met.

use plateforce_registry::Registry;

#[test]
fn every_declared_construct_carries_a_spoken_label() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry");
    let registry = Registry::load(root).expect("the shipped registry does not validate");

    let total = registry.constructs.len();
    let unlabelled: Vec<&str> = registry
        .constructs
        .values()
        .filter(|construct| {
            construct
                .label
                .as_ref()
                .is_none_or(|label| label.trim().is_empty())
        })
        .map(|construct| construct.id.as_str())
        .collect();

    assert!(
        unlabelled.is_empty(),
        "{} of {total} constructs carry no spoken label: {unlabelled:?}",
        unlabelled.len()
    );
}

/// Where the field's word for a quantity is the identifier, the label is that word. `takeoff`
/// appears in 6 of 6 course documents, so a label differing from it would be an invented
/// synonym rather than a translation.
#[test]
fn a_construct_the_field_names_plainly_keeps_that_name() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry");
    let registry = Registry::load(root).expect("the shipped registry does not validate");

    for id in ["takeoff", "landing"] {
        let label = registry.constructs[id].label.as_deref().unwrap_or_default();
        assert_eq!(
            label.to_lowercase(),
            id,
            "{id} is the word the field uses, so its label is that word"
        );
    }
}
