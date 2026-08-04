//! What the terminal prints where a signal has no number to print.
//!
//! The sentence a renderer writes for an absent value has to be true of every status the
//! vocabulary declares. A sentence written for the first signal to ship describes that
//! signal's own comparison, so the second signal to carry no value makes the terminal state
//! something that did not happen to the reader's trace, and the reader has no way to tell.
//!
//! So the assertion is that the terminal names the status the record carries, spelled as the
//! record spells it, rather than describing a comparison of its own.

use std::process::Command;

/// One recording of subject 01, cut twenty samples after its takeoff, which is the shape of
/// the 211 of 244 corpus trials whose recording ends at the plate's floor. There is no
/// landing, so there is no flight time and no second route to the height.
const WHOLE_TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";
const SAMPLES_AFTER_TAKEOFF: usize = 20;

/// The recording untouched, where both routes to a height run and the signal carries a
/// number. The control for every assertion below: without it a renderer that printed the
/// status for every signal would pass.
const A_TRACE_WHOSE_ROUTES_BOTH_RUN: &str =
    "../plateforce-conformance/fixtures/synthetic_untrimmed_step_off.force.txt";

/// The trace cut just after takeoff, written where the run can read it back.
///
/// Cut from the tracked recording rather than committed beside it, because the fixture
/// directory is walked by the batch suite, whose callers assert every trial it holds yields
/// a result.
fn a_recording_that_never_lands() -> std::path::PathBuf {
    let whole = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(WHOLE_TRIAL);
    let text = std::fs::read_to_string(&whole).expect("the tracked recording reads");
    let takeoff = takeoff_sample_of(WHOLE_TRIAL);
    let rows: Vec<&str> = text.lines().take(takeoff + SAMPLES_AFTER_TAKEOFF).collect();
    let cut = std::env::temp_dir().join("plateforce_a_recording_that_never_lands.force.txt");
    std::fs::write(&cut, rows.join("\n")).expect("the cut recording writes");
    cut
}

fn takeoff_sample_of(fixture: &str) -> usize {
    let (document, _) = analyse(fixture, "json");
    let parsed: serde_json::Value = serde_json::from_str(&document).expect("the result parses");
    parsed["ok"]["takeoff_index"]
        .as_u64()
        .expect("the whole recording places a takeoff") as usize
}

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

/// The terminal wraps its signals, so a sentence a reader takes in at once is several lines
/// on the page. Collapsing the whitespace asks what the reader read rather than where the
/// wrapping happened to fall.
fn as_one_line(document: &str) -> String {
    document.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn signal_holding_no_value(document: &str) -> serde_json::Value {
    let parsed: serde_json::Value = serde_json::from_str(document).expect("the result parses");
    let signals = parsed["ok"]["signals"]
        .as_array()
        .expect("a result carries its signals")
        .clone();
    println!("signals on this trial: {}", signals.len());
    let absent: Vec<serde_json::Value> = signals
        .into_iter()
        .filter(|signal| signal["value"].is_null())
        .collect();
    assert_eq!(
        absent.len(),
        1,
        "this recording raises one signal holding no value, and the assertions below are \
         about that signal: {absent:#?}"
    );
    absent.into_iter().next().unwrap()
}

/// What the terminal put where the number goes, which is the clause between the label and
/// the action that follows it.
///
/// Read out of the page rather than matched against a sentence written here, so a renderer
/// that prints the right words in the wrong place fails rather than passes.
fn figure_the_terminal_printed(printed: &str, label: &str) -> String {
    let opening = format!("{label}: ");
    let after = printed
        .split_once(&opening)
        .unwrap_or_else(|| panic!("the terminal printed no signal under {label:?}: {printed}"))
        .1;
    after
        .split_once(". ")
        .unwrap_or_else(|| panic!("the signal's opening clause never closes: {after}"))
        .0
        .to_string()
}

/// The record and the page say one word for one status.
///
/// Read from the same recording twice rather than compared against a word written here: a
/// spelling pinned in this file would agree with the terminal and disagree with the browser,
/// which reads the record.
#[test]
fn the_terminal_names_the_status_the_record_carries() {
    let cut = a_recording_that_never_lands();
    let fixture = cut.to_str().expect("the path is UTF-8");
    let (json, _) = analyse(fixture, "json");
    let signal = signal_holding_no_value(&json);

    let status = signal["status"]
        .as_str()
        .expect("a signal carries a status");
    let label = signal["label"].as_str().expect("a signal carries a label");
    let (text, _) = analyse(fixture, "text");
    let printed = figure_the_terminal_printed(&as_one_line(&text), label);
    println!("the record says {status:?}, and the terminal printed {printed:?}");
    assert_eq!(printed, status.replace('_', " "));
}

/// The sentences three renderers wrote for the first signal to ship. Each describes that
/// signal's own comparison and is true of no status in general, so none of them may stand
/// where the number goes.
///
/// The action beside the signal is free to say any of them, and this signal's does: the
/// engine wrote that sentence about this trace and it is true of it. The assertion is about
/// the clause the renderer supplies, not about the whole page.
#[test]
fn the_terminal_puts_no_comparison_of_its_own_where_the_number_goes() {
    let cut = a_recording_that_never_lands();
    let fixture = cut.to_str().expect("the path is UTF-8");
    let (json, _) = analyse(fixture, "json");
    let signal = signal_holding_no_value(&json);
    let label = signal["label"].as_str().expect("a signal carries a label");

    let (text, _) = analyse(fixture, "text");
    let printed = figure_the_terminal_printed(&as_one_line(&text), label);
    for invented in ["not comparable", "no second route on this trace"] {
        println!("the terminal printed {printed:?}, looking for {invented:?}");
        assert!(!printed.contains(invented));
    }
}

/// The control. A signal carrying a number still prints the number and the threshold it was
/// held against, so the assertions above are about the absent case rather than about a
/// renderer that stopped printing figures.
#[test]
fn a_signal_carrying_a_number_still_prints_it_against_its_threshold() {
    let (json, _) = analyse(A_TRACE_WHOSE_ROUTES_BOTH_RUN, "json");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("the result parses");
    let signals = parsed["ok"]["signals"]
        .as_array()
        .expect("a result carries its signals");
    let value = signals
        .iter()
        .find_map(|signal| signal["value"].as_f64())
        .expect("this recording raises a signal carrying a number");

    let (text, _) = analyse(A_TRACE_WHOSE_ROUTES_BOTH_RUN, "text");
    let printed = as_one_line(&text);
    let expected = format!("{value:.1} percent, past 20 percent.");
    println!("the record says {value}, and the terminal was read for: {expected}");
    assert!(printed.contains(&expected), "{printed}");
}
