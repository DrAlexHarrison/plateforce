//! What an entry says about the choices it leaves to the reader.
//!
//! A parameter the rule will answer for itself and one the reader must answer read the same
//! as a bare name, and the second is the larger half of this registry. A forced decision
//! rendered as an optional one is passed over, and the number then rests on a default nobody
//! stated.

use std::process::Output;

fn show(id: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["--registry", "../../registry", "registry", "show", id])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn parameter_lines(id: &str) -> Vec<String> {
    let output = show(id);
    assert_eq!(output.status.code(), Some(0), "{id} resolves");
    String::from_utf8(output.stdout)
        .expect("the entry is UTF-8")
        .lines()
        .filter(|line| line.trim_start().starts_with("parameter"))
        .map(str::to_string)
        .collect()
}

/// The rule the decision rail already names, which carries one of each kind.
const CARRIES_BOTH_KINDS: &str = "bwepoch.fixed_window";

#[test]
fn a_parameter_the_reader_must_answer_says_so() {
    let lines = parameter_lines(CARRIES_BOTH_KINDS);
    for line in &lines {
        println!("{line}");
    }

    let required: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("required"))
        .collect();
    let optional: Vec<&String> = lines
        .iter()
        .filter(|line| !line.contains("required"))
        .collect();

    // The control. A registry entry carrying only one kind would satisfy either assertion
    // below on its own, and say nothing about whether the two are told apart.
    assert!(
        !required.is_empty() && !optional.is_empty(),
        "this entry carries only one kind, so it cannot show that the two differ: {lines:?}"
    );
}

/// The sharper half: required, and nothing behind it. The reader has to state a value and
/// no default will arrive.
#[test]
fn a_required_parameter_with_no_default_shows_neither_a_value_nor_silence() {
    let lines = parameter_lines("cutoff.residual_analysis.winter");
    let line = lines
        .iter()
        .find(|line| line.contains("noise_dominated_from_hz"))
        .expect("the entry names the parameter");
    println!("{line}");
    assert!(line.contains("required"), "{line}");
    assert!(
        !line.contains(" = "),
        "nothing supplies it, so nothing is shown supplying it: {line}"
    );
}

/// A default is still shown beside the requirement, because a rule that needs a value and
/// has one is a different situation from a rule that needs one and does not.
#[test]
fn a_required_parameter_with_a_default_shows_both() {
    let lines = parameter_lines(CARRIES_BOTH_KINDS);
    let line = lines
        .iter()
        .find(|line| line.contains("duration"))
        .expect("the entry names the parameter");
    println!("{line}");
    assert!(line.contains(" = 1.0"), "{line}");
    assert!(line.contains("required"), "{line}");
    assert!(line.contains("published"), "{line}");
}

/// An entry with nothing to say about a parameter says nothing, rather than saying that it
/// has nothing to say.
#[test]
fn an_optional_parameter_carries_no_statement_about_its_absence() {
    let lines = parameter_lines(CARRIES_BOTH_KINDS);
    let line = lines
        .iter()
        .find(|line| line.contains("anchor"))
        .expect("the entry names the parameter");
    println!("{line}");
    assert!(!line.contains("required"), "{line}");
    assert!(!line.contains("optional"), "{line}");
    assert!(!line.contains("not "), "{line}");
}

/// A parameter whose options are named rather than numbered, which the registry began
/// carrying after this renderer learned to read both shapes.
///
/// The branch was written before any entry exercised it, and reported as untested rather
/// than claimed as covered. This is the entry that exercises it.
#[test]
fn a_default_chosen_by_name_is_shown_by_name() {
    // The whole entry rather than the parameter lines alone: a long parameter wraps, and
    // the continuation carries no "parameter" label to filter on.
    let output = show("phase.braking_start.zero_net_force");
    let said = String::from_utf8(output.stdout).expect("the entry is UTF-8");
    println!("{said}");
    assert!(said.contains("search_signal = velocity_argmin"), "{said}");
    // Both options, so the reader sees what they may choose instead of the default.
    assert!(said.contains("force_bw_crossing"), "{said}");
}
