//! What a folder run says about the registry it read and the plate it was captured on.
//!
//! Three facts a record can claim while the surface never delivers them. A `registry_version`
//! that no binding sets leaves the digest identifying a run hashing a null pin. A run row
//! carrying no `registry_declared_version` names no registry at all. And an
//! `acquisition_complete_count` no flag can move stays zero on every run.
//!
//! Every guard here is paired with a control that comes back the other way, because a run that
//! writes null for a pin nobody gave and a run that writes null for a pin it was given read
//! identically from one side.

use std::process::Output;

/// Most of these recordings end while the athlete is still off the plate, so a requested
/// number declines by name on several trials and the run exits 65. A check that accepted any
/// exit code would pass on a build that cannot run.
const A_FOLDER_RUN_THAT_WROTE_ITS_TABLES: i32 = 0;
const THE_REQUEST_COULD_NOT_BE_READ: i32 = 64;

const EVERY_MEMBER: [&str; 5] = [
    "filter_at_capture=none",
    "tare_state=tared_before_trial",
    "plate_natural_frequency_hz=400",
    "floor_surface=concrete",
    "firmware_version=2.4.1",
];

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
        "--set",
        "weighing.duration=1.0",
        "--set",
        "onset.k=5",
    ];
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("plateforce-record-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

/// The run block a folder run writes beside its tables.
fn run_record(name: &str, extra: &[&str]) -> serde_json::Value {
    let out_dir = scratch(name);
    let output = batch(&out_dir, extra);
    let code = output.status.code().unwrap_or(-1);
    assert_ne!(
        code,
        THE_REQUEST_COULD_NOT_BE_READ,
        "the run refused the request: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        code,
        A_FOLDER_RUN_THAT_WROTE_ITS_TABLES,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(out_dir.join("run.json")).expect("the record is written");
    let record: serde_json::Value = serde_json::from_str(&text).expect("the record parses");
    let _ = std::fs::remove_dir_all(&out_dir);
    record
}

/// The pin is the caller's word, and a run that was given one records it.
///
/// The control is the same run without the flag. Asserting only the pinned side would pass
/// against a surface that wrote the string into every record whatever the caller said.
#[test]
fn a_pinned_folder_run_records_the_pin_and_an_unpinned_one_records_null() {
    let pinned = run_record("pinned", &["--registry-version", "PIN-2026-08-04"]);
    let unpinned = run_record("unpinned", &[]);

    assert_eq!(
        pinned["registry_version"].as_str(),
        Some("PIN-2026-08-04"),
        "the run did not record the revision its caller cited"
    );

    // Null rather than absent or empty. A key a record sometimes omits cannot be told apart
    // from a surface that never carried the field, and `""` cannot be told from a caller who
    // pinned the empty string.
    assert!(
        unpinned.get("registry_version").is_some(),
        "the record does not carry registry_version at all"
    );
    assert!(
        unpinned["registry_version"].is_null(),
        "an unpinned run recorded {}, which reads as a revision somebody chose",
        unpinned["registry_version"]
    );
}

/// A pin is part of what identifies the run, so two runs differing only in it are two runs.
///
/// This is the fact the defect destroyed: `request_digest` hashed the pin, and the pin was
/// always absent, so a folder run that cited a revision and one that cited none were one
/// record.
#[test]
fn two_runs_differing_only_in_the_pin_carry_different_request_digests() {
    let pinned = run_record("digest-pinned", &["--registry-version", "PIN-2026-08-04"]);
    let unpinned = run_record("digest-unpinned", &[]);

    // The control: everything else about these two runs is identical, so the registry digest
    // agreeing is what makes the request digests differing mean the pin and nothing else.
    assert_eq!(
        pinned["registry_digest"], unpinned["registry_digest"],
        "these two runs read the same registry"
    );
    assert_ne!(
        pinned["request_digest"], unpinned["request_digest"],
        "the pin did not reach the digest that identifies the request"
    );
}

/// What the registry claims about itself is the registry's word, so it is on every record
/// whether or not anybody pinned anything, and it is never the caller's pin.
#[test]
fn the_record_carries_what_the_registry_declares_beside_what_the_caller_pinned() {
    let pinned = run_record("declared-pinned", &["--registry-version", "PIN-2026-08-04"]);
    let unpinned = run_record("declared-unpinned", &[]);

    let declared = unpinned["registry_declared_version"].as_str();
    assert!(
        declared.is_some(),
        "the record does not say what the registry declares about itself"
    );
    assert_eq!(
        pinned["registry_declared_version"].as_str(),
        declared,
        "the registry's own claim moved when the caller's pin did"
    );
    assert_ne!(
        pinned["registry_declared_version"].as_str(),
        Some("PIN-2026-08-04"),
        "the caller's pin was written into the registry's claim"
    );
}

/// The block a trace of forces cannot carry, asked of the caller and recorded whole.
///
/// The control is the same run stating nothing, which must report zero complete over the same
/// denominator. A guard over the stated side alone would pass on a surface that reported every
/// trial complete whatever it was told.
#[test]
fn a_run_that_states_its_capture_records_it_and_one_that_does_not_says_so() {
    let mut stated: Vec<&str> = Vec::new();
    for member in EVERY_MEMBER {
        stated.push("--acquisition");
        stated.push(member);
    }
    let described = run_record("acquisition-described", &stated);
    let silent = run_record("acquisition-silent", &[]);

    let computed = described["computed_count"].as_u64().expect("a count");
    assert!(computed > 0, "the run computed nothing to count");
    assert_eq!(
        described["acquisition_complete_count"].as_u64(),
        Some(computed),
        "every computed trial came off the plate the run described"
    );
    assert_eq!(described["acquisition_complete"].as_bool(), Some(true));
    assert_eq!(
        described["acquisition"]["floor_surface"].as_str(),
        Some("concrete"),
        "the record does not carry the block it was given"
    );

    assert_eq!(
        silent["computed_count"].as_u64(),
        Some(computed),
        "the two runs computed the same trials"
    );
    assert_eq!(silent["acquisition_complete_count"].as_u64(), Some(0));
    assert_eq!(silent["acquisition_complete"].as_bool(), Some(false));
    assert!(
        silent["acquisition"]["floor_surface"].is_null(),
        "a run that stated nothing recorded a floor"
    );
}

/// A run whose plate nobody recorded publishes no fingerprint, because it cannot be declared
/// to match another. A run that recorded one publishes its digest.
///
/// The two sides are the control for each other. Over the silent side alone the guard would
/// pass on a surface that published nothing ever; over the described side alone it would pass
/// on one that published a digest whatever the block held.
#[test]
fn only_a_run_that_recorded_its_plate_publishes_a_fingerprint() {
    let mut stated: Vec<&str> = Vec::new();
    for member in EVERY_MEMBER {
        stated.push("--acquisition");
        stated.push(member);
    }
    let described = run_record("fingerprint-described", &stated);
    let silent = run_record("fingerprint-silent", &[]);

    let published = described["run_fingerprint"]
        .as_str()
        .expect("a run that recorded its plate publishes a fingerprint");
    assert!(published.starts_with("content-"), "{published}");

    assert!(
        silent.get("run_fingerprint").is_some(),
        "the record does not carry run_fingerprint at all"
    );
    assert!(
        silent["run_fingerprint"].is_null(),
        "a run whose plate nobody recorded published {}, which the next such run would equal",
        silent["run_fingerprint"]
    );
}

/// The block is inside the fingerprint rather than beside it, so two runs off differently
/// configured plates are two runs.
#[test]
fn two_runs_off_different_plates_fingerprint_differently() {
    let one_plate: Vec<&str> = EVERY_MEMBER
        .iter()
        .flat_map(|member| ["--acquisition", member])
        .collect();
    let mut another_plate: Vec<&str> = Vec::new();
    for member in EVERY_MEMBER {
        another_plate.push("--acquisition");
        another_plate.push(if member.starts_with("floor_surface") {
            "floor_surface=sprung"
        } else {
            member
        });
    }

    let concrete = run_record("plate-concrete", &one_plate);
    let sprung = run_record("plate-sprung", &another_plate);

    // Both published one, so the inequality is between two digests rather than between two
    // runs that withheld theirs.
    assert!(concrete["run_fingerprint"].is_string());
    assert!(sprung["run_fingerprint"].is_string());
    assert_ne!(
        concrete["run_fingerprint"], sprung["run_fingerprint"],
        "the plate the trace came off did not reach the fingerprint"
    );
    // And the runs are otherwise identical, which is what makes the plate the only difference.
    assert_eq!(concrete["request_digest"], sprung["request_digest"]);
}

/// A member the block does not hold is refused by name against the five it declares, rather
/// than read and dropped. A value nobody stored is a default wearing the caller's signature.
#[test]
fn a_member_the_block_does_not_hold_is_refused() {
    let out_dir = scratch("acquisition-unknown");
    let output = batch(&out_dir, &["--acquisition", "debounce_ms=50"]);
    let said = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(THE_REQUEST_COULD_NOT_BE_READ),
        "{said}"
    );
    assert!(said.contains("debounce_ms"), "{said}");
    assert!(
        said.contains("firmware_version"),
        "the refusal does not name what the block does hold: {said}"
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}
