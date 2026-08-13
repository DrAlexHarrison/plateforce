//! What the terminal does when a choice on the path has no defensible default.
//!
//! A number produced without the operator naming the rule behind it carries no record of what
//! produced it. So the run declines, names every open choice in the field's own words, and
//! prints what can be passed instead.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn plateforce(arguments: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn reading(rules: &[&str]) -> Vec<String> {
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
    ]
    .iter()
    .map(|word| word.to_string())
    .collect();
    line.extend(rules.iter().map(|word| word.to_string()));
    line
}

const EVERY_RULE_NAMED: [&str; 12] = [
    "--weighing",
    "bwepoch.fixed_window",
    "--set",
    "weighing.duration=1.0",
    "--onset",
    "onset.threshold.noise_relative",
    "--set",
    "onset.k=5",
    "--takeoff",
    "takeoff.threshold.absolute_force",
    "--set",
    "takeoff.threshold_n=20",
];

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn no_rule_named_produces_no_number_and_a_missing_argument_code() {
    let output = plateforce(&reading(&[]));
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(64), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "no number leaves the tool on a declined run"
    );
    assert!(
        stderr.contains("Standing still, before the jump"),
        "{stderr}"
    );
    assert!(stderr.contains("Start of the jump"), "{stderr}");
    assert!(
        stderr.contains("onset.threshold.noise_relative"),
        "{stderr}"
    );
    assert!(stderr.contains("published at"), "{stderr}");
}

/// Onset threshold and takeoff threshold are both a threshold in newtons and they must not
/// have the same treatment. One moves net impulse reliability from 0.984 to 0.479 on
/// identical data; the other carries no rule that forces a decision, so it is never named as
/// an open choice. A terminal that listed it would be flattening the tiering back out.
#[test]
fn takeoff_is_not_listed_because_no_takeoff_rule_forces_a_decision() {
    let stderr = stderr_of(&plateforce(&reading(&[])));
    let mentions = stderr.matches("takeoff").count();
    println!("takeoff named {mentions} times in the refusal");
    assert_eq!(mentions, 0, "{stderr}");
}

/// The choice is per construct. Six rules force on the weighing epoch and three on onset,
/// and that is two decisions rather than nine.
#[test]
fn the_count_is_taken_over_the_constructs_on_the_path() {
    let stderr = stderr_of(&plateforce(&reading(&[])));
    assert!(
        stderr.contains("2 of 3 choices on the path"),
        "the count carries the denominator it was taken over: {stderr}"
    );
}

/// A rule the registry documents and this build cannot run is never offered as something to
/// pass, and asking for it by name is refused rather than served by the nearest rule, which
/// would carry a published author's citation onto a number their method did not produce.
#[test]
fn a_documented_rule_with_no_code_is_refused_and_never_offered() {
    let offered = stderr_of(&plateforce(&reading(&[])));
    assert!(
        !offered.contains("onset.manual_visual.tillin2010"),
        "{offered}"
    );

    let mut rules = EVERY_RULE_NAMED.to_vec();
    rules[5] = "onset.manual_visual.tillin2010";
    let asked = plateforce(&reading(&rules));
    let stderr = stderr_of(&asked);
    assert_eq!(asked.status.code(), Some(64), "{stderr}");
    assert!(
        stderr.contains("onset.manual_visual.tillin2010"),
        "{stderr}"
    );
}

/// A rule requiring a value the literature publishes several ways, on a construct that is
/// already contested, does not run on whichever value the code happens to hold.
#[test]
fn a_published_disagreement_over_a_value_is_a_choice_too() {
    let mut rules: Vec<&str> = EVERY_RULE_NAMED.to_vec();
    let assignment = rules
        .iter()
        .position(|word| *word == "onset.k=5")
        .expect("the line states k");
    rules.drain(assignment - 1..=assignment);

    let output = plateforce(&reading(&rules));
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(64), "{stderr}");
    assert!(stderr.contains("--set onset.k="), "{stderr}");
    assert!(stderr.contains("published at"), "{stderr}");
}

/// A value written against a step this run does not have is refused rather than accepted and
/// passed to nothing.
#[test]
fn a_value_for_a_step_that_is_not_on_the_path_is_refused() {
    let mut rules: Vec<&str> = EVERY_RULE_NAMED.to_vec();
    rules.push("--set");
    rules.push("landing.threshold_n=20");
    let output = plateforce(&reading(&rules));
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(64), "{stderr}");
    assert!(stderr.contains("landing"), "{stderr}");
}

/// A value a rule publishes several ways and does not require is a value the rule chooses,
/// not a choice the literature forces on the operator. Measured on the shipped registry, 7 of
/// the 19 parameters on executable bindings carry more than one published value on a
/// construct that forces, and exactly 1 of those 7 is not required:
/// `bwepoch.manual_placement.span_seconds`. It is the one entry that separates the rule the
/// browser ships from a looser reading of it.
#[test]
fn a_value_the_rule_does_not_require_is_not_an_open_choice() {
    let mut rules: Vec<&str> = EVERY_RULE_NAMED.to_vec();
    rules[1] = "bwepoch.manual_placement";
    let stderr = stderr_of(&plateforce(&reading(&rules)));
    assert!(
        !stderr.contains("weighing.span_seconds"),
        "the span is the rule's to choose: {stderr}"
    );
}

#[test]
fn every_rule_named_produces_every_number_and_exits_zero() {
    let output = plateforce(&reading(&EVERY_RULE_NAMED));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    // The rows carrying a number, which each carry the rule behind it on the line beneath.
    // Counting every line of the block instead reads the rule a row names as a second row.
    //
    // Found by stepping over the trial block, which names the recording above the numbers and
    // is indented the same way. Counting from the top of the document counts that block.
    let rows = stdout
        .lines()
        .skip_while(|line| !line.is_empty())
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
        .count();
    println!("metric rows: {rows} of 11");
    assert_eq!(rows, 11, "{stdout}");
}

/// A refusal a reader cannot read has not refused, and eighty columns is the floor every
/// common terminal reaches.
#[test]
fn the_refusal_reads_at_eighty_columns_without_splitting_an_id() {
    let stderr = stderr_of(&plateforce(&reading(&[])));
    for line in stderr.lines() {
        assert!(
            line.chars().count() <= 80,
            "{} columns: {line}",
            line.chars().count()
        );
    }
    assert!(stderr.contains("onset.threshold.relative_to_system_weight"));
}
