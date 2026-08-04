//! The spread panel varies the rule that computes the quantity, and says which choices its
//! figure is a spread over.
//!
//! An `axes_over_every_rule` that iterates the three landmark constructs never varies the
//! arithmetic. On the shipped fixture that reports 3.11 cm for jump height while the rules that
//! report that key span 3.38 cm, and a maximum of 0.41585 against a published rule answering
//! 0.44436 for the same quantity: a minimum, a maximum and a median that exclude a published
//! answer to the question the panel is about.

use std::process::Output;

/// The rule the spine runs for this quantity when the request names none, and two published
/// alternatives filed under the same construct that report the same key.
const SPINE_DEFAULT: &str = "jumpheight.takeoff.impulse_momentum";
const CHAVDA: &str = "jumpheight.takeoff.peak_velocity.chavda2018";
const WORK_ENERGY: &str = "jumpheight.takeoff.work_energy";
const QUANTITY: &str = "jump_height_from_takeoff_meters";

fn spread(extra: &[&str]) -> Output {
    run_spread(&["--format", "json"], extra)
}

/// The same run rendered as the table a terminal reader meets.
fn spread_table(extra: &[&str]) -> Output {
    run_spread(&[], extra)
}

fn run_spread(before_subcommand: &[&str], extra: &[&str]) -> Output {
    let mut arguments: Vec<&str> = vec!["--registry", "../../registry"];
    arguments.extend_from_slice(before_subcommand);
    arguments.extend_from_slice(&[
        "spread",
        "../plateforce-conformance/fixtures/subject01_trial1.force.txt",
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
        "--set",
        "takeoff.threshold_n=20",
        "--quantity",
        QUANTITY,
    ]);
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn body(output: &Output) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the panel answers JSON");
    parsed
        .get("ok")
        .cloned()
        .unwrap_or_else(|| panic!("a refusal rather than a result: {parsed}"))
}

/// What one rule answers for this quantity on this trial, run on its own.
fn value_under(rule: &str) -> f64 {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
            "--registry",
            "../../registry",
            "--format",
            "json",
            "analyse",
            "../plateforce-conformance/fixtures/subject01_trial1.force.txt",
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
            "--set",
            "takeoff.threshold_n=20",
            "--derive",
            &format!("jump_height.takeoff_frame={rule}"),
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    parsed["ok"]["metrics"]
        .as_array()
        .expect("metrics")
        .iter()
        .find(|metric| metric["key"] == QUANTITY)
        .and_then(|metric| metric["value"].as_f64())
        .unwrap_or_else(|| panic!("{rule} reported no {QUANTITY}"))
}

/// The assertion that discriminates.
///
/// A test that the spread is non-zero passes on the defect: the landmark axes alone produce
/// 3.11 cm. This asserts the reported range **contains every value a bound rule for this key
/// produces**, which is what the panel's minimum and maximum claim to bound. `chavda2018` at
/// 0.44436 sat outside 0.38479 to 0.41585, on the shipped fixture, with no synthetic trace.
#[test]
fn the_reported_range_contains_every_value_a_rule_for_this_quantity_produces() {
    let answers: Vec<(&str, f64)> = [SPINE_DEFAULT, CHAVDA, WORK_ENERGY]
        .into_iter()
        .map(|rule| (rule, value_under(rule)))
        .collect();
    for (rule, value) in &answers {
        println!("  {rule} answers {value:.7}");
    }
    // The population has to contain the interesting case, or this guard proves nothing. Two
    // rules agreeing to seven places would make any range look complete.
    let lowest = answers.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
    let highest = answers.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
    assert!(
        highest - lowest > 0.01,
        "the rules agree to within {} m, so this trial cannot discriminate",
        highest - lowest
    );

    let panel = body(&spread(&[
        "--derive",
        &format!("jump_height.takeoff_frame={SPINE_DEFAULT}"),
    ]));
    let minimum = panel["minimum"].as_f64().expect("a minimum");
    let maximum = panel["maximum"].as_f64().expect("a maximum");
    println!("  panel reports {minimum:.5} to {maximum:.5}");

    for (rule, value) in &answers {
        assert!(
            *value >= minimum && *value <= maximum,
            "{rule} answers {value:.7} for {QUANTITY}, outside the {minimum:.5} to {maximum:.5} the panel reports as the spread"
        );
    }
}

/// Varying the computing construct changes the figure. Without this the guard above could be
/// satisfied by a range widened for any other reason.
#[test]
fn sweeping_the_computing_construct_moves_the_reported_spread() {
    let without = body(&spread(&[]));
    let with = body(&spread(&[
        "--derive",
        &format!("jump_height.takeoff_frame={SPINE_DEFAULT}"),
    ]));

    let narrow = without["spread_absolute"].as_f64().expect("a spread");
    let wide = with["spread_absolute"].as_f64().expect("a spread");
    println!(
        "  {} combinations gave {narrow:.5} m, {} gave {wide:.5} m",
        without["combinations_run"], with["combinations_run"]
    );
    assert!(
        wide > narrow,
        "binding the arithmetic did not widen the spread: {narrow:.5} against {wide:.5}"
    );
}

/// The record answers the question either way.
///
/// A run that varied the arithmetic says so; a run that did not names the rule it held and the
/// construct it belongs to. The second is the harder half, because the spine runs that rule
/// under its own default and the request names it nowhere.
#[test]
fn the_record_names_what_varied_and_what_stood_still() {
    let without = body(&spread(&[]));
    let varied: Vec<&str> = without["axes_varied"]
        .as_array()
        .expect("axes")
        .iter()
        .filter(|axis| axis["rules_varied"].as_u64().unwrap_or(0) > 1)
        .map(|axis| axis["construct"].as_str().expect("a construct"))
        .collect();
    let held: Vec<(&str, &str)> = without["held_fixed"]
        .as_array()
        .expect("held")
        .iter()
        .map(|rule| {
            (
                rule["construct"].as_str().expect("a construct"),
                rule["method_id"].as_str().expect("an id"),
            )
        })
        .collect();
    println!("  varied {varied:?}");
    println!("  held {held:?}");

    assert!(varied.contains(&"movement_onset"), "{varied:?}");
    assert!(
        !varied.contains(&"jump_height.takeoff_frame"),
        "this run did not vary the arithmetic: {varied:?}"
    );
    assert!(
        held.contains(&("jump_height.takeoff_frame", SPINE_DEFAULT)),
        "a reader holding this figure cannot see that the computing rule stood still: {held:?}"
    );

    // And the other direction, so the record is not merely a constant.
    let with = body(&spread(&[
        "--derive",
        &format!("jump_height.takeoff_frame={SPINE_DEFAULT}"),
    ]));
    let now_varied: Vec<&str> = with["axes_varied"]
        .as_array()
        .expect("axes")
        .iter()
        .filter(|axis| axis["rules_varied"].as_u64().unwrap_or(0) > 1)
        .map(|axis| axis["construct"].as_str().expect("a construct"))
        .collect();
    assert!(
        now_varied.contains(&"jump_height.takeoff_frame"),
        "{now_varied:?}"
    );
    assert!(
        with["held_fixed"].as_array().expect("held").is_empty(),
        "nothing was held: {}",
        with["held_fixed"]
    );
}

/// Every line of the panel reads at eighty columns, the width a redirected document renders
/// at, and the held sentence survives the wrapping whole.
#[test]
fn the_held_line_reads_at_eighty_columns() {
    let output = spread_table(&[]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        assert!(
            line.chars().count() <= 80,
            "{} columns: {line}",
            line.chars().count()
        );
    }
    let unwrapped = stdout.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        unwrapped.contains("so this spread is not over it"),
        "the held sentence is absent:\n{stdout}"
    );
}
