//! The terminal takes a stated body mass and records that the reader stated it.
//!
//! The athlete's mass is not the weighed system mass, so a surface with no way to say it
//! leaves a reader substituting the number next to it, and the record cannot tell the two
//! apart afterwards.
//!
//! This asks the built binary, because a flag whose help describes something the parser does
//! not accept is the same defect the record exists to close, one layer out.

use std::collections::BTreeMap;
use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";
const STATED_MASS_KILOGRAMS: f64 = 61.5;

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

/// A mass the reader stated reaches the record under the reader's own claim, and a run that
/// states none carries no row for it.
///
/// The absence half is the load-bearing one: a row written whatever the caller said would
/// report the software's silence as their choice, which is the claim this record exists to
/// get right.
#[test]
fn a_body_mass_the_reader_states_is_recorded_as_theirs_and_silence_is_recorded_as_nothing() {
    let stated = analysed(&["--body-mass-kg", "61.5"]);
    assert_eq!(
        globals(&stated)["body_mass_kilograms"],
        (STATED_MASS_KILOGRAMS, "stated".to_string())
    );

    let quiet = analysed(&[]);
    assert!(
        !globals(&quiet).contains_key("body_mass_kilograms"),
        "a run nobody stated a mass for carried a row for one: {:?}",
        globals(&quiet)
    );
}

/// The unit travels with the number, because a mass read back without one is a number a
/// reader has to assume the units of, and this record exists so nothing is assumed.
#[test]
fn the_recorded_mass_carries_its_unit() {
    let stated = analysed(&["--body-mass-kg", "61.5"]);
    let row = stated["bound_globals"]
        .as_array()
        .expect("the document carries what the analysis was bound to")
        .iter()
        .find(|bound| bound["name"] == "body_mass_kilograms")
        .expect("the stated mass is on the record")
        .clone();
    assert_eq!(row["unit"], "kilograms");
    assert_eq!(row["unit_symbol"], "kg");
}

/// A number no mass can be is refused under the name the record reports the value by, rather
/// than reaching a rule or being read as another flag.
///
/// The negative case is the one that needed the parser told about it: without that, the line
/// came back naming `-6` as an unexpected argument, which says nothing about what was wrong.
#[test]
fn a_mass_that_is_not_a_positive_finite_number_is_refused_by_name() {
    for (written, expected) in [
        (
            "0",
            "body_mass_kilograms does not accept 0: it takes a mass above zero",
        ),
        (
            "-61.5",
            "body_mass_kilograms does not accept -61.5: it takes a mass above zero",
        ),
        (
            "nan",
            "body_mass_kilograms must be a finite number, got NaN",
        ),
        (
            "inf",
            "body_mass_kilograms must be a finite number, got inf",
        ),
    ] {
        let line = analysing(&["--body-mass-kg", written]);
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

/// The flag takes a value rather than swallowing whatever follows it. Telling the parser to
/// accept negative numbers is what makes the refusal above reachable, and the same setting
/// would let a following flag be read as this one's value.
#[test]
fn the_mass_flag_does_not_read_the_next_flag_as_its_value() {
    let line = analysing(&["--body-mass-kg", "--provenance"]);
    let borrowed: Vec<&str> = line.iter().map(String::as_str).collect();
    let output = plateforce(&borrowed);
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        said.contains("a value is required for '--body-mass-kg <KG>'"),
        "the flag read a flag as its mass:\n{said}"
    );
}
