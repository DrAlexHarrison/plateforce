//! The terminal names the recording, the rate and the missing-value convention, above the
//! numbers it computed from them.
//!
//! A result pasted into a lab report otherwise names no file and no convention, so one of six
//! trials cannot be told from another. The convention mattered most: a reader who declared one
//! was told nothing about what it matched, while the browser's column chooser already counted
//! the same samples, so one file read two ways depending on the surface.
//!
//! The count is stated and never characterised. Under the zero convention a jump trace matches
//! the whole flight phase, so most of what it names is an athlete in the air rather than a gap,
//! which is why the reader leaves the samples exactly as the file wrote them.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";
const ROWS: usize = 6000;

/// Samples of `subject01_trial1` that read exactly 0 N with nothing planted in it. They are
/// the flight phase, which is what makes the zero convention ambiguous on a jump trace.
const FLIGHT_ZEROS: usize = 157;

/// Planted inside the weighing window, which is the first 1200 samples at 1200 Hz, so they
/// land where a vendor's placeholder would do the most damage to the standing weight.
const PLANTED: [usize; 3] = [10, 11, 12];

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// The block is wrapped to the terminal, so a sentence spans two lines and a pattern that
/// cannot cross the break reports absent text as missing.
fn unwrapped(document: &str) -> String {
    document.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn analysed(trial: &str, sentinel: &str) -> String {
    let output = plateforce(&[
        "--registry",
        "../../registry",
        "analyse",
        trial,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        sentinel,
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
    assert!(
        output.status.success(),
        "the run reached a result: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the document is UTF-8")
}

/// The shipped trial with three placeholder rows written into the standing window, which is
/// the export defect the convention exists to name.
fn with_three_zeros_planted() -> String {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let mut rows: Vec<String> = std::fs::read_to_string(&source)
        .expect("the fixture is committed")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        rows.len(),
        ROWS,
        "the fixture is the length this file assumes"
    );
    for index in PLANTED {
        assert_ne!(rows[index], "0", "the planted row was not already a zero");
        rows[index] = "0".to_string();
    }
    let planted = std::env::temp_dir().join("plateforce-three-planted-zeros.force.txt");
    std::fs::write(&planted, rows.join("\n")).expect("the planted trial writes");
    planted.display().to_string()
}

#[test]
fn the_recording_is_named_above_the_numbers_taken_from_it() {
    let document = analysed(FIXTURE, "none");
    let head: Vec<&str> = document.lines().take(3).collect();

    assert_eq!(head[0], "Trial");
    assert!(
        head[1].contains("subject01_trial1.force.txt"),
        "the trial names the file it read: {head:?}"
    );
    assert!(
        head[2].contains("6000 rows at 1200 Hz"),
        "the trial names the rows and the rate nobody can read off the numbers: {head:?}"
    );
}

/// The count moves by exactly what was planted, so the line reports the recording rather than
/// a constant. Both readings carry the denominator, which is what makes 157 and 160 legible as
/// the same file rather than as two.
#[test]
fn the_declared_convention_reports_what_it_matched_with_its_denominator() {
    let untouched = analysed(FIXTURE, "zero");
    assert!(
        unwrapped(&untouched).contains(&format!("{FLIGHT_ZEROS} of {ROWS} samples read it")),
        "the flight phase is what the zero convention matches on a clean trial:\n{untouched}"
    );

    let planted = analysed(&with_three_zeros_planted(), "zero");
    let expected = FLIGHT_ZEROS + PLANTED.len();
    assert!(
        unwrapped(&planted).contains(&format!("{expected} of {ROWS} samples read it")),
        "three planted placeholders move the count by three:\n{planted}"
    );
}

/// The samples stay where they were. Removing them closes the gap and shifts every timestamp
/// after it, which on this trial deletes the flight phase, so a reader who is not told they
/// were kept will assume the opposite of what happened.
#[test]
fn the_matched_samples_are_said_to_be_left_as_the_file_wrote_them() {
    let document = analysed(FIXTURE, "zero");
    assert!(
        unwrapped(&document).contains("counted and left as the file wrote them"),
        "a reader cannot recover this from the numbers:\n{document}"
    );
}

/// Declaring nothing is a declaration, so the line is printed under every convention including
/// the one that names no value at all.
#[test]
fn a_convention_that_matched_nothing_still_says_so() {
    let document = analysed(FIXTURE, "negative_one");
    assert!(
        unwrapped(&document).contains(&format!(
            "-1 N is the value stated for a measurement not taken: 0 of {ROWS}"
        )),
        "the convention the caller declared is reported whether or not it matched:\n{document}"
    );
}

/// The browser's column chooser already tells a reader how many samples read exactly zero
/// before they commit to anything. Under a `none` declaration the terminal said nothing, so the
/// vendor-export defect the whole convention exists for was invisible on the surface a student
/// is most likely to script.
#[test]
fn stating_no_convention_still_counts_the_samples_that_read_exactly_zero() {
    let document = analysed(&with_three_zeros_planted(), "none");
    let expected = FLIGHT_ZEROS + PLANTED.len();

    assert!(
        unwrapped(&document).contains("No value is stated for a measurement not taken"),
        "the reader is told what their own declaration meant:\n{document}"
    );
    assert!(
        unwrapped(&document).contains(&format!("{expected} of {ROWS} samples read exactly 0 N")),
        "the count the column chooser reports reaches the terminal too:\n{document}"
    );
}
