//! Asking the terminal for a number computed from the landmarks.
//!
//! `--derive <construct>=<method>` is how a construct other than the three the path walks is
//! named, and `--set <construct>.<name>=<value>` reaches its parameters under the same
//! spelling. The manifest lists every rule this build runs, so a rule the engine dispatches
//! and the command line cannot reach would be a claim this surface could not honour.

use std::process::Output;

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn analyse(extra: &[&str]) -> Output {
    let mut arguments: Vec<&str> = vec![
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
    ];
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn document(output: &Output) -> serde_json::Value {
    let text = String::from_utf8(output.stdout.clone()).expect("the document is UTF-8");
    serde_json::from_str(&text).expect("the document parses")
}

fn metric<'a>(body: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    body["ok"]["metrics"]
        .as_array()?
        .iter()
        .find(|metric| metric["key"] == key)
}

#[test]
fn a_rule_named_on_the_command_line_reports_its_number_and_the_rule_behind_it() {
    let output = analyse(&[
        "--derive",
        "analysis_window=window_end.takeoff.detected",
        "--derive",
        "peak_force=force.peak.gross",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = document(&output);
    let peak = metric(&body, "peak_force_newtons").expect("the peak was reported");
    println!("{peak}");
    assert_eq!(peak["computed_by"], "force.peak.gross");
    assert!(peak["value"].as_f64().expect("a number") > 0.0);
    assert_eq!(peak["unit"], "newtons");
    // The chain names the rule that placed the window it read, so a reader can see which
    // stretch of the recording the maximum was taken over.
    assert!(
        peak["contributing_method_ids"]
            .as_array()
            .expect("a chain")
            .iter()
            .any(|id| id == "window_end.takeoff.detected"),
        "{peak}"
    );
}

/// The parameter reaches the rule under the construct's own name, and moves the number.
#[test]
fn a_value_set_against_a_derived_construct_reaches_its_rule() {
    let taken_raw = analyse(&[
        "--derive",
        "analysis_window=window_end.takeoff.detected",
        "--derive",
        "peak_force=force.peak.estimator",
    ]);
    let smoothed = analyse(&[
        "--derive",
        "analysis_window=window_end.takeoff.detected",
        "--derive",
        "peak_force=force.peak.estimator",
        "--set",
        "peak_force.averaging_window_seconds=0.1",
    ]);
    assert_eq!(taken_raw.status.code(), Some(0));
    assert_eq!(smoothed.status.code(), Some(0));

    let raw = metric(&document(&taken_raw), "peak_force_newtons").unwrap()["value"]
        .as_f64()
        .unwrap();
    let averaged = metric(&document(&smoothed), "peak_force_newtons").unwrap()["value"]
        .as_f64()
        .unwrap();
    println!("raw {raw:.1} N, 0.1 s averaged {averaged:.1} N");
    assert!(
        averaged < raw,
        "the width the line stated did not reach the rule: {raw} against {averaged}"
    );
}

/// A construct with no rule behind it is refused by name, listing what this build does run.
#[test]
fn a_construct_this_build_runs_no_rule_for_is_refused_by_name() {
    let output = analyse(&["--derive", "mechanical_power=power.peak.instantaneous"]);
    let text = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);
    println!("{text}");
    assert_ne!(output.status.code(), Some(0));
    assert!(text.contains("mechanical_power"), "{text}");
    assert!(text.contains("peak_force"), "{text}");
}

/// An id that is a rule filed under a different construct is refused too, rather than
/// matching nothing and leaving the request short of a number it asked for.
#[test]
fn an_id_filed_under_another_construct_is_refused_by_name() {
    let output = analyse(&["--derive", "peak_force=window_end.takeoff.detected"]);
    let text = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);
    println!("{text}");
    assert_ne!(output.status.code(), Some(0));
    assert!(text.contains("window_end.takeoff.detected"), "{text}");
    assert!(text.contains("force.peak.gross"), "{text}");
}

/// A peak asked for with no window chosen names the choice that is open. The number would
/// otherwise be taken over a window nobody picked, which is the silent default this registry
/// exists to record.
#[test]
fn a_peak_with_no_window_chosen_names_the_open_choice() {
    let output = analyse(&["--derive", "peak_force=force.peak.gross"]);
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    println!("{text}");
    assert!(text.contains("analysis_window"), "{text}");
    assert!(text.contains("decision_not_made"), "{text}");
    // The peak itself is absent rather than present with a value taken over a window nobody
    // chose, so nothing downstream can read one.
    assert!(!text.contains("\"peak_force_newtons\""), "{text}");
}
