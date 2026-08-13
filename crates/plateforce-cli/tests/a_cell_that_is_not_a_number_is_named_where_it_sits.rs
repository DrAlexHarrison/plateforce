//! A cell the reader could not take as a number is reported by its text and its line.
//!
//! The two files a novice arrives with are a spreadsheet export carrying a header row and a
//! comma-separated file whose delimiter nobody named. Both used to answer `column index 1 must
//! be a finite number, got NaN`, which names neither the row nor the text, and states a value
//! the record itself reports as null: the file holds no NaN, it holds the words `Force (N)`.
//!
//! The caller's own values reach the same code and keep the sentence they had, because a number
//! somebody typed is a different fault from a cell somebody exported.

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

/// Written to a path of this test's own, so a sibling lane's file cannot answer for it.
fn file_holding(name: &str, contents: &str) -> String {
    let path = std::env::temp_dir().join(format!("plateforce-unreadable-cell-{name}"));
    std::fs::write(&path, contents).expect("the trial writes");
    path.display().to_string()
}

fn reading(trial: &str, extra: &[&str]) -> Output {
    let mut line = vec![
        "--registry",
        "../../registry",
        "analyse",
        trial,
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
    ];
    line.extend_from_slice(extra);
    plateforce(&line)
}

fn said(output: &Output) -> String {
    let both = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    both.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// A spreadsheet writes its column names into row one, which is the commonest thing a reader
/// meets that is not a measurement.
#[test]
fn a_header_row_is_reported_by_the_words_it_holds() {
    let trial = file_holding("header.csv", "Time,Force (N)\n0,584.3485\n0.001,584.3485\n");
    let output = reading(&trial, &["--column", "1", "--delimiter", ","]);

    assert_eq!(
        said(&output),
        "plateforce: column index 1 reads \"Force (N)\" on line 1, which is not a number"
    );
    assert!(!said(&output).contains("NaN"), "the file holds no NaN");
}

/// With no delimiter named the whole row is one field, so the text carries the separator the
/// caller did not name and the reading ends there rather than at a second run.
#[test]
fn a_row_nobody_split_is_reported_by_the_text_that_carries_the_separator() {
    let trial = file_holding("undelimited.csv", "0,584.3485\n0.001,584.3485\n");
    let output = reading(&trial, &["--column", "0"]);

    assert_eq!(
        said(&output),
        "plateforce: column index 0 reads \"0,584.3485\" on line 1, which is not a number"
    );
}

/// The record every other surface reads carries the text and the line, so a browser tab and an
/// R session can compose the same sentence rather than meeting a code with nothing under it.
#[test]
fn the_record_on_the_wire_carries_the_text_and_the_line() {
    let trial = file_holding(
        "header-json.csv",
        "Time,Force (N)\n0,584.3485\n0.001,584.3485\n",
    );
    let output = reading(
        &trial,
        &["--column", "1", "--delimiter", ",", "--format", "json"],
    );

    let document: serde_json::Value = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|_| serde_json::from_slice(&output.stdout).expect("a refusal document"));
    let refusal = &document["refusal"];

    assert_eq!(refusal["named_value"], serde_json::json!("Force (N)"));
    assert_eq!(refusal["detail"]["line_number"], serde_json::json!(1.0));
    assert_eq!(
        refusal["message"],
        serde_json::json!("column index 1 reads \"Force (N)\" on line 1, which is not a number")
    );
    assert_eq!(refusal["value"], serde_json::Value::Null);
}

/// A number the caller typed is a fault in the request rather than in the recording, and it has
/// no line to name, so it keeps the sentence it had.
#[test]
fn a_value_the_caller_typed_still_reports_the_value_they_typed() {
    let output = reading(FIXTURE, &["--column", "0", "--body-mass-kg", "nan"]);

    assert!(
        said(&output).contains("body_mass_kilograms must be a finite number, got NaN"),
        "the caller's own value keeps its reading: {}",
        said(&output)
    );
}
