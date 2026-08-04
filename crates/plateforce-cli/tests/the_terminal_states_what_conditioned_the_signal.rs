//! The terminal can say what conditioned the signal its numbers were measured on.
//!
//! The phase runs on every analysis, and until this landed only the browser could say
//! anything about it: the terminal, Python and R had no way to name a rule for it or state a
//! value against it, so the record read the software's answer on every run through them. A
//! choice a surface cannot state is a silent default wearing a different costume.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";
const CONSTRUCT: &str = "conditioned_force_signal";
const RULE: &str = "filter.none";
const EDGE: &str = "passband_edge";

fn plateforce(arguments: &[String]) -> Output {
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(borrowed)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// A run that reaches a result, with every choice on the path answered, so anything this file
/// measures is about the conditioning phase rather than about an open decision.
fn analysing(extra: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = [
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
    ]
    .iter()
    .map(|word| word.to_string())
    .collect();
    line.extend(extra.iter().map(|word| word.to_string()));
    line
}

/// The record a run wrote, wherever it wrote it. A result goes to the output stream and a
/// refusal to the error stream, and both are one JSON document on the first line.
fn document(output: &Output) -> serde_json::Value {
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    let first = said
        .lines()
        .find(|line| line.starts_with('{'))
        .unwrap_or_else(|| panic!("the run wrote a document:\n{said}"));
    serde_json::from_str(first).unwrap_or_else(|error| panic!("{error} reading {first}"))
}

/// What the conditioning rule recorded, and where each value it read came from.
fn conditioning_record(extra: &[&str]) -> serde_json::Value {
    let output = plateforce(&analysing(extra));
    let document = document(&output);
    let bound = document["ok"]["bound_methods"]
        .as_array()
        .unwrap_or_else(|| panic!("a result carries its bound methods: {document}"));
    bound
        .iter()
        .find(|method| method["method_id"] == RULE)
        .unwrap_or_else(|| panic!("{RULE} is on the record: {document}"))
        .clone()
}

/// The edge the conditioning rule read reaches `parameter_sources` as the reader's where they
/// stated it and as the rule's where they did not. The two runs produce the same number and
/// differ only in who is recorded as having chosen the signal it was measured on.
#[test]
fn a_stated_conditioning_choice_reaches_the_record_as_the_readers() {
    let unstated = conditioning_record(&[]);
    assert_eq!(
        unstated["parameter_sources"][EDGE], "assumed",
        "an edge nobody was asked about: {unstated}"
    );

    let stated = conditioning_record(&["--choose", "conditioned_force_signal.passband_edge=none"]);
    assert_eq!(
        stated["parameter_sources"][EDGE], "stated",
        "an edge the reader wrote on the line: {stated}"
    );
    assert_eq!(
        stated["unread_parameters"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default(),
        0,
        "the rule read what the line stated: {stated}"
    );
    println!("{EDGE} unstated: assumed, stated: stated");
}

/// Naming the rule the phase runs anyway leaves the same record as leaving it unnamed, so the
/// flag buys a reader the ability to say it and never a different account of what ran.
#[test]
fn naming_the_conditioning_rule_records_what_running_it_unnamed_records() {
    let named = conditioning_record(&["--condition", "conditioned_force_signal=filter.none"]);
    assert_eq!(named, conditioning_record(&[]));
}

/// An edge this rule does not take is refused, in one sentence naming the rule, the name and
/// the value it does take. `filter.none` reports the recording as it was digitised, so a
/// reader who asks it for a 20 Hz passband is asking it for a filter, and answering with
/// `none` would put a word in their record they did not write.
#[test]
fn an_edge_this_rule_does_not_take_is_refused_with_the_one_it_does() {
    let output = plateforce(&analysing(&[
        "--choose",
        "conditioned_force_signal.passband_edge=20",
    ]));
    let refusal = &document(&output)["refusal"];
    assert_eq!(refusal["code"], "value_not_accepted", "{refusal}");
    assert_eq!(refusal["method_id"], RULE, "{refusal}");
    assert_eq!(refusal["parameter"], EDGE, "{refusal}");
    assert_eq!(
        refusal["available"],
        serde_json::json!(["none"]),
        "the refusal names what this rule does take: {refusal}"
    );
    assert!(!output.status.success(), "a refused run reports a failure");
    println!("{}", refusal["message"]);
}

/// A rule this build does not condition with, and a construct it does not condition at all,
/// are different faults listing different alternatives. Either one alone matches no binding,
/// which the phase would otherwise skip in silence.
#[test]
fn a_conditioning_rule_this_build_does_not_run_is_refused_with_the_ones_it_does() {
    let unknown_rule = plateforce(&analysing(&[
        "--condition",
        "conditioned_force_signal=filter.butterworth.single_pass",
    ]));
    let refusal = &document(&unknown_rule)["refusal"];
    assert_eq!(refusal["code"], "method_not_implemented", "{refusal}");
    assert_eq!(refusal["slot"], CONSTRUCT, "{refusal}");
    assert_eq!(refusal["available"], serde_json::json!([RULE]), "{refusal}");

    let unknown_construct = plateforce(&analysing(&["--condition", "movement_onset=filter.none"]));
    let refusal = &document(&unknown_construct)["refusal"];
    assert_eq!(refusal["code"], "method_not_implemented", "{refusal}");
    assert_eq!(
        refusal["available"],
        serde_json::json!([CONSTRUCT]),
        "a construct this phase does not condition names the ones it does: {refusal}"
    );
}

/// One construct given two rules is refused rather than settled by argument order, on the
/// sentence `--set`, `--choose` and `--derive` already refuse a repeated name with.
#[test]
fn a_conditioning_construct_written_twice_is_refused_never_settled_by_position() {
    let output = plateforce(&analysing(&[
        "--condition",
        "conditioned_force_signal=filter.none",
        "--condition",
        "conditioned_force_signal=filter.none",
    ]));
    let said = String::from_utf8_lossy(&output.stderr) + String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains("--condition") && said.contains("takes one value"),
        "two rules for one construct is a line whose meaning depends on order: {said}"
    );
    assert!(!output.status.success());
}

/// `--condition` with the rule left off is a reader who meant to name one, refused as a fault
/// in the line rather than run under whatever the phase would have chosen.
#[test]
fn a_conditioning_construct_given_no_rule_is_refused_rather_than_filled_in() {
    let output = plateforce(&analysing(&["--condition", "conditioned_force_signal="]));
    let said = String::from_utf8_lossy(&output.stderr) + String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains("was given no rule"),
        "an empty rule is a fault in the line: {said}"
    );
    assert!(!output.status.success());
}

/// A folder run states it too, and the folder's record carries the reader's claim on every
/// trial. Both commands read the line through one routine, so a squad's run and a single trace
/// cannot report the same recording as conditioned two ways.
#[test]
fn a_folder_run_states_the_conditioning_choice_on_every_trial() {
    let out = std::env::temp_dir().join(format!(
        "plateforce-conditioning-{}-{}",
        std::process::id(),
        RULE
    ));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("a scratch directory");
    let named = out.display().to_string();

    let mut line: Vec<String> = [
        "--registry",
        "../../registry",
        "batch",
        "../plateforce-conformance/fixtures",
        "--out-dir",
        &named,
        "--trial-suffix",
        ".force.txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--weighing",
        "bwepoch.fixed_window",
        "--onset",
        "onset.threshold.noise_relative",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "weighing.duration=1.0",
        "--set",
        "onset.k=5",
        "--set",
        "takeoff.threshold_n=20",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect();
    line.push("--choose".to_string());
    line.push("conditioned_force_signal.passband_edge=none".to_string());
    plateforce(&line);

    let record = std::fs::read_to_string(out.join("provenance.csv"))
        .expect("a folder run writes its record beside its table");
    let stated = record
        .lines()
        .filter(|row| row.contains(RULE) && row.contains(EDGE))
        .count();
    let assumed = record
        .lines()
        .filter(|row| row.contains(RULE) && row.contains(EDGE) && row.ends_with("assumed"))
        .count();
    println!("{stated} rows name {EDGE} under {RULE}, {assumed} of them as assumed");
    assert!(stated > 0, "the folder record names {EDGE} under {RULE}");
    assert_eq!(
        assumed, 0,
        "the reader stated the edge on the line and the folder record calls it an assumption"
    );
    let _ = std::fs::remove_dir_all(&out);
}
