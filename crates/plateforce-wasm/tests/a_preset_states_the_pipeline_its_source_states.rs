//! A named published pipeline resolves to the document a person could have written by hand.
//!
//! The demonstration trial lives in this crate, and `plateforce-analysis` cannot reach it
//! without a circular dependency, so the test that runs a resolved preset lives here while
//! the resolution itself stays where the maths is.

use std::collections::BTreeMap;

use plateforce_analysis::method_set::MethodSet;
use plateforce_registry::{Preset, PresetBinding};
use plateforce_wasm::registry_embed;

fn shipped(id: &str) -> Preset {
    registry_embed::load()
        .expect("the embedded registry assembles")
        .registry
        .presets
        .get(id)
        .unwrap_or_else(|| panic!("{id} is a shipped preset"))
        .clone()
}

#[test]
fn owen2014_resolves_to_every_value_its_source_states_and_no_others() {
    let preset = shipped("owen2014");
    let document = MethodSet::from_preset(&preset, "0.1.0", "content-test", None)
        .expect("every rule owen2014 binds has a rule behind it");
    println!(
        "{}",
        serde_json::to_string_pretty(&document).expect("serialises")
    );

    assert_eq!(document.preset.as_deref(), Some("owen2014"));
    assert_eq!(
        document.schema,
        plateforce_analysis::method_set::METHOD_SET_SCHEMA
    );

    // Two constructs, because the source states two. The third is not guessed.
    let bound: Vec<&str> = document
        .bindings
        .iter()
        .map(|binding| binding.construct.as_str())
        .collect();
    assert_eq!(bound, vec!["system_weight", "movement_onset"]);
    assert!(
        !bound.contains(&"takeoff"),
        "a slot the source is silent about is absent rather than filled"
    );

    // All three parameters the registry files under this citation, including the operator
    // one. A document carrying two of them would run the same numbers and misattribute a
    // third of the pipeline to a default nobody chose.
    let onset = document
        .bindings
        .iter()
        .find(|binding| binding.construct == "movement_onset")
        .expect("the source states an onset rule");
    assert_eq!(onset.method_id, "onset.threshold.noise_relative");
    assert_eq!(onset.parameters.get("k"), Some(&5.0));
    assert_eq!(
        onset.parameters.get("offset_ms"),
        Some(&30.0),
        "the thirty millisecond step back is stated by the source and must be stated here"
    );

    let weighing = document
        .bindings
        .iter()
        .find(|binding| binding.construct == "system_weight")
        .expect("the source states a weighing rule");
    assert_eq!(weighing.method_id, "bwepoch.fixed_window");
    assert_eq!(weighing.parameters.get("duration"), Some(&1.0));

    // The document a preset produced reads the same as one a person wrote, so it round
    // trips through the wire without the preset name changing what it says.
    let written = serde_json::to_string(&document).expect("serialises");
    let read: MethodSet = serde_json::from_str(&written).expect("reads back");
    assert_eq!(read, document);
}

/// A preset naming a rule the registry carries and this build does not run refuses where
/// the caller asked, rather than making the registry refuse to load for everyone.
#[test]
fn a_preset_binding_a_rule_this_build_does_not_run_refuses_by_name() {
    let mut preset = shipped("owen2014");
    preset.bindings.push(PresetBinding {
        construct: "movement_onset".into(),
        method_id: "onset.yank_inflection.sahrom2020".into(),
        parameters: BTreeMap::new(),
        options: BTreeMap::new(),
        note: None,
    });

    let refusal = MethodSet::from_preset(&preset, "0.1.0", "content-test", None)
        .expect_err("a rule with no implementation behind it refuses");
    println!("{}", refusal.message());
    assert!(refusal
        .message()
        .contains("onset.yank_inflection.sahrom2020"));
    assert!(
        refusal.message().contains("onset.threshold.noise_relative"),
        "the refusal names what the caller could have asked for instead"
    );
}
