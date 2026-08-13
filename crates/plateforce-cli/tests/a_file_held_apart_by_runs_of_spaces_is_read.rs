//! A multi-column export whose columns are held apart by runs of spaces is readable.
//!
//! Fixed-width exports pad their columns to a width, so the gap between two values is several
//! spaces and its width varies down the file. A single stated space reads each run as one field
//! followed by several empty ones, which refuses on the first row, and no other separator fits,
//! so the file was unreadable rather than merely awkward.
//!
//! The word is `whitespace` because that is what the reader already calls it in the record, and
//! a reader who reads one and writes the other should not have to translate.

use std::process::{Command, Output};

/// The same trace written three ways. Any difference between the numbers they produce would be
/// the separator changing what was read rather than how it was split.
const ROWS: usize = 40;

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// Written under this file's own name, because the scratch directory is shared with every other
/// test running beside it.
fn written(name: &str, rows: impl Fn(usize) -> String) -> String {
    let body: String = (0..ROWS).map(&rows).collect();
    let path = std::env::temp_dir().join(format!("plateforce-runs-of-spaces-{name}"));
    std::fs::write(&path, body).expect("the trial writes");
    path.display().to_string()
}

fn seconds(row: usize) -> f64 {
    row as f64 / 1200.0
}

/// A quiet stance, which is all this file needs: the question is whether the columns were split,
/// not what the rules make of them.
fn newtons(row: usize) -> f64 {
    586.0 + (row % 3) as f64 * 0.1
}

fn analysing(trial: &str, separator: &str) -> Output {
    plateforce(&[
        "--registry",
        "../../registry",
        "analyse",
        trial,
        "--delimiter",
        separator,
        "--column",
        "1",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--weighing",
        "bwepoch.fixed_window",
        "--set",
        "weighing.duration=0.01",
        "--onset",
        "onset.threshold.noise_relative",
        "--set",
        "onset.k=5",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "takeoff.threshold_n=20",
    ])
}

fn system_weight(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.contains("System weight"))
        .map(|line| line.split_whitespace().collect::<Vec<&str>>().join(" "))
        .unwrap_or_default()
}

fn refusal(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// The shape measured on a real export: two columns with a run of spaces between them.
#[test]
fn runs_of_spaces_are_read_when_the_word_is_named() {
    let trial = written("run.txt", |row| {
        format!("{:.6}    {:.4}\n", seconds(row), newtons(row))
    });
    let output = analysing(&trial, "whitespace");

    assert_eq!(output.status.code(), Some(0), "{}", refusal(&output));
    assert!(
        system_weight(&output).contains("N"),
        "the force column was read: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// Padding to a fixed width makes the run's width vary down the file, which is what defeats
/// every single-character separator rather than only the obvious one.
#[test]
fn a_run_whose_width_varies_is_read_the_same_way() {
    let padded = written("padded.txt", |row| {
        format!("{:>12.6}{:>14.4}\n", seconds(row), newtons(row))
    });
    let plain = written("plain.txt", |row| {
        format!("{:.6}    {:.4}\n", seconds(row), newtons(row))
    });

    let from_padded = analysing(&padded, "whitespace");
    let from_plain = analysing(&plain, "whitespace");

    assert_eq!(
        from_padded.status.code(),
        Some(0),
        "{}",
        refusal(&from_padded)
    );
    assert_eq!(system_weight(&from_padded), system_weight(&from_plain));
}

/// The one answer, three ways. A separator decides how a row is split and never what the file
/// holds, so a tab file and a space-run file carrying the same numbers must agree exactly.
#[test]
fn the_same_trace_reads_the_same_however_its_columns_are_held_apart() {
    let by_tab = written("tab.txt", |row| {
        format!("{:.6}\t{:.4}\n", seconds(row), newtons(row))
    });
    let by_run = written("run-compare.txt", |row| {
        format!("{:.6}    {:.4}\n", seconds(row), newtons(row))
    });

    let tabbed = analysing(&by_tab, "\t");
    let spaced = analysing(&by_run, "whitespace");

    assert_eq!(tabbed.status.code(), Some(0), "{}", refusal(&tabbed));
    assert!(!system_weight(&tabbed).is_empty(), "the tab file was read");
    assert_eq!(system_weight(&tabbed), system_weight(&spaced));
}

/// A stated space is one space and does not widen to a run. Widening it would mean a caller who
/// named a single space silently got a different reader, which is the defect this whole flag
/// exists to avoid.
#[test]
fn a_single_stated_space_still_means_exactly_one_space() {
    let trial = written("single.txt", |row| {
        format!("{:.6}    {:.4}\n", seconds(row), newtons(row))
    });
    let output = analysing(&trial, " ");

    assert_eq!(output.status.code(), Some(64), "{}", refusal(&output));
    assert!(
        refusal(&output).contains("is not a number"),
        "the empty field between two runs is named: {}",
        refusal(&output)
    );
}

/// Neither a character nor the word, so the refusal names both forms rather than only the one
/// the caller happened to miss.
#[test]
fn a_word_that_is_neither_names_both_forms() {
    let trial = written("neither.txt", |row| {
        format!("{:.6}    {:.4}\n", seconds(row), newtons(row))
    });
    let output = analysing(&trial, "spaces");

    assert_eq!(output.status.code(), Some(64));
    assert!(
        refusal(&output).contains("one character or the word whitespace"),
        "{}",
        refusal(&output)
    );
}
