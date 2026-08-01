//! The registry in this repository has to pass its own validation, here rather than in CI.
//!
//! `Registry::load` validates, and until this file existed the only thing that called it on the
//! shipped tree was the Python wheel smoke test in CI. So a registry edit could pass the whole
//! workspace suite locally, be pushed, and fail three minutes later on a rule the repository
//! already knew how to check. That happened on the entry this test was written beside: a
//! disagreement declared in one direction only, which validation catches and nothing local ran.

use plateforce_registry::Registry;

#[test]
fn the_registry_this_repository_ships_passes_validation() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry");
    match Registry::load(root) {
        Ok(registry) => {
            let census = registry.census();
            assert!(
                census.computation_entries > 200,
                "the registry loaded but holds only {} computation entries, so this test is \
                 pointed at the wrong tree",
                census.computation_entries
            );
        }
        Err(error) => panic!("the shipped registry does not validate:\n{error}"),
    }
}
