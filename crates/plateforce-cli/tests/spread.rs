//! Sweeping one quantity over every rule this build runs for each step on its path.
//!
//! `analyse` reports this for the jump height without being asked. This command reports it
//! for any other quantity, and it answers the same open choices first, so a sweep is never a
//! way to get a number the analysis itself would have refused to give.

use std::process::Output;

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn spread(extra: &[&str], rules_named: bool) -> Output {
    let mut arguments: Vec<&str> = vec![
        "--registry",
        "../../registry",
        "spread",
        FIXTURE,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
    ];
    if rules_named {
        arguments.extend([
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
        ]);
    }
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn body(output: &Output) -> serde_json::Value {
    let text = String::from_utf8(output.stdout.clone()).expect("the document is UTF-8");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
    parsed["ok"].clone()
}

/// Every figure carries what it was taken over, so a percentage is never read without the
/// count behind it.
#[test]
fn the_sweep_reports_what_ran_and_what_did_not() {
    let output = spread(&["--format", "json"], true);
    assert_eq!(output.status.code(), Some(0));
    let response = body(&output);

    let run = response["combinations_run"].as_u64().expect("a count");
    let succeeded = response["succeeded"].as_u64().expect("a count");
    let failed = response["failed"].as_u64().expect("a count");
    println!("succeeded {succeeded}, failed {failed}, of combinations_run {run}");
    assert!(run > 1, "a sweep over one combination is not a sweep");
    assert_eq!(succeeded + failed, run, "every combination is accounted for");
}

/// A quantity other than the one `analyse` headlines, which is the reason this command
/// exists rather than a flag on that one.
#[test]
fn a_quantity_the_analysis_does_not_headline_can_be_swept() {
    let output = spread(&["--format", "json", "--quantity", "time_to_takeoff_seconds"], true);
    assert_eq!(output.status.code(), Some(0));
    let response = body(&output);
    assert_eq!(response["quantity_key"], "time_to_takeoff_seconds");
    assert_eq!(response["unit_symbol"], "s");

    let headline = body(&spread(&["--format", "json"], true));
    assert_eq!(headline["quantity_key"], "jump_height_from_takeoff_meters");
}

/// The same rail `analyse` meets, from the same code, rather than a second one that could
/// answer differently.
#[test]
fn an_unanswered_choice_refuses_here_exactly_as_it_does_in_the_analysis() {
    let output = spread(&[], false);
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(said.contains("have no default"), "{said}");
    assert!(said.contains("--onset"), "{said}");
}

/// A quantity nothing computes is refused rather than swept into an empty answer.
#[test]
fn a_quantity_this_build_does_not_compute_is_refused() {
    let output = spread(&["--quantity", "nothing.computes.this"], true);
    println!("exit {:?}", output.status.code());
    assert_ne!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty(), "no number is published");
}
