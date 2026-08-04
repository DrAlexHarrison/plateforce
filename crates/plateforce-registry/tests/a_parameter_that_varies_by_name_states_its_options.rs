//! A parameter the literature varies by name carries its options as data, so a surface can
//! offer them and a result can record which one was bound.

use plateforce_registry::{assemble, ViolationKind};

const CONSTRUCTS: &str = r#"
[[construct]]
id = "braking_phase_start"
title = "Braking phase start"
unit = "seconds"
"#;

/// The shape the ten regression coefficient sets need: one option, several numbers, a
/// different unit on each.
const METHOD: &str = r#"
[[method]]
id = "phase.braking_start.zero_net_force"
construct = "braking_phase_start"
title = "The instant net force returns through zero"
rule = "Braking begins where the search signal returns through zero."
status = "accepted"
confidence = "high"

[[method.parameter]]
name = "search_signal"
unit = "enumeration"
default_key = "velocity_argmin"
default_source = "mcmahon2018"

[[method.parameter.value]]
key = "velocity_argmin"
label = "Minimum centre of mass velocity"

[[method.parameter.value]]
key = "force_bw_crossing"
label = "Force returning through system weight"

[[method.parameter.value.number]]
name = "jump_height_coefficient"
value = 61.9
unit = "watts_per_centimetre"

[[method.parameter.value.number]]
name = "body_mass_coefficient"
value = 36.0
unit = "watts_per_kilogram"

[[method.citation]]
key = "mcmahon2018"
role = "proposes"
reference = "McMahon, Suchomel, Lake and Comfort 2018, Strength Cond J 40(4):96-106"
obtained = true
"#;

fn violations_of(method: &str) -> Vec<String> {
    let assembled = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/phase.toml", method),
    ])
    .expect("the files describe one registry");
    assembled
        .violations
        .iter()
        .map(|violation| violation.to_string())
        .collect()
}

fn kinds_of(method: &str) -> Vec<ViolationKind> {
    let assembled = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/phase.toml", method),
    ])
    .expect("the files describe one registry");
    assembled
        .violations
        .into_iter()
        .map(|violation| violation.kind)
        .collect()
}

/// The control, and it carries the coefficient-set shape as well as the plain enumeration,
/// so every assertion below is against a registry that is otherwise sound.
#[test]
fn a_parameter_naming_its_options_and_defaulting_to_one_of_them_raises_nothing() {
    assert_eq!(violations_of(METHOD), Vec::<String>::new());
}

/// The numbers survive the round trip with their own units, which is the whole reason a set
/// of regression coefficients cannot live in `published_values`.
#[test]
fn one_option_carries_several_numbers_each_in_its_own_unit() {
    let assembled = assemble([
        ("constructs.toml", CONSTRUCTS),
        ("methods/phase.toml", METHOD),
    ])
    .expect("the files describe one registry");
    let method = &assembled.registry.methods["phase.braking_start.zero_net_force"];
    let parameter = &method.parameters[0];
    assert_eq!(parameter.named_values.len(), 2);
    assert_eq!(parameter.default_key.as_deref(), Some("velocity_argmin"));

    let numbers = &parameter.named_values[1].numbers;
    assert_eq!(numbers.len(), 2);
    assert_eq!(numbers[0].value, 61.9);
    assert_eq!(numbers[0].unit, "watts_per_centimetre");
    assert_eq!(numbers[1].unit, "watts_per_kilogram");
}

#[test]
fn a_default_naming_an_option_the_parameter_does_not_list_is_refused() {
    let kinds = kinds_of(&METHOD.replace(
        "default_key = \"velocity_argmin\"",
        "default_key = \"acceleration_argmin\"",
    ));
    assert!(
        kinds.contains(&ViolationKind::DefaultNamesUnknownValue {
            parameter: "search_signal".to_string(),
            key: "acceleration_argmin".to_string(),
        }),
        "{kinds:?}"
    );
}

/// Two defaults are two claims about the one value the software binds, and a reader has no
/// way to tell which the code took.
#[test]
fn a_parameter_declaring_a_default_twice_is_refused() {
    let kinds = kinds_of(&METHOD.replace(
        "default_key = \"velocity_argmin\"",
        "default_key = \"velocity_argmin\"\ndefault = 5.0",
    ));
    assert!(
        kinds.contains(&ViolationKind::DefaultDeclaredTwice {
            parameter: "search_signal".to_string(),
        }),
        "{kinds:?}"
    );
}

#[test]
fn a_named_default_with_nobody_named_as_having_chosen_it_is_refused() {
    let kinds = kinds_of(&METHOD.replace("default_source = \"mcmahon2018\"\n", ""));
    assert!(
        kinds.contains(&ViolationKind::DefaultWithoutSource {
            parameter: "search_signal".to_string(),
        }),
        "{kinds:?}"
    );
}

/// Options are keyed, so a repeated key is one option overwriting another wherever a surface
/// builds a map of them.
#[test]
fn a_parameter_listing_one_option_twice_is_refused() {
    let kinds =
        kinds_of(&METHOD.replace("key = \"force_bw_crossing\"", "key = \"velocity_argmin\""));
    assert!(
        kinds.contains(&ViolationKind::NamedValueDeclaredTwice {
            parameter: "search_signal".to_string(),
            key: "velocity_argmin".to_string(),
        }),
        "{kinds:?}"
    );
}

/// A number without its unit is the unit confusion the registry records instances of, so it
/// is refused before validation rather than reported by it.
#[test]
fn an_option_stating_a_number_without_its_unit_does_not_parse() {
    let Err(error) = assemble([
        ("constructs.toml", CONSTRUCTS),
        (
            "methods/phase.toml",
            &METHOD.replace("unit = \"watts_per_centimetre\"\n", ""),
        ),
    ]) else {
        panic!("a coefficient with no unit assembled into a registry");
    };
    assert!(error.to_string().contains("unit"), "{error}");
}
