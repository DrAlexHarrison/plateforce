//! What a bias was measured against is a name, and a reader can resolve every name it takes.
//!
//! An entry, a construct, the entry itself, or one of the external criteria the vocabulary
//! declares. The field was free text, so a mistyped instrument became a fourth instrument and
//! the registry loaded.

use plateforce_registry::{assemble, ViolationKind};

const CONSTRUCTS: &str = r#"
[[construct]]
id = "jump_height_from_takeoff"
title = "Jump height from takeoff"
unit = "meters"
"#;

const NO_PARAMETER: &str = "";

const ONE_PARAMETER: &str = r#"
[[method.parameter]]
name = "gravity_meters_per_second_squared"
unit = "meters_per_second_squared"
"#;

/// One entry carrying one bias, whose criterion, kind, direction and parameters the tests set.
fn method(criterion: &str, kind: &str, direction: &str, parameter: &str) -> String {
    format!(
        r#"
[[method]]
id = "jumpheight.impulse_momentum"
construct = "jump_height_from_takeoff"
title = "Jump height from takeoff velocity"
rule = "Jump height is takeoff velocity squared over twice gravity."
status = "accepted"
confidence = "medium"
{parameter}
[[method.bias]]
magnitude = 1.0
unit = "meters"
direction = "{direction}"
criterion = "{criterion}"
criterion_kind = "{kind}"
"#
    )
}

fn violations(written: &str) -> Vec<ViolationKind> {
    assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/jump-height.toml", written),
    ])
    .expect("the files describe one registry")
    .violations
    .into_iter()
    .map(|violation| violation.kind)
    .collect()
}

#[test]
fn a_criterion_the_registry_carries_raises_nothing() {
    for criterion in ["jump_height_from_takeoff", "jumpheight.impulse_momentum"] {
        let raised = violations(&method(criterion, "model", "high", ONE_PARAMETER));
        assert!(raised.is_empty(), "{criterion}: {raised:?}");
    }
}

/// The three instruments the vocabulary declares are criteria a bias may name and the registry
/// does not carry, so they pass the same check that refuses a name nobody can resolve.
#[test]
fn every_external_criterion_the_vocabulary_declares_raises_nothing() {
    for criterion in [
        "motion_capture_marker",
        "rubber_band_goniometer",
        "static_dead_weight_calibration",
    ] {
        let raised = violations(&method(criterion, "instrument", "high", NO_PARAMETER));
        assert!(raised.is_empty(), "{criterion}: {raised:?}");
    }
}

/// One character. The defect this rule exists for is not an invented instrument, it is a
/// typed one, and the two are indistinguishable to a field that accepts any string.
#[test]
fn a_criterion_naming_nothing_is_refused() {
    let raised = violations(&method(
        "motion_capture_marke",
        "instrument",
        "high",
        NO_PARAMETER,
    ));
    assert!(
        raised.contains(&ViolationKind::BiasCriterionUnresolved {
            criterion: "motion_capture_marke".to_string(),
        }),
        "{raised:?}"
    );
}

/// A model comparison against the entry itself sweeps a parameter of that entry, so an entry
/// declaring none has named itself for some other reason, most likely by copying a neighbour.
#[test]
fn a_model_self_comparison_with_no_parameter_to_sweep_is_refused() {
    let raised = violations(&method(
        "jumpheight.impulse_momentum",
        "model",
        "high",
        NO_PARAMETER,
    ));
    assert!(
        raised.contains(&ViolationKind::SelfComparisonSweepsNoParameter),
        "{raised:?}"
    );

    let with_one = violations(&method(
        "jumpheight.impulse_momentum",
        "model",
        "high",
        ONE_PARAMETER,
    ));
    assert!(
        !with_one.contains(&ViolationKind::SelfComparisonSweepsNoParameter),
        "{with_one:?}"
    );
}

/// Two implementations of one rule disagreeing is the other self-comparison, and it sweeps no
/// parameter, so the rule above must not reach it.
#[test]
fn two_implementations_of_one_rule_need_no_parameter() {
    let raised = violations(&method(
        "jumpheight.impulse_momentum",
        "instrument",
        "either",
        NO_PARAMETER,
    ));
    assert!(raised.is_empty(), "{raised:?}");
}

/// The definition of record is not biased against itself. The figure beside it is the
/// reference's own spread, so a direction there is a claim the comparison cannot support.
#[test]
fn a_definition_of_record_reporting_a_direction_is_refused() {
    let raised = violations(&method(
        "jumpheight.impulse_momentum",
        "human_visual",
        "high",
        NO_PARAMETER,
    ));
    assert!(
        raised.contains(&ViolationKind::DefinitionOfRecordCarriesADirection {
            direction: "high".to_string(),
        }),
        "{raised:?}"
    );

    for direction in ["none", "either"] {
        let spread = violations(&method(
            "jumpheight.impulse_momentum",
            "human_visual",
            direction,
            NO_PARAMETER,
        ));
        assert!(spread.is_empty(), "{direction}: {spread:?}");
    }
}
