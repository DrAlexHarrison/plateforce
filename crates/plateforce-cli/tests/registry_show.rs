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

/// What the registry says about a parameter reaches the terminal, not only the browser.
///
/// 185 of the registry's 241 parameters carry a note, and it holds what the name cannot: which
/// of four studies disagreed about a window width and whether they disagreed about the
/// measurement or the acceptance criterion. The browser has drawn these since it had a drawer.
/// The terminal printed the name alone, so a reader comparing the two surfaces on one entry got
/// two different registries.
///
/// Read out of the registry rather than written here. A literal copied into this file goes
/// stale the first time somebody edits the note, and the assertion then holds the terminal to a
/// sentence the registry has stopped saying.
#[test]
fn what_the_registry_says_about_a_parameter_reaches_the_terminal() {
    let registry =
        plateforce_registry::Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
            .expect("the shipped registry loads");

    // Three entries rather than one: a long note that wraps, a note beside named values, and
    // one whose parameters carry none, so the check is caught both saying and not saying.
    let noted = [CARRIES_BOTH_KINDS, "onset.op.search_upper_bound"];
    let mut sentences_checked = 0;
    for id in noted {
        let entry = registry.methods.get(id).expect("the entry is in the registry");
        let said = String::from_utf8(show(id).stdout).expect("the entry is UTF-8");
        // Whitespace-normalised, because the renderer wraps a note across lines at the
        // terminal's width and a byte comparison would fail on a note that is present.
        let flattened = said.split_whitespace().collect::<Vec<_>>().join(" ");
        for parameter in &entry.parameters {
            let Some(note) = parameter.notes.as_deref().map(str::trim) else {
                continue;
            };
            if note.is_empty() {
                continue;
            }
            let wanted = note.split_whitespace().collect::<Vec<_>>().join(" ");
            sentences_checked += 1;
            assert!(
                flattened.contains(&wanted),
                "{id} says this about {}, and the terminal does not: {wanted:?}",
                parameter.name
            );
        }
    }
    assert!(
        sentences_checked >= 5,
        "checked {sentences_checked} notes, too few to have read the entries"
    );

    // The control, and getting it right took two tries worth recording. It has to declare a
    // parameter: an entry with none never reaches the parameter block at all, and reports every
    // line after the heading as printed under one. And that parameter's own line has to fit in
    // one, because a note and the continuation of a long parameter line are written at the same
    // indent and cannot be told apart afterwards. `spline_order = 3.0, required, published 3.0`
    // fits, says nothing, and names no values, so the line under it belongs to the next field.
    let silent = "drift.aerial_phase_spline.alcantara2019";
    let entry = registry
        .methods
        .get(silent)
        .expect("the entry is in the registry");
    assert!(
        entry.parameters.len() == 1
            && entry.parameters.iter().all(|parameter| {
                parameter.named_values.is_empty()
                    && parameter
                        .notes
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
            }),
        "{silent} has gained a note or a named value, so it can no longer serve as the control"
    );
    let said = String::from_utf8(show(silent).stdout).expect("the entry is UTF-8");
    println!("{said}");
    let after_the_parameter = said
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("parameter"))
        .nth(1)
        .unwrap_or_default();
    assert!(
        after_the_parameter.trim_start().starts_with("citation")
            || after_the_parameter.trim_start().starts_with("bias")
            || after_the_parameter.trim().is_empty(),
        "{silent} says nothing about its one parameter and the terminal wrote \
         {after_the_parameter:?} under it"
    );
}
