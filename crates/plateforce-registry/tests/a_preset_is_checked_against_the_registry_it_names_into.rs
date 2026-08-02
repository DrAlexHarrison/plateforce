//! A preset names rules that live elsewhere, so every one of its rules is a question about
//! two files at once. Each is asserted here by writing the fault and watching it fire.

use plateforce_registry::{assemble, ViolationKind};

const CONSTRUCTS: &str = r#"
[[construct]]
id = "system_weight"
title = "System weight"
unit = "newtons"

[[construct]]
id = "movement_onset"
title = "Movement onset"
unit = "seconds"

[[construct]]
id = "takeoff"
title = "Takeoff"
unit = "seconds"
"#;

const METHODS: &str = r#"
[[method]]
id = "bwepoch.fixed_window"
construct = "system_weight"
title = "Mean of force over a fixed span"
rule = "System weight is the mean of force over a fixed span."
status = "accepted"
confidence = "high"

[[method]]
id = "onset.threshold.noise_relative"
construct = "movement_onset"
title = "Threshold at a multiple of baseline noise"
rule = "Onset is the first crossing of k standard deviations of the weighing epoch."
status = "accepted"
confidence = "high"
"#;

const SOUND_PRESET: &str = r#"
[[preset]]
id = "owen2014"
title = "Owen et al. 2014"
description = "The pairing that paper states."

[[preset.binding]]
construct = "system_weight"
method_id = "bwepoch.fixed_window"

[[preset.binding]]
construct = "movement_onset"
method_id = "onset.threshold.noise_relative"

[[preset.citation]]
key = "owen2014"
role = "proposes"
reference = "Owen et al. 2014, JSCR 28(6):1552-1558"
obtained = false
"#;

fn violations_of(preset: &str) -> Vec<String> {
    let assembled = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/minimal.toml", METHODS),
        ("presets/under-test.toml", preset),
    ])
    .expect("the files describe one registry");
    assembled
        .violations
        .iter()
        .map(|violation| violation.to_string())
        .collect()
}

fn kinds_of(preset: &str) -> Vec<ViolationKind> {
    let assembled = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/minimal.toml", METHODS),
        ("presets/under-test.toml", preset),
    ])
    .expect("the files describe one registry");
    assembled
        .violations
        .into_iter()
        .map(|violation| violation.kind)
        .collect()
}

/// The control. Without it, every assertion below could be passing because this registry
/// raises violations whatever the preset says.
#[test]
fn a_preset_whose_bindings_all_resolve_raises_nothing() {
    assert_eq!(violations_of(SOUND_PRESET), Vec::<String>::new());
}

#[test]
fn a_preset_is_counted_on_its_own_denominator_and_never_added_to_another() {
    let assembled = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/minimal.toml", METHODS),
        ("presets/under-test.toml", SOUND_PRESET),
    ])
    .expect("the files describe one registry");
    let census = assembled.registry.census();
    assert_eq!(census.preset_entries, 1);
    assert_eq!(census.computation_entries, 2);
    assert_eq!(census.constructs, 3);
    assert_eq!(census.protocol_entries, 0);
}

#[test]
fn a_preset_naming_a_rule_the_registry_does_not_carry_is_refused() {
    let broken = SOUND_PRESET.replace(
        "method_id = \"bwepoch.fixed_window\"",
        "method_id = \"bwepoch.no_such_rule\"",
    );
    assert!(
        kinds_of(&broken).contains(&ViolationKind::PresetBindsUnknownMethod {
            preset: "owen2014".to_string(),
            method_id: "bwepoch.no_such_rule".to_string(),
        }),
        "{:?}",
        kinds_of(&broken)
    );
    assert_eq!(
        violations_of(&broken),
        vec![
            "owen2014: binds 'bwepoch.no_such_rule', which the registry does not carry".to_string()
        ]
    );
}

/// The four fields of this one are the likeliest to be wired to the wrong place, so the
/// rendered sentence is asserted rather than only the variant.
#[test]
fn a_preset_binding_a_rule_under_another_construct_is_refused_and_names_both() {
    let broken = SOUND_PRESET.replace(
        "method_id = \"bwepoch.fixed_window\"",
        "method_id = \"onset.threshold.noise_relative\"",
    );
    assert!(
        kinds_of(&broken).contains(&ViolationKind::PresetBindingConstructMismatch {
            preset: "owen2014".to_string(),
            method_id: "onset.threshold.noise_relative".to_string(),
            declared: "system_weight".to_string(),
            actual: "movement_onset".to_string(),
        }),
        "{:?}",
        kinds_of(&broken)
    );
    assert_eq!(
        violations_of(&broken),
        vec![
            "owen2014: binds 'onset.threshold.noise_relative' under construct 'system_weight', \
             and that entry's construct is 'movement_onset'"
                .to_string()
        ]
    );
}

#[test]
fn a_preset_stating_a_pipeline_with_no_source_for_it_is_refused() {
    let citation_removed = SOUND_PRESET.split("[[preset.citation]]").next().unwrap();
    assert!(
        kinds_of(citation_removed).contains(&ViolationKind::PresetWithoutCitation {
            preset: "owen2014".to_string(),
        }),
        "{:?}",
        kinds_of(citation_removed)
    );
    assert_eq!(
        violations_of(citation_removed),
        vec!["owen2014: states a pipeline and cites no source for it".to_string()]
    );
}

/// A preset asserting silence about a construct nobody declared would read as a source that
/// says nothing about it, and the software would agree.
#[test]
fn a_preset_silent_about_a_construct_the_registry_does_not_carry_is_refused() {
    let broken = SOUND_PRESET.replace(
        "description = \"The pairing that paper states.\"",
        "description = \"The pairing that paper states.\"\nstates_nothing_about = [\"takoff\"]",
    );
    assert!(
        kinds_of(&broken).contains(&ViolationKind::PresetSilentAboutUnknownConstruct {
            preset: "owen2014".to_string(),
            construct: "takoff".to_string(),
        }),
        "{:?}",
        kinds_of(&broken)
    );
    assert_eq!(
        violations_of(&broken),
        vec![
            "owen2014: states its source says nothing about 'takoff', which is not in constructs.toml"
                .to_string()
        ]
    );
}

/// Silence about a construct that exists is a fact about the source, and the rule that
/// catches the misspelling must not catch this.
#[test]
fn a_preset_silent_about_a_declared_construct_raises_nothing() {
    let silent = SOUND_PRESET.replace(
        "description = \"The pairing that paper states.\"",
        "description = \"The pairing that paper states.\"\nstates_nothing_about = [\"takeoff\"]",
    );
    assert_eq!(violations_of(&silent), Vec::<String>::new());
}

/// Two bindings for one construct leave a preset whose stated pipeline is not the one it
/// would run, and the surviving binding is decided by file order.
#[test]
fn a_preset_binding_one_construct_twice_is_refused() {
    let broken = SOUND_PRESET.replace(
        "construct = \"movement_onset\"\nmethod_id = \"onset.threshold.noise_relative\"",
        "construct = \"system_weight\"\nmethod_id = \"bwepoch.fixed_window\"",
    );
    assert!(
        kinds_of(&broken).contains(&ViolationKind::PresetBindsOneConstructTwice {
            preset: "owen2014".to_string(),
            construct: "system_weight".to_string(),
        }),
        "{:?}",
        kinds_of(&broken)
    );
    assert_eq!(
        violations_of(&broken),
        vec![
            "owen2014: binds construct 'system_weight' more than once, so one binding replaced another"
                .to_string()
        ]
    );
}

#[test]
fn two_presets_under_one_id_are_a_refusal_rather_than_a_census_of_one() {
    let Err(error) = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/minimal.toml", METHODS),
        ("presets/first.toml", SOUND_PRESET),
        ("presets/second.toml", SOUND_PRESET),
    ]) else {
        panic!("one id defined twice assembled into a registry of one");
    };
    assert!(error.to_string().contains("owen2014"), "{error}");
}

/// A preset file outside `presets/` would assemble into no population, and the sentence a
/// reader gets has to name the directory that would have held it.
#[test]
fn a_preset_file_outside_the_presets_directory_names_where_presets_live() {
    let Err(error) = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/minimal.toml", METHODS),
        ("draft.toml", SOUND_PRESET),
    ]) else {
        panic!("a file no population owns assembled into a registry");
    };
    assert!(error.to_string().contains("presets/"), "{error}");
}
