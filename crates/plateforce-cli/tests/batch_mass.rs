//! What a folder run takes for the athlete's mass, from the terminal.
//!
//! One flag covers one athlete and a squad, because a reader stating either is doing one
//! thing. Which of the two a run stated reaches one field of the record and never both, so a
//! reader meets one answer to the question of whose mass produced a number.
//!
//! The committed fixtures are one subject, which is the whole population this file can use.
//! The squad cases live in `plateforce-batch`, over generated traces.

use std::process::Output;

const PATTERN: &str = "subject{subject}_trial{trial}";

const A_FULLY_SPECIFIED_RUN: [&str; 20] = [
    "--registry",
    "../../registry",
    "batch",
    "../plateforce-conformance/fixtures",
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
    "--pattern",
    PATTERN,
];

const EVERY_VALUE_NAMED: [&str; 6] = [
    "--set",
    "weighing.duration=1.0",
    "--set",
    "onset.k=5",
    "--set",
    "takeoff.threshold_n=20",
];

/// Five of the six trials end before the athlete lands, so the flight-time height declines by
/// name. A run that wrote its table and could not produce one requested number exits here.
const A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER: i32 = 65;
const THE_LINE_CANNOT_BE_READ: i32 = 64;

fn scratch(name: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "plateforce-batch-mass-{name}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn run(out_dir: &std::path::Path, masses: &[&str]) -> Output {
    let mut line: Vec<String> = A_FULLY_SPECIFIED_RUN
        .iter()
        .chain(EVERY_VALUE_NAMED.iter())
        .map(|word| (*word).to_string())
        .collect();
    line.extend(["--out-dir".to_string(), out_dir.display().to_string()]);
    line.extend(masses.iter().map(|word| (*word).to_string()));
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&line)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn record(out_dir: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(out_dir.join("run.json")).expect("a record"))
        .expect("the record parses")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A mass keyed by the athlete the pattern named reaches the per-athlete field, and the
/// folder-wide list carries no mass at all.
#[test]
fn a_mass_keyed_by_athlete_is_recorded_against_that_athlete() {
    let out_dir = scratch("keyed");
    let output = run(&out_dir, &["--body-mass-kg", "01=58"]);
    assert_eq!(
        output.status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER),
        "{}",
        stderr_of(&output)
    );

    let run = record(&out_dir);
    let by_athlete = &run["body_mass_kilograms_by_subject"];
    println!("by athlete: {by_athlete}");
    assert_eq!(by_athlete["01"]["value"], 58.0);
    assert_eq!(by_athlete["01"]["unit"], "kilograms");
    assert_eq!(by_athlete["01"]["source"], "stated");

    let folder_wide: Vec<&str> = run["bound_globals"]
        .as_array()
        .expect("the run names what it was bound to")
        .iter()
        .filter_map(|bound| bound["name"].as_str())
        .filter(|name| name.contains("body_mass"))
        .collect();
    println!("folder-wide mass rows: {folder_wide:?}");
    assert!(folder_wide.is_empty(), "{folder_wide:?}");
}

/// The other shape, and the control that stops the guard above being met by a run that
/// records a mass nowhere.
#[test]
fn one_mass_for_the_folder_is_recorded_for_the_folder() {
    let out_dir = scratch("bare");
    let output = run(&out_dir, &["--body-mass-kg", "58"]);
    assert_eq!(
        output.status.code(),
        Some(A_TRIAL_COULD_NOT_PRODUCE_A_REQUESTED_NUMBER),
        "{}",
        stderr_of(&output)
    );

    let run = record(&out_dir);
    let folder_wide: Vec<f64> = run["bound_globals"]
        .as_array()
        .expect("the run names what it was bound to")
        .iter()
        .filter(|bound| {
            bound["name"]
                .as_str()
                .unwrap_or_default()
                .contains("body_mass")
        })
        .filter_map(|bound| bound["value"].as_f64())
        .collect();
    println!("folder-wide mass rows: {folder_wide:?}");
    assert_eq!(folder_wide, vec![58.0]);
    assert!(
        run["body_mass_kilograms_by_subject"].is_null(),
        "a folder of one athlete carried a per-athlete map as well"
    );
}

/// A mass written against a name the folder does not hold covers no trial. The refusal names
/// what the folder does hold, which is what a reader fixes the line with.
#[test]
fn a_mass_for_an_athlete_who_is_not_here_is_refused() {
    let out_dir = scratch("unknown");
    let output = run(&out_dir, &["--body-mass-kg", "99=58"]);
    let said = stderr_of(&output);
    println!("{said}");
    assert_eq!(
        output.status.code(),
        Some(THE_LINE_CANNOT_BE_READ),
        "{said}"
    );
    assert!(said.contains("99"), "{said}");
    assert!(said.contains("01"), "{said}");
    assert!(
        !out_dir.join("results.csv").exists(),
        "a refused run wrote a table"
    );
}

/// Both spellings on one line leave it unsaid which trials the folder-wide one covers.
#[test]
fn a_mass_for_the_folder_beside_one_by_athlete_is_refused() {
    let out_dir = scratch("mixed");
    let output = run(
        &out_dir,
        &["--body-mass-kg", "58", "--body-mass-kg", "01=58"],
    );
    let said = stderr_of(&output);
    println!("{said}");
    assert_eq!(
        output.status.code(),
        Some(THE_LINE_CANNOT_BE_READ),
        "{said}"
    );
    assert!(said.contains("--body-mass-kg"), "{said}");
}

/// A mass at or below zero divides into an infinity or flips the sign of every quantity
/// scaled by it, and it is refused in both spellings by the same check.
#[test]
fn a_mass_at_or_below_zero_is_refused_in_either_spelling() {
    for masses in [
        vec!["--body-mass-kg", "0"],
        vec!["--body-mass-kg", "01=0"],
        vec!["--body-mass-kg", "-58"],
        vec!["--body-mass-kg", "01=-58"],
    ] {
        let out_dir = scratch(&format!(
            "nonpositive-{}",
            masses[1].replace(['=', '-'], "_")
        ));
        let output = run(&out_dir, &masses);
        let said = stderr_of(&output);
        println!("{:?} -> {said}", masses[1]);
        assert_eq!(
            output.status.code(),
            Some(THE_LINE_CANNOT_BE_READ),
            "{said}"
        );
        assert!(said.contains("body_mass_kilograms"), "{said}");
    }
}

/// Every refusal this flag raises reads at eighty columns, which is the floor every common
/// terminal reaches.
#[test]
fn every_mass_refusal_reads_at_eighty_columns() {
    for masses in [
        vec!["--body-mass-kg", "99=58"],
        vec!["--body-mass-kg", "58", "--body-mass-kg", "01=58"],
        vec!["--body-mass-kg", "58", "--body-mass-kg", "61"],
        vec!["--body-mass-kg", "not-a-number"],
        vec!["--body-mass-kg", "=58"],
        vec!["--body-mass-kg", "01=58", "--body-mass-kg", "01=61"],
    ] {
        let out_dir = scratch(&format!("width-{}", masses.len()));
        let said = stderr_of(&run(&out_dir, &masses));
        assert!(!said.trim().is_empty(), "{masses:?} was not refused");
        for line in said.lines() {
            assert!(
                line.chars().count() <= 80,
                "{} columns on {masses:?}: {line}",
                line.chars().count()
            );
        }
    }
}
