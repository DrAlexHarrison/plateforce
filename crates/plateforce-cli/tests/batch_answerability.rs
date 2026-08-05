//! Whether one request gets one answer on the folder and on the single trial.
//!
//! The terminal refused a rule whose required number the literature publishes several ways;
//! the folder ran every trial at whichever value the code held and recorded that nobody was
//! asked. One surface treated the choice as forced and the other as defaultable, over the same
//! typed request, and the folder is the surface where the unmade choice is multiplied by the
//! trial count into a spreadsheet nobody re-reads the provenance of.
//!
//! Both halves are here on purpose. A guard holding only the refusal is met by a build that
//! refuses everything, and the value this run is about, `onset.k`, moves net impulse
//! reliability from 0.984 to 0.479 on identical data.

use std::process::Output;

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// The rules and values both surfaces are given, less the one under test.
const EVERY_RULE_NAMED: [&str; 8] = [
    "--weighing",
    "bwepoch.fixed_window",
    "--set",
    "weighing.duration=1.0",
    "--onset",
    "onset.threshold.noise_relative",
    "--takeoff",
    "takeoff.threshold.absolute_force",
];

/// The two values a fully specified run states. `takeoff.threshold_n` is on a construct that
/// forces nothing, so it is not the value under test; it is here because a run missing it is
/// answering a different question.
const EVERY_VALUE_NAMED: [&str; 4] = ["--set", "onset.k=5", "--set", "takeoff.threshold_n=20"];

const HOW_THE_TRACES_READ: [&str; 6] = [
    "--column",
    "0",
    "--sample-rate-hz",
    "1200",
    "--sentinel",
    "none",
];

/// Five of the six committed trials were trimmed before the athlete landed, so the flight-time
/// height declines by name on them. A run that produced its table and could not produce one
/// requested number exits here, which is a different answer from refusing to run at all.
const A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER: i32 = 65;
const A_CHOICE_ON_THE_PATH_IS_STILL_OPEN: i32 = 64;

fn plateforce(arguments: &[String]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn words(parts: &[&[&str]]) -> Vec<String> {
    parts
        .iter()
        .flat_map(|part| part.iter())
        .map(|word| (*word).to_string())
        .collect()
}

fn one_trial(values: &[&str]) -> Output {
    let mut line = words(&[
        &["--registry", "../../registry", "analyse", FIXTURE],
        &HOW_THE_TRACES_READ,
        &EVERY_RULE_NAMED,
        values,
    ]);
    line.retain(|word| !word.is_empty());
    plateforce(&line)
}

/// A directory of its own per run, so what a run wrote is what this test put there.
fn scratch(name: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "plateforce-answerability-{name}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn the_folder(out_dir: &std::path::Path, values: &[&str]) -> Output {
    let named = out_dir.display().to_string();
    let mut line = words(&[
        &[
            "--registry",
            "../../registry",
            "batch",
            "../plateforce-conformance/fixtures",
            "--out-dir",
            &named,
            "--trial-suffix",
            ".force.txt",
        ],
        &HOW_THE_TRACES_READ,
        &EVERY_RULE_NAMED,
        values,
    ]);
    line.retain(|word| !word.is_empty());
    plateforce(&line)
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// One request, both surfaces, the same refusal word for word.
#[test]
fn the_folder_refuses_the_value_the_single_trial_refuses() {
    let out_dir = scratch("refused");
    let alone = one_trial(&[]);
    let folder = the_folder(&out_dir, &[]);

    let told_one = stderr_of(&alone);
    let told_many = stderr_of(&folder);
    println!("one trial exits {:?}:\n{told_one}", alone.status.code());
    println!("the folder exits {:?}:\n{told_many}", folder.status.code());

    assert_eq!(
        alone.status.code(),
        Some(A_CHOICE_ON_THE_PATH_IS_STILL_OPEN),
        "{told_one}"
    );
    assert_eq!(
        folder.status.code(),
        Some(A_CHOICE_ON_THE_PATH_IS_STILL_OPEN),
        "{told_many}"
    );
    assert_eq!(
        told_one, told_many,
        "one request was refused two ways on two surfaces"
    );
    assert!(told_many.contains("--set onset.k="), "{told_many}");
    assert!(told_many.contains("published at"), "{told_many}");
}

/// A refused folder run leaves nothing behind. A table written beside a refusal is the
/// artifact this refusal exists to stop.
#[test]
fn a_refused_folder_writes_no_table() {
    let out_dir = scratch("nothing-written");
    let folder = the_folder(&out_dir, &[]);

    let written: Vec<String> = std::fs::read_dir(&out_dir)
        .expect("the run was given a folder")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    println!("written on a refused run: {written:?}");
    assert!(folder.stdout.is_empty(), "a refused run printed a table");
    assert!(written.is_empty(), "{written:?}");
}

/// The other half. Without this, a build that refused every folder run would pass the guard
/// above, and the reliability figure the value moves would never be reachable at all.
#[test]
fn naming_every_value_runs_on_both_surfaces() {
    let out_dir = scratch("named");
    let alone = one_trial(&EVERY_VALUE_NAMED);
    let folder = the_folder(&out_dir, &EVERY_VALUE_NAMED);

    let told_one = stderr_of(&alone);
    let told_many = stderr_of(&folder);
    assert_eq!(alone.status.code(), Some(0), "{told_one}");
    assert_eq!(
        folder.status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER),
        "{told_many}"
    );
    assert!(
        !alone.stdout.is_empty(),
        "the single trial produced nothing"
    );
    assert!(!folder.stdout.is_empty(), "the folder produced nothing");
    assert!(
        out_dir.join("results.csv").exists(),
        "the folder wrote no table"
    );
}

/// A value the folder was given is recorded as the caller's, not as one nobody was asked
/// about. The refusal above and this record are the two halves of one claim: nothing runs on
/// an unmade choice, and a made one says who made it.
#[test]
fn a_stated_value_is_recorded_as_stated() {
    let out_dir = scratch("recorded");
    the_folder(&out_dir, &EVERY_VALUE_NAMED);
    let text = std::fs::read_to_string(out_dir.join("provenance.csv")).expect("a record");

    let sources: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("onset.threshold.noise_relative") && line.contains(",k,"))
        .filter_map(|line| line.rsplit(',').next())
        .collect();
    println!("what the record says about k: {sources:?}");
    assert!(!sources.is_empty(), "no row in the record names k");
    assert!(
        sources.iter().all(|source| *source == "stated"),
        "{sources:?}"
    );
}

/// A refusal a reader cannot read has not refused, and eighty columns is the floor every
/// common terminal reaches. Held over the value refusal, which no guard reached: the sentence
/// it shipped with ran to a hundred columns with the program name in front of it.
#[test]
fn the_refusal_reads_at_eighty_columns_on_both_surfaces() {
    let out_dir = scratch("width");
    for (surface, output) in [
        ("one trial", one_trial(&[])),
        ("the folder", the_folder(&out_dir, &[])),
    ] {
        for line in stderr_of(&output).lines() {
            assert!(
                line.chars().count() <= 80,
                "{surface}: {} columns: {line}",
                line.chars().count()
            );
        }
    }
}
