//! A step left open is reported as unnamed, rather than as a rule spelled with no characters.
//!
//! `'' was passed as the takeoff method` tells a caller they passed something they did not
//! pass, which is the interface describing the request the software assembled rather than the
//! one they wrote. Two ways in reach it: a preset whose source states nothing about a step, and
//! a call that names no rules at all.
//!
//! The terminal and the browser each escape it by enumerating the open choices before a request
//! is built, so the sentence is met by the surfaces that do not: a notebook and an R session
//! reach the engine directly.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn analysing(extra: &[&str]) -> String {
    let mut line = vec![
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
    ];
    line.extend_from_slice(extra);
    let output = plateforce(&line);
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .split_whitespace()
    .collect::<Vec<&str>>()
    .join(" ")
}

/// `owen2014` states nothing about takeoff, which the registry records, so a reader who picks
/// it has left a step open rather than named a rule that answers to nothing.
#[test]
fn a_preset_that_leaves_a_step_open_says_the_step_is_unnamed() {
    let said = analysing(&["--preset", "owen2014"]);

    assert!(
        said.contains("no rule is named for the takeoff step"),
        "the step is reported as unnamed: {said}"
    );
    assert!(
        !said.contains("'' was passed"),
        "nobody passed an empty rule: {said}"
    );
}

/// The rules for the open step are still named, because a reader who has just been told a step
/// is open needs the same list they would get any other way.
#[test]
fn the_rules_for_the_open_step_are_still_named() {
    let said = analysing(&["--preset", "owen2014"]);

    assert!(
        said.contains("takeoff.threshold.absolute_force"),
        "the rules for the open step are listed: {said}"
    );
}

/// A rule that genuinely resolves to nothing is a different fault and keeps its own sentence,
/// so the guard reads the unstated case rather than swallowing the mistyped one.
#[test]
fn a_rule_that_answers_to_nothing_still_names_what_was_passed() {
    let said = analysing(&[
        "--weighing",
        "bwepoch.fixed_window",
        "--onset",
        "onset.nonsense",
        "--takeoff",
        "takeoff.threshold.absolute_force",
    ]);

    assert!(
        said.contains("'onset.nonsense' was passed as the movement_onset method"),
        "what the caller actually typed is quoted back: {said}"
    );
}
