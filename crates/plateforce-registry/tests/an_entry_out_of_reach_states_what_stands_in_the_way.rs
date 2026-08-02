//! What stops an entry being computed on a recording is a fact about the entry, so it sits
//! beside the entry rather than in a table somebody keeps in step by hand.

use plateforce_registry::{assemble, Boundary, ViolationKind};

const CONSTRUCTS: &str = r#"
[[construct]]
id = "jump_height_from_takeoff"
title = "Jump height from takeoff"
unit = "meters"
"#;

const METHOD: &str = r#"
[[method]]
id = "jumpheight.motion_capture"
construct = "jump_height_from_takeoff"
title = "Marker displacement between standing and apex"
rule = "Jump height is the rise of a marker between standing and apex."
status = "accepted"
confidence = "medium"

[method.reach]
boundary = "equipment"
"#;

fn assembled_with(method: &str) -> plateforce_registry::Assembled {
    assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/jump-height.toml", method),
    ])
    .expect("the files describe one registry")
}

#[test]
fn an_entry_names_its_boundary_and_raises_nothing() {
    let assembled = assembled_with(METHOD);
    assert!(
        assembled.violations.is_empty(),
        "{:?}",
        assembled.violations
    );
    let reach = assembled.registry.methods["jumpheight.motion_capture"]
        .reach
        .as_ref()
        .expect("the entry carries a reach block");
    assert_eq!(reach.boundary, Boundary::Equipment);
    assert_eq!(reach.query, None);
}

/// An entry nobody classified carries the query that would settle it, which is the one case
/// where a question beside a boundary is the content rather than doubt about it.
#[test]
fn an_undetermined_boundary_carries_the_query_that_would_settle_it() {
    let assembled = assembled_with(&METHOD.replace(
        "boundary = \"equipment\"",
        "boundary = \"undetermined\"\nquery = \"the plate model and channel order from the thesis methods\"",
    ));
    assert!(
        assembled.violations.is_empty(),
        "{:?}",
        assembled.violations
    );
}

#[test]
fn a_query_beside_a_settled_boundary_is_refused() {
    let assembled = assembled_with(&METHOD.replace(
        "boundary = \"equipment\"",
        "boundary = \"equipment\"\nquery = \"is it really though\"",
    ));
    let kinds: Vec<ViolationKind> = assembled
        .violations
        .into_iter()
        .map(|violation| violation.kind)
        .collect();
    assert!(
        kinds.contains(&ViolationKind::ReachQueryOnSettledBoundary {
            boundary: Boundary::Equipment,
        }),
        "{kinds:?}"
    );
}

/// The five spellings are the whole vocabulary. A sixth would reach a surface with no arm
/// for it, so it fails to parse rather than loading as a boundary nobody can render.
#[test]
fn a_boundary_the_vocabulary_does_not_carry_does_not_parse() {
    let Err(error) = assemble([
        ("constructs.toml", CONSTRUCTS),
        (
            "methods/jump-height.toml",
            &METHOD.replace("boundary = \"equipment\"", "boundary = \"inconvenience\""),
        ),
    ]) else {
        panic!("a sixth boundary spelling assembled into a registry");
    };
    assert!(error.to_string().contains("inconvenience"), "{error}");
}

/// Every spelling the classification writes, against what serde reads, so a mistyped variant
/// fails here rather than reporting an entry as reachable when the classification said it is
/// not.
#[test]
fn every_boundary_the_classification_writes_is_one_the_registry_reads() {
    for spelling in ["protocol", "equipment", "both", "source", "undetermined"] {
        let assembled = assembled_with(&METHOD.replace(
            "boundary = \"equipment\"",
            &format!("boundary = \"{spelling}\""),
        ));
        let reach = assembled.registry.methods["jumpheight.motion_capture"]
            .reach
            .as_ref()
            .expect("the entry carries a reach block");
        assert_eq!(reach.boundary.as_registry_str(), spelling);
    }
}
