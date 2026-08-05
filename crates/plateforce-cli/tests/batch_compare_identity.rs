//! What a comparison's digest identifies, held to the name it carries.
//!
//! The field used to be called `request_digest` beside a comment calling it the identity of
//! what ran. It is taken over the base request, which is what the sweep varied, so two
//! comparisons over one folder that swept different rules answered alike. On a run whose
//! acquisition block nobody filled, `run_fingerprint` is withheld by design, and this was the
//! only digest on the row.
//!
//! Both directions are here. The first says the digest is blind to the axis, which is what
//! `base_` now claims; the second says it is not blind to the base, which is the control that
//! stops the first being met by a constant.

use std::process::Output;

const HOW_THE_RUN_READS: [&str; 18] = [
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
    "--mode",
    "compare",
    "--weighing",
    "bwepoch.fixed_window",
    "--set",
    "weighing.duration=1.0",
];

const THE_RULES_HELD_STILL: [&str; 4] = [
    "--onset",
    "onset.threshold.noise_relative",
    "--takeoff",
    "takeoff.threshold.absolute_force",
];

fn scratch(name: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "plateforce-compare-identity-{name}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn compare(out_dir: &std::path::Path, extra: &[&str]) -> Output {
    let named = out_dir.display().to_string();
    let mut line: Vec<String> = HOW_THE_RUN_READS.iter().map(|w| w.to_string()).collect();
    line.extend(["--out-dir".to_string(), named]);
    line.extend(THE_RULES_HELD_STILL.iter().map(|w| w.to_string()));
    line.extend(extra.iter().map(|w| (*w).to_string()));
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&line)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// The record a comparison writes, with the run held to exiting cleanly first so a missing
/// file cannot read as a disagreeing digest.
fn record(name: &str, extra: &[&str]) -> serde_json::Value {
    let out_dir = scratch(name);
    let output = compare(&out_dir, extra);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(out_dir.join("compare-run.json")).expect("a record");
    let _ = std::fs::remove_dir_all(&out_dir);
    serde_json::from_str(&text).expect("the record parses")
}

fn swept(record: &serde_json::Value) -> Vec<String> {
    record["method_ids"]
        .as_array()
        .expect("the record names the rules it swept")
        .iter()
        .map(|id| id.as_str().unwrap_or_default().to_string())
        .collect()
}

fn digest(record: &serde_json::Value) -> String {
    record["base_request_digest"]
        .as_str()
        .expect("the record carries the digest of the request the sweep varied")
        .to_string()
}

/// Two sweeps of one base request over one folder. The digest names the base, so it is the
/// same; what tells the two runs apart is beside it.
#[test]
fn two_axes_over_one_base_request_share_the_base_digest() {
    let one = record(
        "axis-absolute",
        &[
            "--set",
            "onset.k=5",
            "--against",
            "onset.threshold.absolute_force",
        ],
    );
    let other = record(
        "axis-relative",
        &[
            "--set",
            "onset.k=5",
            "--against",
            "onset.threshold.relative_to_system_weight",
        ],
    );

    println!("one   swept {:?} under {}", swept(&one), digest(&one));
    println!("other swept {:?} under {}", swept(&other), digest(&other));
    assert_ne!(
        swept(&one),
        swept(&other),
        "these two runs were meant to sweep different rules"
    );
    assert_eq!(
        digest(&one),
        digest(&other),
        "the base digest moved with the axis, so it is not the base it is named for"
    );
}

/// The control. A digest that never moved would satisfy the guard above, so this moves the
/// base and requires it to follow.
#[test]
fn moving_the_base_request_moves_the_base_digest() {
    let axis = ["--against", "onset.threshold.absolute_force"];
    let mut at_five = vec!["--set", "onset.k=5"];
    at_five.extend(axis);
    let mut at_three = vec!["--set", "onset.k=3"];
    at_three.extend(axis);

    let five = record("base-five", &at_five);
    let three = record("base-three", &at_three);

    println!("k=5 under {}", digest(&five));
    println!("k=3 under {}", digest(&three));
    assert_eq!(
        swept(&five),
        swept(&three),
        "both runs sweep one axis, so only the base separates them"
    );
    assert_ne!(
        digest(&five),
        digest(&three),
        "a value the base request states did not reach the digest named after it"
    );
}
