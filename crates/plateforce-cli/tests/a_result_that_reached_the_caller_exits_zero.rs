//! A run that produced a result exits zero, whatever any single quantity did.
//!
//! Five of the six committed fixtures end while the athlete is still airborne, which is a
//! property of real collections rather than of these files, so a trial with no touchdown is the
//! ordinary case. Flight time and the height resting on it decline by name, in place, and the
//! run used to exit 65 carrying a complete document. `plateforce analyse ... && echo done`
//! never printed done, and a `set -e` script stopped on a run that worked.
//!
//! A status reports whether a result reached the caller. Which quantities it holds is in the
//! result: in place in the text, in `refusals` in the document, and in `refusals.csv` for a
//! folder. Three scripts in this repository had already written comments explaining that they
//! must not read the old status as failure.

use std::process::{Command, Output};

/// Trimmed before the athlete came back down, so no touchdown is in the recording to find.
const TRIMMED_BEFORE_LANDING: &str =
    "../plateforce-conformance/fixtures/subject01_trial2.force.txt";

/// The one committed fixture that returns to the plate, so every quantity resolves.
const LANDS_ON_THE_PLATE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn analysing(trial: &str, extra: &[&str]) -> Output {
    let mut line = vec![
        "--registry",
        "../../registry",
        "analyse",
        trial,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--preset",
        "sams",
    ];
    line.extend_from_slice(extra);
    plateforce(&line)
}

/// Asserted together, because a change that turned the status green by dropping the declined
/// quantity from the output would pass a status-only check and be a far worse defect.
#[test]
fn a_quantity_that_declined_is_reported_in_the_result_and_not_in_the_status() {
    let output = analysing(TRIMMED_BEFORE_LANDING, &[]);
    let printed = String::from_utf8_lossy(&output.stdout).to_string();

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        printed.contains("Flight time"),
        "the quantity is still named: {printed}"
    );
    assert!(
        printed.contains("no value"),
        "and it is still reported as having none: {printed}"
    );
    assert!(
        printed.contains("Jump height, takeoff frame"),
        "the quantities that did compute are all there: {printed}"
    );
}

/// The document a program reads carries the same decline, under the code it is published as.
#[test]
fn the_document_carries_the_refusal_the_status_no_longer_reports() {
    let output = analysing(TRIMMED_BEFORE_LANDING, &["--format", "json"]);
    assert_eq!(output.status.code(), Some(0));

    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("a whole document");
    let refusals = document["ok"]["refusals"]
        .as_array()
        .expect("the refusals relation is present");

    assert!(
        !refusals.is_empty(),
        "the route a caller reads instead of the status is not empty"
    );
    assert!(
        refusals.iter().all(|refusal| refusal["code"].is_string()),
        "every refusal carries the code a caller branches on: {refusals:?}"
    );
}

/// A trial that does land answers the same way, so the status does not distinguish the two and
/// a caller cannot read completeness out of it by accident.
#[test]
fn a_trial_that_lands_exits_the_same_way_as_one_that_does_not() {
    assert_eq!(analysing(LANDS_ON_THE_PLATE, &[]).status.code(), Some(0));
    assert_eq!(
        analysing(TRIMMED_BEFORE_LANDING, &[]).status.code(),
        Some(0)
    );
}

/// A run that produced nothing still reports why, so the change narrows what a non-zero status
/// means rather than removing it.
#[test]
fn a_run_that_produced_no_result_still_reports_its_fault() {
    let missing_file = plateforce(&[
        "--registry",
        "../../registry",
        "analyse",
        "no-such-trial.force.txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--preset",
        "sams",
    ]);
    assert_eq!(missing_file.status.code(), Some(66), "EX_NOINPUT");

    let open_choice = plateforce(&[
        "--registry",
        "../../registry",
        "analyse",
        LANDS_ON_THE_PLATE,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
    ]);
    assert_eq!(open_choice.status.code(), Some(64), "EX_USAGE");
}
