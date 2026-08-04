//! What the terminal prints where a quantity's arithmetic produced no number.
//!
//! This column printed `NaN`, by accident of formatting a float, while the same command's
//! `--format json` wrote `null`, which is also what it writes for a quantity no rule
//! produced. So one command had two renderings of one result and only the one nobody parses
//! could tell the two states apart.
//!
//! The words are read out of the page and the states out of the record, from the same run, so
//! this asserts that the page agrees with the document rather than that the page contains a
//! sentence written here. A renderer that printed the right words against the wrong rows
//! fails.
//!
//! The control is the intact recording, where no row is in that state, and it can come back
//! empty for the same reason the real query would.

use std::collections::BTreeSet;
use std::process::Command;

/// One recording of subject 01 with three samples of its quiet stance unreadable, inside the
/// one-second weighing window this request binds.
const INTERRUPTED: &str = "../plateforce-conformance/damaged/subject01_trial1_interrupted.force.txt";
const INTACT: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

const SAMPLES_CARRYING_NO_NUMBER: usize = 3;
const ROWS_IN_THE_RECORDING: usize = 6000;

fn analyse(fixture: &str, format: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
            "--registry",
            "../../registry",
            "--format",
            format,
            "analyse",
            fixture,
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
            "--delimiter",
            "\t",
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
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    // This request declines a landmark on the interrupted recording, so the terminal exits
    // non-zero while writing a whole document. Reading the status as failure is how a harness
    // once reported that no surface had answered at all.
    String::from_utf8(output.stdout).expect("the document is UTF-8")
}

/// The labels the record says are in each state, keyed by label because the label is what the
/// page carries.
fn labels_by_state(document: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let parsed: serde_json::Value = serde_json::from_str(document).expect("the result parses");
    let mut not_a_number = BTreeSet::new();
    let mut no_value = BTreeSet::new();
    for metric in parsed["ok"]["metrics"]
        .as_array()
        .expect("a result carries its metrics")
    {
        if !metric["value"].is_null() {
            continue;
        }
        let label = metric["label"]
            .as_str()
            .expect("every metric carries a label")
            .to_string();
        match metric["carried_no_number"]
            .as_bool()
            .expect("every metric says whether its arithmetic produced a number")
        {
            true => not_a_number.insert(label),
            false => no_value.insert(label),
        };
    }
    (not_a_number, no_value)
}

/// The labels the page put each phrase against, read out of the printed column.
fn labels_the_page_marked(printed: &str, phrase: &str) -> BTreeSet<String> {
    printed
        .lines()
        .filter_map(|line| {
            let (label, _unit) = line.split_once(&format!("  {phrase} "))?;
            Some(label.trim().to_string())
        })
        .collect()
}

#[test]
fn the_page_and_the_record_agree_about_which_rows_carried_no_number() {
    let (not_a_number, no_value) = labels_by_state(&analyse(INTERRUPTED, "json"));
    assert!(
        !not_a_number.is_empty() && !no_value.is_empty(),
        "this recording puts rows in both states, and a page showing only one of them cannot \
         show that the terminal tells them apart: {not_a_number:?} {no_value:?}"
    );

    let printed = analyse(INTERRUPTED, "text");
    println!("{printed}");
    assert_eq!(labels_the_page_marked(&printed, "not a number"), not_a_number);
    assert_eq!(labels_the_page_marked(&printed, "no value"), no_value);
}

#[test]
fn the_terminal_says_what_the_recording_lost_and_over_how_many_samples() {
    let printed = analyse(INTERRUPTED, "text");
    let said = printed.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        said.contains(&format!(
            "{SAMPLES_CARRYING_NO_NUMBER} of {ROWS_IN_THE_RECORDING} samples carry no number"
        )),
        "the count carries the population it was taken over: {said}"
    );

    // The control, and it is the reason the sentence is conditional rather than always
    // printed: a recording that lost nothing says nothing.
    let intact = analyse(INTACT, "text");
    assert!(!intact.contains("carry no number"), "{intact}");
}

#[test]
fn an_intact_recording_marks_no_row_either_way() {
    let (not_a_number, no_value) = labels_by_state(&analyse(INTACT, "json"));
    assert_eq!(not_a_number, BTreeSet::new());
    assert_eq!(no_value, BTreeSet::new());

    let printed = analyse(INTACT, "text");
    assert_eq!(labels_the_page_marked(&printed, "not a number"), BTreeSet::new());
    assert_eq!(labels_the_page_marked(&printed, "no value"), BTreeSet::new());
}
