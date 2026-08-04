//! The single-trial run can be told about the plate it came off, and says whether it was.
//!
//! Measured 2026-08-04, three of the five surfaces could not be given an acquisition block at
//! all: the desktop app, this terminal and the browser. The two that could were R and Python,
//! the two that require a programming language. A surface that never asks for the block cannot
//! be repaired by a default, so the completeness a fingerprint carries is asked for here.

use std::process::Command;

const TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// Every member the block holds. Written out rather than read from `Acquisition::MEMBERS` on
/// purpose: a test that builds its input from the same constant the parser validates against
/// agrees with itself however the block changes, and would keep passing while a member silently
/// left the set.
const EVERY_MEMBER: [&str; 5] = [
    "filter_at_capture=none",
    "tare_state=tared",
    "plate_natural_frequency_hz=400",
    "floor_surface=concrete",
    "firmware_version=1.0",
];

fn analyse(acquisition: &[&str]) -> String {
    let (out, _) = run(acquisition);
    out
}

/// Both streams, because a refusal is written to the one a result is not. Reading only stdout
/// reports a refusal absent while a caller sees the message.
fn refusal(acquisition: &[&str]) -> String {
    let (out, err) = run(acquisition);
    format!("{out}{err}")
}

fn run(acquisition: &[&str]) -> (String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_plateforce"));
    command.args([
        "--registry",
        "../../registry",
        "--format",
        "json",
        "analyse",
        TRIAL,
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
    ]);
    for stated in acquisition {
        command.args(["--acquisition", stated]);
    }
    let output = command
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the terminal runs");
    (
        String::from_utf8(output.stdout).expect("the result is text"),
        String::from_utf8(output.stderr).expect("the refusal is text"),
    )
}

fn completeness(document: &str) -> bool {
    let parsed: serde_json::Value = serde_json::from_str(document)
        .unwrap_or_else(|error| panic!("the result is json: {error}\n{document}"));
    parsed["ok"]["acquisition_complete"]
        .as_bool()
        .unwrap_or_else(|| panic!("the result carries acquisition_complete\n{document}"))
}

#[test]
fn a_run_told_nothing_about_the_plate_fingerprints_as_incomplete() {
    assert!(
        !completeness(&analyse(&[])),
        "a run stating no acquisition member claimed a complete block"
    );
}

#[test]
fn a_run_told_every_member_fingerprints_as_complete() {
    assert!(
        completeness(&analyse(&EVERY_MEMBER)),
        "this surface cannot be given an acquisition block, which is the defect this file exists \
         for: every member was stated and the result still fingerprints as incomplete"
    );
}

/// The case that separates asking from pretending to ask. A surface could accept the flag, record
/// nothing, and pass the test above by always answering the caller; it could equally fill the block
/// and call any of it enough. Four of five is neither.
#[test]
fn a_block_short_of_one_member_is_still_incomplete() {
    let all_but_one = &EVERY_MEMBER[..EVERY_MEMBER.len() - 1];
    assert!(
        !completeness(&analyse(all_but_one)),
        "a block missing {} claimed to be complete",
        EVERY_MEMBER[EVERY_MEMBER.len() - 1]
    );
}

/// A refusal naming four of five members teaches four. The parser reads the accepted names off the
/// block itself, so this asserts every one by name rather than that a refusal happened.
#[test]
fn an_unknown_member_is_refused_by_name_and_the_refusal_names_them_all() {
    let refused = refusal(&["bogus=1"]);
    for member in EVERY_MEMBER {
        let name = member.split('=').next().expect("a member name");
        assert!(
            refused.contains(name),
            "the refusal does not name {name}, so a caller learns fewer members than exist\n\
             {refused}"
        );
    }
}
