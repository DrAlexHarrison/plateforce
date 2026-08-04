//! The word `--set` takes, checked against the parser rather than against another string.
//!
//! Help text described one grammar and the parser accepted another, and neither of them was
//! wrong on its own terms: the flags are named after slots, the registry names constructs,
//! and the two words coincide everywhere except `weighing`/`system_weight` and
//! `onset`/`movement_onset`. So the check that holds is the one that runs the parser.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// The one grammar. Every surface that spells it is checked against this literal, and the
/// parser is made to prove it accepts that spelling and refuses the other one.
const SHAPE: &str = "<slot>.<name>=<value>";

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// The `--set` block of a rendered help page. `batch` wraps its options over several lines,
/// so the block runs to the next flag rather than to the next newline.
fn set_help(help: &str) -> String {
    let start = help
        .find("--set ")
        .unwrap_or_else(|| panic!("the help page offers --set:\n{help}"));
    let rest = &help[start..];
    let end = rest[1..]
        .find("\n      --")
        .map(|offset| offset + 1)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

fn analysing(rules: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = [
        "--registry",
        "../../registry",
        "analyse",
        FIXTURE,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--weighing",
        "bwepoch.fixed_window",
        "--set",
        "weighing.duration=1.0",
        "--onset",
        "onset.threshold.noise_relative",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "takeoff.threshold_n=20",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect();
    line.extend(rules.iter().map(|word| word.to_string()));
    line
}

fn run(arguments: &[String]) -> Output {
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    plateforce(&borrowed)
}

#[test]
fn both_commands_show_the_grammar_their_parser_accepts() {
    for command in ["analyse", "batch"] {
        let help = stdout_of(&plateforce(&[command, "--help"]));
        let block = set_help(&help);
        println!("{command}: {}", block.replace('\n', " "));
        assert!(
            block.contains(SHAPE),
            "{command} --help spells --set differently from its parser:\n{block}"
        );
    }
}

/// The three refusals a malformed assignment can reach, each naming the grammar it wanted.
#[test]
fn a_refused_assignment_names_the_same_grammar_the_help_shows() {
    let carries_no_equals = stderr_of(&run(&analysing(&["--set", "onset-k-5"])));
    println!("{}", carries_no_equals.trim());
    assert!(carries_no_equals.contains(SHAPE), "{carries_no_equals}");

    let names_no_slot = stderr_of(&run(&analysing(&["--set", "k=5"])));
    println!("{}", names_no_slot.trim());
    assert!(names_no_slot.contains(SHAPE), "{names_no_slot}");

    // The reader's own words are echoed back rather than a fragment split off them. A run
    // that named the leading word would report `jump_height` for a value written against
    // `jump_height.takeoff_frame`, which is a step this run has.
    let names_a_step_this_run_has_not = stderr_of(&run(&analysing(&["--set", "landing.k=5"])));
    println!("{}", names_a_step_this_run_has_not.trim());
    assert!(
        names_a_step_this_run_has_not.contains("landing.k names no step"),
        "{names_a_step_this_run_has_not}"
    );
    assert!(
        names_a_step_this_run_has_not.contains("weighing, onset, takeoff"),
        "the refusal names what this run does have:\n{names_a_step_this_run_has_not}"
    );
}

/// What makes the grammar a fact rather than a wording preference. `onset` is the word the
/// method flag carries; `movement_onset` is the construct the registry files that rule under.
/// A run that took the construct would accept both, and one that took neither would refuse
/// the reading that works.
#[test]
fn the_word_the_method_flag_carries_is_the_word_the_assignment_takes() {
    let by_slot = run(&analysing(&["--set", "onset.k=5"]));
    println!("--set onset.k=5 exits {:?}", by_slot.status.code());
    assert_eq!(by_slot.status.code(), Some(0), "{}", stderr_of(&by_slot));

    let by_construct = run(&analysing(&["--set", "movement_onset.k=5"]));
    let said = stderr_of(&by_construct);
    println!("--set movement_onset.k=5 says {}", said.trim());
    assert_eq!(by_construct.status.code(), Some(64), "{said}");
    assert!(said.contains("movement_onset.k names no step"), "{said}");
}
