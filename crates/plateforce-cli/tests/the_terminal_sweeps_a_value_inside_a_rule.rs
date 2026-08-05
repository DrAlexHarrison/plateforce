//! A sweep varies the choice a reader would otherwise make once. Which rule runs is one such
//! choice and the number the rule reads is another, and the terminal states both.
//!
//! Measured on subject 01 trial 1: the six published values of `onset.k` move the jump height
//! 0.0198 m, and the five onset rules move it 0.0193 m. So the value inside one rule moves the
//! number as far as the choice of rule does, and until this flag a terminal user could ask
//! only the smaller of the two questions. The notebook and R take `parameter` and `values` for
//! it and have since they shipped.

use std::process::Output;

const QUANTITY: &str = "jump_height_from_takeoff_meters";

/// The values `onset.threshold.noise_relative` is published at, which is the set a reader
/// sweeping `k` means.
const PUBLISHED_K: &str = "2,2.5,3,4,5,8";

fn run(before_subcommand: &[&str], extra: &[&str]) -> Output {
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

fn record(extra: &[&str]) -> serde_json::Value {
    let output = run(&["--format", "json"], extra);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the sweep answers JSON");
    parsed
        .get("ok")
        .cloned()
        .unwrap_or_else(|| panic!("a refusal rather than a result: {parsed}"))
}

fn panel(extra: &[&str]) -> String {
    let output = run(&[], extra);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A line that refused, with the exit status read outside any pipe.
fn refusal(extra: &[&str]) -> (i32, String) {
    let output = run(&[], extra);
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().expect("the process exited"), said)
}

#[test]
fn a_value_the_registry_publishes_several_ways_can_be_swept_from_the_terminal() {
    let swept = record(&["--vary", &format!("onset.k={PUBLISHED_K}")]);
    let spread = swept["spread_absolute"].as_f64().expect("a spread");
    println!(
        "  {} of {} combinations produced a value, spanning {spread:.5} m",
        swept["succeeded"], swept["combinations_run"]
    );

    assert_eq!(swept["combinations_run"].as_u64(), Some(6));
    assert_eq!(swept["succeeded"].as_u64(), Some(6));

    // The control on the assertion under it. A sweep whose values did not reach the rule
    // would report zero here, which is the shape a swept knob that moves nothing takes.
    assert!(
        spread > 0.0,
        "six published values of k produced one number, so nothing was swept"
    );

    // Against the choice of rule, on the same trial, because the claim is that the value
    // inside a rule is a choice of the same size as the choice of rule and neither figure is
    // written here.
    let over_the_rules = record(&["--slot", "onset"]);
    let by_rule = over_the_rules["spread_absolute"]
        .as_f64()
        .expect("a spread");
    println!(
        "  the {} onset rules span {by_rule:.5} m",
        over_the_rules["combinations_run"]
    );
    assert!(
        spread > by_rule / 2.0,
        "k spans {spread:.5} m against {by_rule:.5} m for the rule, so this trial does not \
         show a value choice to be the size of a rule choice"
    );
}

/// The record says a value moved, and which one.
#[test]
fn the_record_names_the_value_it_varied_and_the_rule_it_held() {
    let swept = record(&["--vary", &format!("onset.k={PUBLISHED_K}")]);
    let axes = swept["axes_varied"].as_array().expect("axes");
    println!("  axes_varied {}", swept["axes_varied"]);

    assert_eq!(axes.len(), 1, "{:?}", swept["axes_varied"]);
    let axis = &axes[0];
    assert_eq!(axis["construct"].as_str(), Some("movement_onset"));
    assert_eq!(axis["parameter"].as_str(), Some("k"));
    assert_eq!(axis["values_varied"].as_u64(), Some(6));
    assert_eq!(axis["rules_varied"].as_u64(), Some(0));

    // The rule stood still while its value moved, and a reader of this figure can see both.
    let held: Vec<(&str, &str)> = swept["held_fixed"]
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
    assert!(
        held.contains(&("movement_onset", "onset.threshold.noise_relative")),
        "{held:?}"
    );
}

/// The panel a terminal reader meets says what moved.
///
/// An axis over a value carries no rules, so a panel filtering on the rule count printed the
/// figure, the two ends and four held lines, and never named the choice the figure is a spread
/// over. The two ends named the same three rules twice, which reads as a disagreement between
/// a rule and a copy of it.
#[test]
fn the_panel_names_the_value_that_moved_rather_than_the_rules_that_did_not() {
    let printed = panel(&["--vary", &format!("onset.k={PUBLISHED_K}")]);
    println!("{printed}");
    let unwrapped = printed.split_whitespace().collect::<Vec<_>>().join(" ");

    assert!(
        unwrapped.contains("varied movement_onset.k (6 values)"),
        "the panel does not say what it varied:\n{printed}"
    );
    // The ends are the values, because every combination ran the same rules and printing them
    // would name one identical list against each end.
    assert!(
        unwrapped.contains("lowest 0.3915 m k 2") && unwrapped.contains("highest 0.4113 m k 8"),
        "the ends do not name the values that produced them:\n{printed}"
    );
    for line in printed.lines() {
        assert!(
            line.chars().count() <= 80,
            "{} columns: {line}",
            line.chars().count()
        );
    }
}

/// Gravity is a value of the run rather than of any rule, and the engine offers it as an axis.
/// A reader who bound it with `--gravity` sweeps it by name.
#[test]
fn gravity_is_a_value_this_surface_can_sweep() {
    let swept = record(&[
        "--vary",
        "global.gravity_meters_per_second_squared=9.76,9.81",
    ]);
    println!(
        "  {} combinations spanning {} m",
        swept["combinations_run"], swept["spread_absolute"]
    );
    assert_eq!(swept["combinations_run"].as_u64(), Some(2));
    assert!(swept["spread_absolute"].as_f64().expect("a spread") > 0.0);
}

/// Every way of writing a line this flag cannot act on, refused by name.
///
/// A run that varied nothing and reported a spread of zero is the failure this whole command
/// exists to publish, so each of these is a refusal rather than a default.
#[test]
fn a_line_this_flag_cannot_act_on_is_refused_by_name() {
    for (extra, expected) in [
        (
            vec!["--vary", "onset.k=2,5", "--slot", "onset"],
            "--slot varies which rule runs and --vary varies a value inside one",
        ),
        (
            vec!["--vary", "onset.k=2,5", "--vary", "weighing.duration=1,2"],
            "a sweep varies one value, and this run named onset.k and weighing.duration",
        ),
        (
            vec!["--vary", "onset.k=2,2"],
            "--vary onset.k names 2 twice, and one value is one variant",
        ),
        (
            vec!["--vary", "onset.k=fast"],
            "--vary onset.k was given 'fast', which is not a number",
        ),
        (
            vec!["--vary", "nonsense.k=2,5"],
            "--vary nonsense.k names no step of this run",
        ),
        (
            vec!["--vary", "onset.k"],
            "--vary takes <slot>.<name>=<value>,<value>, and 'onset.k' carries no =",
        ),
    ] {
        let (code, said) = refusal(&extra);
        println!("  {extra:?} -> {code}: {}", said.trim());
        assert_eq!(code, 64, "{extra:?} exited {code}: {said}");
        assert!(said.contains(expected), "{extra:?} said: {said}");
    }
}
