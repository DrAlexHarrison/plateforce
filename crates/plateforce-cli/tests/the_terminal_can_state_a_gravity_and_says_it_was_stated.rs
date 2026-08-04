//! The terminal wrote standard gravity as a literal into every request it built, so a reader
//! who had measured a gravity at their own plate had no way to say so, and every result this
//! surface produced carried a value nobody had been asked about with nothing saying that.
//!
//! Four of the eleven numbers move when that value moves. This asks the built binary, because
//! a flag whose help describes something the parser does not accept is the same defect the
//! record exists to close, one layer out.

use std::collections::BTreeMap;
use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";
const STANDARD: f64 = 9.80665;

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// One analysis with every choice on the path named, so nothing below is refused for a
/// decision the registry forces rather than for the thing under test.
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
        "--delimiter",
        "\t",
        "--weighing",
        "bwepoch.fixed_window",
        "--set",
        "weighing.duration=1.0",
        "--onset",
        "onset.threshold.noise_relative",
        "--set",
        "onset.k=5.0",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "takeoff.threshold_n=20.0",
    ]
    .iter()
    .map(|word| (*word).to_string())
    .collect();
    line.extend(extra.iter().map(|word| (*word).to_string()));
    line
}

fn analysed(extra: &[&str]) -> serde_json::Value {
    let line = analysing(extra);
    let borrowed: Vec<&str> = line.iter().map(String::as_str).collect();
    let output = plateforce(&borrowed);
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let document: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("the run produced a document:\n{text}"));
    document["ok"].clone()
}

fn globals(document: &serde_json::Value) -> BTreeMap<String, (f64, String)> {
    document["bound_globals"]
        .as_array()
        .expect("the document carries what the analysis was bound to")
        .iter()
        .map(|bound| {
            (
                bound["name"].as_str().expect("a name").to_string(),
                (
                    bound["value"].as_f64().expect("a value"),
                    bound["source"].as_str().expect("a source").to_string(),
                ),
            )
        })
        .collect()
}

fn numbers(document: &serde_json::Value) -> BTreeMap<String, Option<f64>> {
    document["metrics"]
        .as_array()
        .expect("the document carries the numbers")
        .iter()
        .map(|metric| {
            (
                metric["key"].as_str().expect("a key").to_string(),
                metric["value"].as_f64(),
            )
        })
        .collect()
}

/// A gravity the reader stated reaches the numbers and reaches the record, and the set of
/// numbers it reached is measured here rather than listed.
#[test]
fn a_gravity_the_reader_states_moves_the_numbers_and_is_recorded_as_theirs() {
    let quiet = analysed(&[]);
    let stated = analysed(&["--gravity", "9.70"]);

    let (before, after) = (numbers(&quiet), numbers(&stated));
    let moved: Vec<&String> = before
        .keys()
        .filter(|key| before[*key] != after[*key])
        .collect();
    println!(
        "{} of {} numbers moved: {moved:?}",
        moved.len(),
        before.len()
    );
    assert!(
        !moved.is_empty(),
        "the flag was accepted and moved nothing, so it reached no rule"
    );

    assert_eq!(
        globals(&stated)["gravity_meters_per_second_squared"],
        (9.70, "stated".to_string())
    );
    assert_eq!(
        globals(&quiet)["gravity_meters_per_second_squared"],
        (STANDARD, "assumed".to_string()),
        "a run nobody stated a gravity for still ran at one, and says nobody chose it"
    );
}

/// The record is in front of a reader who did not know to ask for it. `--provenance` widens
/// what each rule shows; it is not the gate on whether the analysis says what it ran under.
#[test]
fn the_terminal_names_the_gravity_without_being_asked_for_provenance() {
    let mut line = analysing(&[]);
    line.retain(|word| word != "--format" && word != "json");
    let borrowed: Vec<&str> = line.iter().map(String::as_str).collect();
    let output = plateforce(&borrowed);
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    println!("{text}");
    assert!(
        text.contains("gravity_meters_per_second_squared = 9.80665 m/s2, assumed"),
        "the terminal ran at a gravity and did not say which:\n{text}"
    );
}

/// Every landmark the request can carry, placed from the line, with the record saying it was
/// placed. Three of the four are recorded on the rule they override; the touchdown runs no
/// rule of its own, so nothing but the analysis can carry it.
#[test]
fn a_landmark_placed_by_hand_moves_the_analysis_and_leaves_a_record() {
    let placed = analysed(&["--place", "onset=4000", "--place", "touchdown=5800"]);
    assert_eq!(placed["onset_index"].as_u64(), Some(4000));
    assert_eq!(placed["touchdown_index"].as_u64(), Some(5800));
    assert_eq!(
        globals(&placed)["touchdown_index"],
        (5800.0, "stated".to_string())
    );

    let overridden: Vec<&str> = placed["bound_methods"]
        .as_array()
        .expect("the document carries the rules")
        .iter()
        .filter(|bound| bound["manual_override"].as_bool() == Some(true))
        .map(|bound| bound["method_id"].as_str().expect("an id"))
        .collect();
    assert_eq!(overridden, vec!["onset.threshold.noise_relative"]);

    let anchored = analysed(&["--place", "weighing=50"]);
    assert_eq!(anchored["weighing_start_index"].as_u64(), Some(50));
}

/// A landmark stated twice is refused rather than resolved to whichever came last, and a
/// sample written against something this run has no landmark for is refused by name with the
/// list. Both would otherwise be read, accepted and passed to nothing.
#[test]
fn a_line_that_places_a_landmark_twice_or_places_nothing_is_refused_by_name() {
    for (extra, expected) in [
        (
            vec!["--place", "onset=100", "--place", "onset=200"],
            "--place onset was given both 100 and 200",
        ),
        (
            vec!["--place", "elbow=100"],
            "--place elbow names no landmark of this run, which has weighing, onset, takeoff, touchdown",
        ),
        (
            vec!["--place", "onset=halfway"],
            "--place onset was given 'halfway', which is not a sample index counting from zero",
        ),
        (
            vec!["--place", "onset"],
            "--place takes <slot>=<sample>, and 'onset' carries no =",
        ),
    ] {
        let line = analysing(&extra);
        let borrowed: Vec<&str> = line.iter().map(String::as_str).collect();
        let output = plateforce(&borrowed);
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            said.contains(expected),
            "expected '{expected}' and the run said:\n{said}"
        );
        assert_ne!(output.status.code(), Some(0), "a refused line exited clean");
    }
}
