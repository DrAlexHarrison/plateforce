//! One case per rule, written by breaking a sound registry in one place.
//!
//! A rule that never fires in a test cannot be told apart from one whose condition is
//! unreachable, and both read as a validator that passes.

use plateforce_registry::{assemble, ViolationKind};

const CONSTRUCTS: &str = r#"
[[construct]]
id = "movement_onset"
title = "Movement onset"
unit = "seconds"

[[construct]]
id = "system_weight"
title = "System weight"
unit = "newtons"
"#;

/// Two entries that disagree with each other in both directions, one carrying a bias with
/// its criterion, a sourced default, a citation, a surfacing verdict and a failure rate
/// consistent with its own numerator and denominator.
const METHODS: &str = r#"
[[method]]
id = "onset.threshold.noise_relative"
construct = "movement_onset"
title = "Threshold at a multiple of baseline noise"
rule = "Onset is the first crossing of k standard deviations of the weighing epoch."
status = "accepted"
confidence = "high"

[[method.parameter]]
name = "k"
unit = "standard_deviations"
published_values = [5.0, 8.0]
default = 5.0
default_source = "owen2014"

[[method.parameter]]
name = "dwell_seconds"
unit = "seconds"
published_values = [1.0]
default = 1.0
default_source = "hawkin_glossary"
required = true

[[method.bias]]
magnitude = 0.011
unit = "meters"
criterion = "motion_capture_marker"
criterion_kind = "simultaneous_capture"

[[method.bias]]
magnitude = 1.0
unit = "seconds"
direction = "long"
equals_parameter = "dwell_seconds"
criterion = "system_weight"
criterion_kind = "model"

[[method.citation]]
key = "owen2014"
role = "proposes"
reference = "Owen et al. 2014, JSCR 28(6):1552-1558"
obtained = true

[[method.citation]]
key = "hawkin_glossary"
role = "uses"
reference = "Hawkin Dynamics glossary, the dwell this entry defaults to"
obtained = false

[method.gui]
surfacing = "force_a_decision"
rationale = "The choice moves the number by more than a training effect."

[method.failure]
rate = 0.1434
numerator = 35
denominator = 244
corpus = "the 244-trial corpus"
definition = "places onset more than two seconds before takeoff"
detectability = "silent"

[[method.disagrees_with]]
id = "onset.threshold.absolute_force"
kind = "genuine"

[[method]]
id = "onset.threshold.absolute_force"
construct = "movement_onset"
title = "Threshold at a fixed force"
rule = "Onset is the first crossing of a fixed force above system weight."
status = "accepted"
confidence = "high"

[[method.disagrees_with]]
id = "onset.threshold.noise_relative"
kind = "genuine"
"#;

const PROTOCOLS: &str = r#"
[[protocol]]
id = "acquisition.trimmed_recording"
area = "acquisition"
title = "The recording holds one jump"
description = "Two tools assume the recording was trimmed to a single jump."
affects = ["onset.threshold.noise_relative"]
provenance = "observed_from_code"
"#;

fn kinds_of(constructs: &str, methods: &str, protocols: &str) -> Vec<ViolationKind> {
    let assembled = assemble([
        ("constructs.toml", constructs),
        ("methods/onset.toml", methods),
        ("protocols/acquisition.toml", protocols),
    ])
    .expect("the files describe one registry");
    assembled
        .violations
        .into_iter()
        .map(|violation| violation.kind)
        .collect()
}

fn methods_broken(from: &str, to: &str) -> Vec<ViolationKind> {
    assert_eq!(
        METHODS.matches(from).count(),
        1,
        "the break matched something other than exactly one place"
    );
    kinds_of(CONSTRUCTS, &METHODS.replace(from, to), PROTOCOLS)
}

/// The control. Every assertion below rests on this registry being sound, so a rule firing
/// there would make each of them pass for the wrong reason.
#[test]
fn a_registry_that_breaks_no_rule_raises_nothing() {
    assert_eq!(kinds_of(CONSTRUCTS, METHODS, PROTOCOLS), Vec::new());
}

#[test]
fn an_id_that_is_not_a_dotted_canonical_name_is_refused() {
    let kinds = methods_broken(
        "id = \"onset.threshold.absolute_force\"\nconstruct",
        "id = \"absoluteforce\"\nconstruct",
    );
    assert!(kinds.contains(&ViolationKind::IdNotDotted), "{kinds:?}");
}

#[test]
fn a_method_naming_a_construct_that_is_not_declared_is_refused() {
    let kinds = methods_broken(
        "construct = \"movement_onset\"\ntitle = \"Threshold at a fixed force\"",
        "construct = \"movement_start\"\ntitle = \"Threshold at a fixed force\"",
    );
    assert!(
        kinds.contains(&ViolationKind::UnknownConstruct {
            construct: "movement_start".to_string(),
        }),
        "{kinds:?}"
    );
}

/// A protocol reaches entries rather than constructs alone, and the rule that checks it is a
/// second call site of the same variant.
#[test]
fn a_protocol_affecting_something_the_registry_does_not_carry_is_refused() {
    let kinds = kinds_of(
        CONSTRUCTS,
        METHODS,
        &PROTOCOLS.replace(
            "affects = [\"onset.threshold.noise_relative\"]",
            "affects = [\"onset.threshold.never_written\"]",
        ),
    );
    assert!(
        kinds.contains(&ViolationKind::UnknownConstruct {
            construct: "onset.threshold.never_written".to_string(),
        }),
        "{kinds:?}"
    );
}

#[test]
fn disagreeing_with_an_entry_that_does_not_exist_is_refused() {
    let kinds = methods_broken(
        "id = \"onset.threshold.absolute_force\"\nkind = \"genuine\"",
        "id = \"onset.threshold.imaginary\"\nkind = \"genuine\"",
    );
    assert!(
        kinds.contains(&ViolationKind::UnknownDisagreement {
            target: "onset.threshold.imaginary".to_string(),
        }),
        "{kinds:?}"
    );
}

/// Disagreement is a relationship, so one side recording it and the other not means a reader
/// arriving from the silent side never learns there is an argument.
#[test]
fn a_disagreement_the_other_side_does_not_record_is_refused() {
    let kinds = methods_broken(
        "\n[[method.disagrees_with]]\nid = \"onset.threshold.noise_relative\"\nkind = \"genuine\"\n",
        "\n",
    );
    assert!(
        kinds.contains(&ViolationKind::AsymmetricDisagreement {
            target: "onset.threshold.absolute_force".to_string(),
        }),
        "{kinds:?}"
    );
}

/// An absent criterion is a parse error, so what this rule catches is the blank one.
#[test]
fn a_bias_stated_against_a_blank_criterion_is_refused() {
    let kinds = methods_broken(
        "criterion = \"motion_capture_marker\"",
        "criterion = \"   \"",
    );
    assert!(
        kinds.contains(&ViolationKind::BiasWithoutCriterion),
        "{kinds:?}"
    );
}

/// A rule that waits a dwell before declaring stabilisation overstates by exactly that
/// dwell, so the recorded magnitude is that parameter's value rather than a constant. The
/// four cases below are the four ways that identity can be false while looking true.
#[test]
fn a_bias_equalling_a_parameter_this_entry_does_not_carry_is_refused() {
    let kinds = methods_broken(
        "equals_parameter = \"dwell_seconds\"",
        "equals_parameter = \"dwell_ms\"",
    );
    assert!(
        kinds.contains(&ViolationKind::BiasNamesUnknownParameter {
            parameter: "dwell_ms".to_string(),
        }),
        "{kinds:?}"
    );
}

#[test]
fn a_bias_equalling_a_parameter_that_declares_no_default_is_refused() {
    let kinds = methods_broken("default = 1.0\ndefault_source = \"hawkin_glossary\"\n", "");
    assert!(
        kinds.contains(&ViolationKind::BiasNamesParameterWithoutDefault {
            parameter: "dwell_seconds".to_string(),
        }),
        "{kinds:?}"
    );
}

/// The number and the parameter it claims to equal, held together. Without this the
/// magnitude is right at the published default and wrong everywhere else.
#[test]
fn a_bias_disagreeing_with_the_parameter_it_equals_is_refused() {
    let kinds = methods_broken(
        "default = 1.0\ndefault_source",
        "default = 2.0\ndefault_source",
    );
    assert!(
        kinds.contains(&ViolationKind::BiasMagnitudeDisagreesWithParameter {
            parameter: "dwell_seconds".to_string(),
            stated: 1.0,
            declared: 2.0,
        }),
        "{kinds:?}"
    );
}

/// An identity between quantities in different units is not an identity.
#[test]
fn a_bias_in_a_different_unit_from_the_parameter_it_equals_is_refused() {
    let kinds = methods_broken(
        "magnitude = 1.0\nunit = \"seconds\"\ndirection = \"long\"",
        "magnitude = 1.0\nunit = \"milliseconds\"\ndirection = \"long\"",
    );
    assert!(
        kinds.contains(&ViolationKind::BiasUnitDiffersFromParameter {
            parameter: "dwell_seconds".to_string(),
            stated: "milliseconds".to_string(),
            declared: "seconds".to_string(),
        }),
        "{kinds:?}"
    );
}

/// A bias that names no parameter is a fixed quantity and none of the four rules touch it.
#[test]
fn a_bias_that_names_no_parameter_is_left_alone() {
    let kinds = methods_broken("equals_parameter = \"dwell_seconds\"\n", "");
    assert_eq!(kinds, Vec::new());
}

#[test]
fn a_numeric_default_with_nobody_named_as_having_chosen_it_is_refused() {
    let kinds = methods_broken("default_source = \"owen2014\"\n", "");
    assert!(
        kinds.contains(&ViolationKind::DefaultWithoutSource {
            parameter: "k".to_string(),
        }),
        "{kinds:?}"
    );
}

/// Naming a chooser the entry does not cite reads as provenance and carries none. The route a
/// reader takes from a bound value is the entry it came from, so a key resolvable only on some
/// other entry, or nowhere, leaves them holding a number and a word.
#[test]
fn a_default_naming_a_chooser_this_entry_does_not_cite_is_refused() {
    let kinds = methods_broken(
        "default_source = \"owen2014\"\n",
        "default_source = \"owen2015\"\n",
    );
    assert!(
        kinds.contains(&ViolationKind::DefaultSourceNamesNoCitation {
            parameter: "k".to_string(),
            source: "owen2015".to_string(),
        }),
        "{kinds:?}"
    );
}

/// A recommendation is the strongest thing this registry says, so it may not rest on a
/// source nobody read.
#[test]
fn a_recommendation_resting_on_a_source_nobody_obtained_is_refused() {
    let kinds = methods_broken("obtained = true", "obtained = false");
    assert!(
        !kinds
            .iter()
            .any(|kind| matches!(kind, ViolationKind::RecommendedOnUnobtainedSource { .. })),
        "an accepted entry must not raise it: {kinds:?}"
    );

    let recommended = METHODS
        .replace("obtained = true", "obtained = false")
        .replacen("status = \"accepted\"", "status = \"recommended\"", 1);
    let kinds = kinds_of(CONSTRUCTS, &recommended, PROTOCOLS);
    assert!(
        kinds.contains(&ViolationKind::RecommendedOnUnobtainedSource {
            citation: "owen2014".to_string(),
        }),
        "{kinds:?}"
    );
}

/// Every other verdict decides its own behaviour. Refusing decides only that the rule is not
/// offered, so what a reader is owed instead has nowhere else to live.
#[test]
fn refusing_to_offer_a_rule_without_saying_what_the_refusal_is_for_is_refused() {
    let kinds = methods_broken(
        "surfacing = \"force_a_decision\"\nrationale = \"The choice moves the number by more than a training effect.\"",
        "surfacing = \"refuse\"",
    );
    assert!(
        kinds.contains(&ViolationKind::RefuseWithoutRationale),
        "{kinds:?}"
    );
}

#[test]
fn a_failure_rate_stated_without_a_denominator_is_refused() {
    let kinds = methods_broken("denominator = 244", "denominator = 0");
    assert!(
        kinds.contains(&ViolationKind::FailureWithoutDenominator),
        "{kinds:?}"
    );
}

#[test]
fn a_failure_rate_that_does_not_match_its_own_numerator_and_denominator_is_refused() {
    let kinds = methods_broken("rate = 0.1434", "rate = 0.9");
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, ViolationKind::FailureRateInconsistent { .. })),
        "{kinds:?}"
    );
}

/// The tolerance exists so a rounded literal passes. A rule that accepted any rate would
/// pass the case above too, so the boundary is asserted from both sides.
#[test]
fn a_rounded_failure_rate_is_accepted_and_a_transcription_error_is_not() {
    assert_eq!(methods_broken("rate = 0.1434", "rate = 0.143"), Vec::new());
    assert!(
        !methods_broken("rate = 0.1434", "rate = 0.1400").is_empty(),
        "a rate wrong in the third decimal passed"
    );
}

/// The two populations are counted apart and their ids still share one namespace, so an id
/// in both is a collision even though neither count changes.
#[test]
fn one_id_in_both_populations_is_refused_even_though_neither_census_moves() {
    let kinds = kinds_of(
        CONSTRUCTS,
        METHODS,
        &PROTOCOLS.replace(
            "id = \"acquisition.trimmed_recording\"",
            "id = \"onset.threshold.noise_relative\"",
        ),
    );
    assert!(kinds.contains(&ViolationKind::DuplicateId), "{kinds:?}");
}
