//! A plate recorded once fills the acquisition block of every run that names it.
//!
//! Asking five questions at every analysis asks the same answers hundreds of times, and that is
//! what leaves a block short of a member. What a saved plate must never become is a second home
//! for the fact: every run below is checked for the members themselves, in its own record, so a
//! result that travelled away from the machine that produced it carries the plate it came off.

use std::path::PathBuf;
use std::process::Command;

const TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// Every member and the answer this plate holds. Written out rather than read from
/// `Acquisition::MEMBERS`, for the reason the sibling file states: a test that builds its input
/// from the constant the parser validates against agrees with itself however the block changes.
const EVERY_MEMBER: [(&str, &str); 5] = [
    ("filter_at_capture", "none"),
    ("tare_state", "tared"),
    ("plate_natural_frequency_hz", "400"),
    ("floor_surface", "concrete"),
    ("firmware_version", "2.1"),
];

/// A plates folder this test owns. Never the machine's own, which a test that wrote into it
/// would edit for whoever ran it.
struct Plates(PathBuf);

impl Plates {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "plateforce-plate-guard-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn folder(&self) -> String {
        self.0.display().to_string()
    }

    /// One plate saved, as the decree spells it.
    fn save(&self, name: &str, members: &[(&str, &str)]) -> String {
        let mut command = Command::new(env!("CARGO_BIN_EXE_plateforce"));
        command.args([
            "--plates",
            &self.folder(),
            "--format",
            "json",
            "plate",
            "save",
            name,
        ]);
        for (member, value) in members {
            command.args(["--acquisition", &format!("{member}={value}")]);
        }
        let output = command
            .env("NO_COLOR", "1")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("the terminal runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let saved: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("the result is json");
        saved["ok"]["revision"]
            .as_str()
            .expect("a saved plate has a revision")
            .to_string()
    }

    /// One analysis, returning both streams so a refusal is readable too.
    fn analyse(&self, extra: &[&str]) -> (String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_plateforce"));
        command.args([
            "--plates",
            &self.folder(),
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
        command.args(extra);
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
}

impl Drop for Plates {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn result_in(document: &str) -> serde_json::Value {
    let parsed: serde_json::Value = serde_json::from_str(document)
        .unwrap_or_else(|error| panic!("the result is json: {error}\n{document}"));
    parsed["ok"].clone()
}

/// The whole point, in the shape the decree names it: five answers recorded once, and a later
/// run that types none of them fingerprints as complete and carries every one of them.
#[test]
fn a_run_that_names_a_saved_plate_carries_its_members_and_fingerprints_complete() {
    let plates = Plates::new("carries");
    plates.save("lab-kistler-1", &EVERY_MEMBER);

    let (document, _) = plates.analyse(&["--plate", "lab-kistler-1"]);
    let result = result_in(&document);

    assert_eq!(
        result["acquisition_complete"].as_bool(),
        Some(true),
        "a run filled from a complete plate did not fingerprint as complete\n{document}"
    );
    for (member, value) in EVERY_MEMBER {
        let written = &result["acquisition"][member];
        let carried = written.as_str().map(str::to_string).or_else(|| {
            written.as_f64().map(|number| {
                // The one member the block holds as a number, compared as the answer rather than
                // as its spelling, so 400 and 400.0 are the same answer.
                format!("{number}")
            })
        });
        assert_eq!(
            carried.as_deref(),
            Some(value),
            "the record does not carry {member}, so the plate is a place the result points at\n{document}"
        );
    }
    assert_eq!(
        result["plate_profile"]["name"].as_str(),
        Some("lab-kistler-1"),
        "the record does not name the plate it was filled from\n{document}"
    );
}

/// A member written beside the plate is the answer that runs, and what it replaced reaches the
/// record. The second half is the load-bearing one: without it two runs off one plate differ
/// with nothing in either record saying why.
#[test]
fn a_member_stated_beside_a_plate_wins_and_the_record_says_what_it_replaced() {
    let plates = Plates::new("stated-beside");
    plates.save("lab-kistler-1", &EVERY_MEMBER);

    let (document, _) = plates.analyse(&[
        "--plate",
        "lab-kistler-1",
        "--acquisition",
        "firmware_version=2.2",
    ]);
    let result = result_in(&document);

    assert_eq!(
        result["acquisition"]["firmware_version"].as_str(),
        Some("2.2"),
        "the saved answer ran where the caller stated one\n{document}"
    );
    assert_eq!(
        result["plate_profile"]["superseded_members"]["firmware_version"].as_str(),
        Some("2.1"),
        "the record does not say what the stated member replaced\n{document}"
    );
}

/// The stale-fact risk this shape accepts, made visible rather than impossible. Two results off
/// one plate name, taken either side of an edit, carry different members and different
/// revisions, so a reader holding both can see that the plate moved under them.
#[test]
fn two_results_off_one_plate_name_show_that_the_plate_was_edited() {
    let plates = Plates::new("edited");
    let before = plates.save("lab-kistler-1", &EVERY_MEMBER);
    let (first, _) = plates.analyse(&["--plate", "lab-kistler-1"]);

    let mut edited = EVERY_MEMBER;
    edited[4] = ("firmware_version", "2.2");
    let after = plates.save("lab-kistler-1", &edited);
    let (second, _) = plates.analyse(&["--plate", "lab-kistler-1"]);

    assert_ne!(
        before, after,
        "an edited plate hashed to the revision it held before"
    );
    assert_eq!(
        result_in(&first)["plate_profile"]["revision"].as_str(),
        Some(before.as_str()),
        "{first}"
    );
    assert_eq!(
        result_in(&second)["plate_profile"]["revision"].as_str(),
        Some(after.as_str()),
        "{second}"
    );
    assert_ne!(
        result_in(&first)["acquisition"],
        result_in(&second)["acquisition"],
        "the two records carry the same members, so the edit reached neither"
    );
}

/// A plate this machine has no record of is answered by name rather than run under an empty
/// block, which would be a result quietly computed against settings nobody stated.
#[test]
fn a_plate_nobody_saved_is_refused_by_name() {
    let plates = Plates::new("absent");
    plates.save("lab-kistler-1", &EVERY_MEMBER);

    let (document, refusal) = plates.analyse(&["--plate", "lab-kistler-9"]);

    assert!(
        document.is_empty(),
        "a run named a plate nobody saved and produced a result\n{document}"
    );
    assert!(refusal.contains("lab-kistler-9"), "{refusal}");
}
