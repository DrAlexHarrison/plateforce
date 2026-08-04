//! The registry in this repository has to pass its own validation, here rather than in CI.
//!
//! `Registry::load` validates, so without a local caller on the shipped tree a registry edit
//! passes the whole workspace suite here and fails minutes later in CI, on a rule the
//! repository already knows how to check. A disagreement declared in one direction only is
//! the shape of edit that does it.

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
