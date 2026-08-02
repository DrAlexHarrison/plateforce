//! What a script reads when a run declines, as against what a person reads.
//!
//! A refusal is the product here, so the pipe carries the record every surface publishes: the
//! code, the rule, the parameter and the value, rather than a sentence to be parsed back
//! apart. The screen carries the layout, which for an unmade choice is the list of rules that
//! would answer it.

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

fn reading(format: &str, rules: &[&str]) -> Vec<String> {
    let mut line: Vec<&str> = vec![
        "--registry",
        "../../registry",
        "--format",
        format,
        "analyse",
        FIXTURE,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
    ];
    line.extend_from_slice(rules);
    line.into_iter().map(str::to_string).collect()
}

fn run(format: &str, rules: &[&str]) -> Output {
    let line = reading(format, rules);
    plateforce(&line.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Every rule named and every published value stated, so the run reaches the engine.
const EVERY_CHOICE_ANSWERED: [&str; 12] = [
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

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn a_run_that_computed_nothing_hands_a_pipe_the_record_and_not_a_sentence() {
    let output = run("json", &[]);
    assert_eq!(output.status.code(), Some(64), "{}", stderr_of(&output));
    assert!(
        output.stdout.is_empty(),
        "stdout carries a document or it is empty, and this run produced no number"
    );

    let envelope: serde_json::Value =
        serde_json::from_str(&stderr_of(&output)).expect("a declined run writes one document");
    let refusal = &envelope["refusal"];
    assert_eq!(refusal["code"], "decision_not_made");
    let outstanding: Vec<&str> = refusal["available"]
        .as_array()
        .expect("the constructs still open")
        .iter()
        .map(|value| value.as_str().expect("a construct id"))
        .collect();
    println!(
        "constructs still open: {} of 3 on the path",
        outstanding.len()
    );
    assert_eq!(outstanding, ["system_weight", "movement_onset"]);
}

/// The same refusal, and the two renderings differ in the layout rather than in the fact.
#[test]
fn the_screen_takes_the_candidates_and_the_pipe_takes_the_code() {
    let shown = stderr_of(&run("text", &[]));
    assert!(
        shown.contains("onset.threshold.noise_relative"),
        "a person is shown what to pass: {shown}"
    );
    assert!(
        !shown.contains("\"code\""),
        "a terminal is not handed a document: {shown}"
    );

    let piped = stderr_of(&run("json", &[]));
    assert!(
        !piped.contains("--onset <METHOD>"),
        "a pipe is not handed a layout: {piped}"
    );
    assert!(piped.contains("\"decision_not_made\""), "{piped}");
}

/// A landmark that declined while the other numbers computed. The record rides in the
/// result, because a reader who redirected the numbers to a file must not lose the reason
/// one of them is absent.
#[test]
fn a_landmark_that_declined_carries_its_code_beside_the_numbers_that_did_not() {
    let mut rules: Vec<&str> = EVERY_CHOICE_ANSWERED.to_vec();
    // The threshold scales with the noise, so a k this large puts it below anything the
    // trace reaches and the rule has no band left to search.
    rules[7] = "onset.k=5000";
    let output = run("json", &rules);
    assert_eq!(output.status.code(), Some(65), "{}", stderr_of(&output));

    let document: serde_json::Value =
        serde_json::from_str(&stdout_of(&output)).expect("a partial run writes a whole document");
    let refusals = document["ok"]["refusals"]
        .as_array()
        .expect("the refusals ride in the result");
    println!("refusals carried in the result: {}", refusals.len());
    assert_eq!(refusals.len(), 1);

    let refusal = &refusals[0];
    assert_eq!(refusal["code"], "collapsed_band");
    assert_eq!(refusal["method_id"], "onset.threshold.noise_relative");
    assert_eq!(refusal["parameter"], "k");
    assert_eq!(refusal["value"], 5000.0);
    assert!(
        refusal["detail"]["dispersion_newtons"].is_number(),
        "the numbers the rule read while declining: {refusal}"
    );
}

/// `onset` is this command's flag and `movement_onset` is what the registry declares. A
/// record handing back the first gives a caller a word it cannot look up.
#[test]
fn a_record_names_the_construct_the_registry_declares_and_not_the_flag() {
    let mut rules: Vec<&str> = EVERY_CHOICE_ANSWERED.to_vec();
    rules[7] = "onset.k=5000";
    let output = run("json", &rules);
    let document: serde_json::Value =
        serde_json::from_str(&stdout_of(&output)).expect("a partial run writes a whole document");
    let named = document["ok"]["refusals"][0]["slot"]
        .as_str()
        .expect("the construct the refusal happened under")
        .to_string();

    // Asked of the registry rather than compared against a word written here, so the test
    // fails when the record stops resolving rather than when the spelling changes.
    let entry = plateforce(&[
        "--registry",
        "../../registry",
        "--format",
        "json",
        "registry",
        "show",
        "onset.threshold.noise_relative",
    ]);
    assert!(entry.status.success(), "{}", stderr_of(&entry));
    let filed: serde_json::Value =
        serde_json::from_str(&stdout_of(&entry)).expect("the registry answers with a document");
    assert_eq!(named, filed["ok"]["method"]["construct"]);
    println!("the construct the record names, as the registry files it: {named}");

    let flags = stdout_of(&plateforce(&["analyse", "--help"]));
    assert!(
        flags.contains("--onset"),
        "the flag this run was written with: {flags}"
    );
}

/// A rule the build cannot run is a request asking for something not on offer, and the record
/// names both the id that was passed and the ids that would have worked.
#[test]
fn an_id_with_no_rule_behind_it_publishes_the_code_and_what_would_have_run() {
    let output = run("json", &["--onset", "onset.threshold.invented"]);
    assert_eq!(output.status.code(), Some(64), "{}", stderr_of(&output));
    let envelope: serde_json::Value =
        serde_json::from_str(&stderr_of(&output)).expect("a declined run writes one document");
    let refusal = &envelope["refusal"];
    assert_eq!(refusal["code"], "method_not_implemented");
    assert_eq!(refusal["method_id"], "onset.threshold.invented");
    assert_eq!(refusal["slot"], "movement_onset");
    let available = refusal["available"]
        .as_array()
        .expect("what would have run");
    println!("rules offered for the step: {}", available.len());
    assert!(available
        .iter()
        .any(|id| id == "onset.threshold.noise_relative"));
}

/// A registry that does not load is a published code, and a directory this command was never
/// given is a fault in the line. The first is a code every surface can raise and the second
/// is one no other surface has, so the record says so rather than inventing one.
#[test]
fn a_fault_in_the_line_publishes_no_code_and_a_refused_rule_publishes_one() {
    let missing = plateforce(&[
        "--registry",
        "no/such/registry",
        "--format",
        "json",
        "registry",
        "census",
    ]);
    assert_eq!(missing.status.code(), Some(78), "{}", stderr_of(&missing));
    let envelope: serde_json::Value =
        serde_json::from_str(&stderr_of(&missing)).expect("a declined run writes one document");
    assert_eq!(envelope["refusal"]["code"], "registry_invalid");

    let malformed = plateforce(&[
        "--registry",
        "../../registry",
        "--format",
        "json",
        "analyse",
        FIXTURE,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--set",
        "onset-k-5",
    ]);
    assert_eq!(
        malformed.status.code(),
        Some(64),
        "{}",
        stderr_of(&malformed)
    );
    let envelope: serde_json::Value =
        serde_json::from_str(&stderr_of(&malformed)).expect("a declined run writes one document");
    assert!(
        envelope["refusal"]["code"].is_null(),
        "the shape of an assignment reached no rule: {envelope}"
    );
    assert!(envelope["refusal"]["message"]
        .as_str()
        .expect("the sentence")
        .contains("carries no ="));
}
