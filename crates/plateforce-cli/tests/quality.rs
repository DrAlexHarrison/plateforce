//! What the software knows about a number it prints, on the surface whose numbers get
//! pasted into a paper.
//!
//! A value the browser flags and the terminal does not is a confident wrong number reaching
//! a reader through the quieter of two doors, so the assertion is that the signal travels in
//! both bodies and sits beside the value it is about.

use std::process::Command;

/// A recording holding a step off the plate before the jump, where takeoff lands on the
/// step-off and the impulse route to a height returns 0.87 mm against 45.4 cm from flight
/// time. `MISSION.md` P5 names this recording as the pillar's own test.
const TRACE_WHOSE_ROUTES_DISAGREE: &str =
    "../plateforce-conformance/fixtures/synthetic_untrimmed_step_off.force.txt";

/// One trimmed jump, where the two routes sit 3 cm apart and the signal has nothing to say.
const TRACE_WHOSE_ROUTES_AGREE: &str =
    "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn analyse(fixture: &str, format: &str) -> (String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
            "--registry",
            "../../registry",
            "--format",
            format,
            "analyse",
            fixture,
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
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
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    (
        String::from_utf8(output.stdout).expect("the document is UTF-8"),
        output.status.code(),
    )
}

#[test]
fn a_pipe_reading_the_result_can_branch_on_what_the_software_doubts() {
    let (document, _) = analyse(TRACE_WHOSE_ROUTES_DISAGREE, "json");
    let parsed: serde_json::Value = serde_json::from_str(&document).expect("the result parses");
    let signals = parsed["ok"]["signals"]
        .as_array()
        .expect("a result carries its signals");
    println!("signals on this trial: {}", signals.len());
    assert_eq!(signals.len(), 1, "{signals:#?}");

    let signal = &signals[0];
    assert_eq!(signal["status"], "disagrees");
    assert!(
        signal["value"].as_f64().expect("a computed value") > signal["threshold"].as_f64().unwrap()
    );
    let qualifies: Vec<&str> = signal["qualifies"]
        .as_array()
        .expect("a signal names what it qualifies")
        .iter()
        .map(|key| key.as_str().expect("a metric key"))
        .collect();
    assert!(
        qualifies.contains(&"jump_height_from_takeoff_meters"),
        "{qualifies:?}"
    );
}

/// Beside the number rather than in a block at the end, because a reader scanning the values
/// does not go to the end.
#[test]
fn the_signal_sits_between_the_value_it_qualifies_and_the_next_one() {
    let (document, _) = analyse(TRACE_WHOSE_ROUTES_DISAGREE, "text");
    let lines: Vec<&str> = document.lines().collect();
    let qualified = lines
        .iter()
        .position(|line| line.contains("Jump height, takeoff frame"))
        .expect("the qualified metric is printed");
    let next_metric = lines
        .iter()
        .position(|line| line.contains("Jump height, flight time"))
        .expect("the following metric is printed");
    let said = lines
        .iter()
        .position(|line| line.contains("past 20.0 percent"))
        .expect("the signal is printed");
    println!("qualified at {qualified}, signal at {said}, next metric at {next_metric}");
    assert!(qualified < said && said < next_metric);
}

/// One signal over two qualifying metrics is said once. Saying it under each would read as
/// two findings about one comparison.
#[test]
fn a_signal_over_two_metrics_is_said_once() {
    let (document, _) = analyse(TRACE_WHOSE_ROUTES_DISAGREE, "text");
    let times = document.matches("past 20.0 percent").count();
    println!("times the signal is said: {times}");
    assert_eq!(times, 1);
}

/// A signal is not a refusal. The number is a number, and a shell that treated the doubt as
/// a failure would drop a result the software stands behind.
#[test]
fn a_signal_does_not_move_the_exit_code() {
    let (_, code) = analyse(TRACE_WHOSE_ROUTES_DISAGREE, "json");
    assert_eq!(code, Some(0));
}

/// The control: a trace whose two routes agree produces no signal, so a passing assertion
/// above is about this recording's disagreement rather than about a signal that always fires.
#[test]
fn a_trace_whose_routes_agree_says_nothing() {
    let (document, code) = analyse(TRACE_WHOSE_ROUTES_AGREE, "json");
    let parsed: serde_json::Value = serde_json::from_str(&document).expect("the result parses");
    let signals = parsed["ok"]["signals"]
        .as_array()
        .expect("a result carries its signals");
    println!(
        "signals under a trace whose routes agree: {}",
        signals.len()
    );
    assert!(signals.is_empty(), "{signals:#?}");
    assert_eq!(code, Some(0));
}
