//! What a folder comparison varies, and what its record says produced the numbers.
//!
//! The founding measurement of this project is a spread across published onset rules over 244
//! trials. The surface that runs 244 trials swept exactly that construct and no other, so the
//! one step it could compare was the one that measurement happens to need. These guards are
//! from the terminal rather than from the library, because the library reaching every step
//! proves nothing about whether the flags reach the library.
//!
//! Every guard is paired with a control that comes back the other way. A comparison whose
//! numbers do not move reads identically whether the rules agree or the sweep never reached
//! them, and this project has been caught by that shape before.

use std::process::Output;

const THE_REQUEST_COULD_NOT_BE_READ: i32 = 64;

/// Subject 01, the only athlete whose traces are public, and two synthetic recordings named as
/// synthetic. The pattern names six of the eight.
fn compare(out_dir: &std::path::Path, extra: &[&str]) -> Output {
    let named = out_dir.display().to_string();
    let mut arguments: Vec<&str> = vec![
        "--registry",
        "../../registry",
        "batch",
        "../plateforce-conformance/fixtures",
        "--out-dir",
        &named,
        "--mode",
        "compare",
        "--trial-suffix",
        ".force.txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--pattern",
        "subject{subject}_trial{trial}",
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
        std::env::temp_dir().join(format!("plateforce-compare-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

/// The record and the paired values from one comparison.
fn swept(name: &str, extra: &[&str]) -> (serde_json::Value, Vec<String>) {
    let out_dir = scratch(name);
    let output = compare(&out_dir, extra);
    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("compare-run.json")).unwrap())
            .expect("the record parses");
    let paired = std::fs::read_to_string(out_dir.join("paired.csv")).expect("the table is written");
    // The value column, off the header rather than by a position written here.
    let header: Vec<&str> = paired.lines().next().unwrap().split(',').collect();
    let column = header
        .iter()
        .position(|name| *name == "value")
        .expect("the table has a value column");
    let values: Vec<String> = paired
        .lines()
        .skip(1)
        .filter_map(|line| line.split(',').nth(column).map(str::to_string))
        .collect();
    let _ = std::fs::remove_dir_all(&out_dir);
    (record, values)
}

/// The lane's own subject. Three steps named from the terminal, three answers.
///
/// The controls are each other. A surface pinned to onset returns the onset answer, or no
/// answer, whichever rules the caller names, and both fail here.
#[test]
fn naming_a_takeoff_or_a_weighing_rule_sweeps_that_step_and_not_onset() {
    let steps = [
        (
            "weighing",
            "system_weight",
            "bwepoch.adaptive_lowest_variance",
        ),
        (
            "onset",
            "movement_onset",
            "onset.threshold.relative_to_system_weight",
        ),
        ("takeoff", "takeoff", "takeoff.threshold.flight_noise_k_sd"),
    ];
    let mut answers: Vec<(&str, Vec<String>)> = Vec::new();
    for (slot, construct, rule) in steps {
        let (record, values) = swept(slot, &["--against", rule]);
        assert_eq!(record["slot"].as_str(), Some(slot));
        // Named as the registry declares it, so a reader can look the step up. The registry
        // declares no `weighing` and no `onset`.
        assert_eq!(record["construct"].as_str(), Some(construct));
        assert_eq!(
            record["method_ids"][1].as_str(),
            Some(rule),
            "the rule the caller wrote is the one compared against"
        );
        assert_eq!(values.len(), 12, "2 rules over 6 trials");
        answers.push((slot, values));
    }

    println!(
        "subject 01, 6 trials: {}",
        answers
            .iter()
            .map(|(slot, values)| format!("{slot} {} rows", values.len()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (index, (slot, values)) in answers.iter().enumerate() {
        for (other, others) in answers.iter().skip(index + 1) {
            assert_ne!(
                values, others,
                "sweeping {slot} and sweeping {other} produced the same numbers"
            );
        }
    }
}

/// What the record says the comparison held still, which is the other half of what the figure
/// is over.
///
/// A spread across takeoff rules taken under one onset rule is not a spread across the run,
/// and the rules that stood still are recoverable from nothing else in the folder.
#[test]
fn the_record_names_what_the_comparison_held_as_well_as_what_it_varied() {
    let (record, _) = swept(
        "held",
        &["--against", "takeoff.threshold.flight_noise_k_sd"],
    );
    let held: Vec<&str> = record["held_fixed"]
        .as_array()
        .expect("the record carries what it held")
        .iter()
        .map(|rule| rule["construct"].as_str().unwrap_or_default())
        .collect();
    println!("swept takeoff, held {held:?}");
    assert!(held.contains(&"movement_onset"), "{held:?}");
    assert!(held.contains(&"system_weight"), "{held:?}");
    assert!(
        !held.contains(&"takeoff"),
        "the step it swept is listed as held: {held:?}"
    );
}

/// The plate and the pin a comparison was given, on the record it leaves behind.
///
/// The control is the same comparison without the flags, because a record that writes null for
/// a plate nobody gave and one that writes null for a plate it was given read identically from
/// the reader's side.
#[test]
fn a_comparison_records_the_plate_and_the_revision_its_caller_stated() {
    let stated = [
        "--registry-version",
        "PIN-2026-08-04",
        "--acquisition",
        "filter_at_capture=none",
        "--acquisition",
        "tare_state=tared_before_trial",
        "--acquisition",
        "plate_natural_frequency_hz=400",
        "--acquisition",
        "floor_surface=concrete",
        "--acquisition",
        "firmware_version=2.4.1",
    ];
    let mut with_plate = vec!["--against", "onset.threshold.absolute_force"];
    with_plate.extend(stated);
    let (told, _) = swept("told", &with_plate);
    let (untold, _) = swept("untold", &["--against", "onset.threshold.absolute_force"]);

    assert_eq!(told["registry_version"].as_str(), Some("PIN-2026-08-04"));
    assert!(
        untold.get("registry_version").is_some() && untold["registry_version"].is_null(),
        "an unpinned comparison recorded {}, which reads as a revision somebody chose",
        untold["registry_version"]
    );
    // The registry's own claim, under its own name rather than under the caller's.
    assert!(
        told["registry_declared_version"].is_string(),
        "the record names no registry at all"
    );

    assert_eq!(
        told["acquisition"]["floor_surface"].as_str(),
        Some("concrete"),
        "the block the caller stated is not on the record"
    );
    assert_eq!(told["acquisition_complete"].as_bool(), Some(true));
    assert!(untold["acquisition"]["floor_surface"].is_null());
    assert_eq!(untold["acquisition_complete"].as_bool(), Some(false));

    // A filled block fingerprints and an unfilled one is withheld, so a comparison nobody can
    // declare a match for carries nothing that could be mistaken for one.
    println!(
        "fingerprint told {:?}, untold {:?}",
        told["run_fingerprint"], untold["run_fingerprint"]
    );
    assert!(told["run_fingerprint"].is_string());
    assert!(untold["run_fingerprint"].is_null());

    // How the traces were read, which no recording in that folder states and which moves every
    // number after it. 1200 Hz read as 1000 moves velocity, displacement and impulse by 20%.
    assert_eq!(told["format"]["sample_rate_hz"].as_f64(), Some(1200.0));
    assert_eq!(told["format"]["force_column_index"].as_u64(), Some(0));
}

/// What a comparison says when the name it was given is not a rule it can run.
///
/// Each of the three sends the caller somewhere that works. The assertions are on what the
/// sentence does not say as much as on what it does, because every one of these read as a
/// plausible refusal while pointing at a flag that does not exist.
#[test]
fn a_comparison_refuses_a_name_it_cannot_run_by_naming_what_it_takes() {
    let refused = |extra: &[&str]| -> String {
        let out_dir = scratch("refusal");
        let output = compare(&out_dir, extra);
        assert_eq!(
            output.status.code().unwrap_or(-1),
            THE_REQUEST_COULD_NOT_BE_READ,
            "the comparison ran"
        );
        let said = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = std::fs::remove_dir_all(&out_dir);
        said
    };

    // A step written where a rule goes, which is the slip between `spread --slot takeoff` and
    // `--against`. It gets that step's rules rather than every rule in the build.
    let step_word = refused(&["--against", "takeoff"]);
    println!("{}", step_word.trim());
    assert!(
        step_word.contains("takeoff.threshold.longest_run"),
        "{step_word}"
    );
    assert!(!step_word.contains("this comparison"), "{step_word}");
    assert!(
        !step_word.contains("onset.threshold.noise_relative"),
        "an onset rule is offered as a takeoff rule: {step_word}"
    );

    // A name that is neither. Every rule, under a sentence that claims no step for them.
    let nothing = refused(&["--against", "not.a.rule"]);
    println!("{}", nothing.trim());
    assert!(nothing.contains("answers to no rule"), "{nothing}");
    assert!(!nothing.contains("that step"), "{nothing}");

    // A step this build runs a rule for that this run bound nothing for. One way in, and it is
    // the one that works: `--derive` takes this construct and no `--peak_force` flag exists.
    let unbound = refused(&["--against", "force.peak.gross"]);
    println!("{}", unbound.trim());
    assert!(
        unbound.contains("--derive peak_force=<method>"),
        "{unbound}"
    );
    assert!(!unbound.contains("--peak_force "), "{unbound}");

    // Rules from two steps is two comparisons, and neither is run silently.
    let two = refused(&[
        "--against",
        "onset.threshold.absolute_force",
        "--against",
        "takeoff.threshold.longest_run",
    ]);
    println!("{}", two.trim());
    assert!(
        two.contains("movement_onset") && two.contains("takeoff"),
        "{two}"
    );
}
