//! A folder run under one request.
//!
//! The founding measurement behind this project varies an onset rule and its `k` and the
//! weighing start, so a run that cannot state those is a run that cannot reproduce the
//! thing the registry exists to record. A folder is also the run a class or a squad
//! actually does, which is why the flag matters more here than on one trial.

use std::process::Output;

fn batch(out_dir: &std::path::Path, extra: &[&str]) -> Output {
    let named = out_dir.display().to_string();
    let mut arguments: Vec<&str> = vec![
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
    ];
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn sources(out_dir: &std::path::Path) -> std::collections::BTreeMap<String, usize> {
    let text = std::fs::read_to_string(out_dir.join("provenance.csv")).expect("a record");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let column = header
        .iter()
        .position(|name| *name == "source")
        .expect("the record says where each value came from");
    let mut counted = std::collections::BTreeMap::new();
    for line in lines {
        let fields: Vec<&str> = line.split(',').collect();
        if let Some(source) = fields.get(column) {
            *counted.entry((*source).to_string()).or_insert(0) += 1;
        }
    }
    counted
}

/// What a run over this folder exits with, and why it is not zero.
///
/// Five of the six subject-01 trials never return above the takeoff threshold, so no touchdown
/// is placed on them, so `jumpheight.takeoff.flight_time` has no interval to work on and
/// declines by name. A batch holding a trial whose requested headline number could not be
/// produced is not a clean run, and the exit code is where a reader learns that without
/// reading the record.
///
/// It used to be zero, and the height came back empty with nothing anywhere saying why.
const A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER: i32 = 65;

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("plateforce-batch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

/// Without the flag every value is the rule's own, and the record says so honestly. The
/// defect was that there was no way to make it say anything else.
#[test]
fn a_value_stated_for_a_folder_is_recorded_as_stated() {
    let without = scratch("plain");
    assert_eq!(
        batch(&without, &[]).status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER)
    );
    let before = sources(&without);

    let with = scratch("stated");
    let output = batch(
        &with,
        &[
            "--set",
            "weighing.duration=1.0",
            "--set",
            "onset.k=5",
            "--set",
            "takeoff.threshold_n=20",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER)
    );
    let after = sources(&with);

    println!("without --set {before:?}");
    println!("with --set    {after:?}");
    assert_eq!(
        before.get("stated"),
        None,
        "nothing was stated, so nothing reads stated"
    );
    assert!(
        after.get("stated").copied().unwrap_or(0) > 0,
        "a value the operator stated reads stated"
    );
    assert!(
        after.get("assumed").copied().unwrap_or(0) < before.get("assumed").copied().unwrap_or(0),
        "and it is no longer counted as the rule's own"
    );

    let _ = std::fs::remove_dir_all(&without);
    let _ = std::fs::remove_dir_all(&with);
}

/// The same spelling `analyse` takes, so a reader who wrote `--set onset.k` on one trial
/// writes it on a folder.
#[test]
fn an_assignment_this_command_cannot_read_is_refused_before_a_trial_is() {
    let out = scratch("refused");
    let output = batch(&out, &["--set", "onset-k-5"]);
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(said.contains("--set takes"), "{said}");
    assert!(
        !out.join("results.csv").exists(),
        "no trial was read before the refusal"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// A marker read as force moves system weight, and system weight carries into every
/// impulse, velocity and height a run reports. There is no default because there is no
/// answer the software could pick that is not a guess about somebody else's export.
#[test]
fn a_folder_run_cannot_proceed_without_saying_how_a_missing_sample_is_written() {
    let out = scratch("nosentinel");
    let named = out.display().to_string();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
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
            "--weighing",
            "bwepoch.fixed_window",
            "--onset",
            "onset.threshold.noise_relative",
            "--takeoff",
            "takeoff.threshold.absolute_force",
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(said.contains("--sentinel"), "{said}");
    assert!(
        !out.join("results.csv").exists(),
        "no trial was read before the refusal"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// The record says how the folder was read, so two runs that read one folder differently
/// are two runs rather than one with a number that moved.
#[test]
fn the_record_names_the_convention_the_run_applied() {
    let out = scratch("recorded");
    assert_eq!(
        batch(&out, &[]).status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER)
    );
    let run: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("run.json")).expect("a record"))
            .expect("the record parses");
    for named in [
        "sentinel",
        "delimiter",
        "force_column_index",
        "sample_rate_hz",
    ] {
        assert!(
            run.get(named).is_some(),
            "the record carries {named}: {run}"
        );
    }
    println!("the run records {:?}", run["sentinel"]);
    let _ = std::fs::remove_dir_all(&out);
}
