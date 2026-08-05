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

/// What the record says produced one named value on one named rule, and None where no row
/// names it. Read off the relation rather than counted, so a claim about one value cannot be
/// met by a total that moved somewhere else.
fn source_of(out_dir: &std::path::Path, method_id: &str, parameter: &str) -> Option<String> {
    let text = std::fs::read_to_string(out_dir.join("provenance.csv")).expect("a record");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let column = |name: &str| header.iter().position(|held| *held == name).expect(name);
    let (rule, held, source) = (column("method_id"), column("parameter"), column("source"));
    lines
        .map(|line| line.split(',').collect::<Vec<&str>>())
        .find(|fields| fields.get(rule) == Some(&method_id) && fields.get(held) == Some(&parameter))
        .and_then(|fields| fields.get(source).map(|found| (*found).to_string()))
}

/// What a run over this folder exits with, and why it is not zero.
///
/// Five of the six subject-01 trials never return above the takeoff threshold, so no touchdown
/// is placed on them, so `jumpheight.takeoff.flight_time` has no interval to work on and
/// declines by name. A batch holding a trial whose requested headline number could not be
/// produced is not a clean run, and the exit code is where a reader learns that without
/// reading the record.
const A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER: i32 = 65;

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("plateforce-batch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

/// What the record says about one value the operator stated and one they did not.
///
/// Taken over `takeoff.threshold_n`, which sits on a construct that forces no decision, so it
/// is a value the run may reach either way. The two values the path does force are named in
/// both arms: a folder run naming neither is refused before a trial is read, the way one trial
/// has always been, so a baseline that named nothing would be comparing two refusals.
#[test]
fn a_value_stated_for_a_folder_is_recorded_as_stated() {
    let forced = vec!["--set", "weighing.duration=1.0", "--set", "onset.k=5"];

    let without = scratch("plain");
    assert_eq!(
        batch(&without, &forced).status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER)
    );
    let before = sources(&without);

    let with = scratch("stated");
    let mut also = forced.clone();
    also.extend(["--set", "takeoff.threshold_n=20"]);
    let output = batch(&with, &also);
    assert_eq!(
        output.status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER)
    );
    let after = sources(&with);

    println!("without the value {before:?}");
    println!("with the value    {after:?}");
    assert_eq!(
        source_of(&without, "takeoff.threshold.absolute_force", "threshold_n"),
        Some("assumed".to_string()),
        "a value nobody stated reads as the rule's own"
    );
    assert_eq!(
        source_of(&with, "takeoff.threshold.absolute_force", "threshold_n"),
        Some("stated".to_string()),
        "a value the operator stated reads stated"
    );
    assert!(
        after.get("stated").copied().unwrap_or(0) > before.get("stated").copied().unwrap_or(0),
        "stating a value did not move the count of stated rows"
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
        batch(
            &out,
            &["--set", "weighing.duration=1.0", "--set", "onset.k=5"]
        )
        .status
        .code(),
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
