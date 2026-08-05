//! Argument parsing was a second refusal channel, and a program could not branch on it.
//!
//! Every decline from a rule carries a code, a method id and the values that were available.
//! A decline from the parser carried a sentence beginning `error:` and nothing else, and it
//! exited 64, which is also the status of `decision_not_made` and `conventions_not_comparable`.
//! So a caller reading only the status could not tell **there is no such operation** from **a
//! decision on your path is still open**, and those want opposite responses: the first is
//! re-read the manifest, the second is state a value and run again.
//!
//! `phase2/ROADMAP.md` section 8a names this as M8's second requirement, a refusal vocabulary
//! an agent can branch on. This file is the measurement.

use std::process::Output;

fn plateforce(args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(args)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// The code a call came back with, read off the refusal envelope rather than off the prose.
///
/// `None` where nothing parseable came back, which is the state this file exists to remove
/// and is therefore never treated as an acceptable answer.
fn code_of(output: &Output) -> Option<String> {
    // A refusal carrying no document goes to stderr, which is where `stream_for` sends one,
    // so both channels are read from the same place by the same reader.
    let text = String::from_utf8_lossy(&output.stderr).to_string();
    let document: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    document["refusal"]["code"].as_str().map(str::to_string)
}

fn status(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

/// A run that is answerable but for one open decision, which is the call the parser's own
/// refusal used to be indistinguishable from.
fn a_run_with_one_open_decision() -> Vec<&'static str> {
    vec![
        "--registry",
        "../../registry",
        "analyse",
        "../plateforce-conformance/fixtures/subject01_trial1.force.txt",
        "--sample-rate-hz",
        "1200",
        "--column",
        "0",
        "--sentinel",
        "none",
        "--weighing",
        "bwepoch.fixed_window",
        "--set",
        "weighing.duration=1.0",
        "--onset",
        "onset.threshold.noise_relative",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--format",
        "json",
    ]
}

#[test]
fn a_word_this_build_does_not_offer_is_refused_in_the_published_vocabulary() {
    let mut line = a_run_with_one_open_decision();
    line.push("--nonexistent-flag");
    let output = plateforce(&line);

    assert!(!output.status.success(), "a mistyped flag ran the analysis");
    let code = code_of(&output).expect(
        "the parser answered with no code at all, so a program has only prose to branch on, \
         which is the gap this file measures",
    );
    assert_eq!(code, "command_line_not_parsed");

    // The parser's own sentence is carried through, because it names the offending token and
    // a message that dropped it would leave the caller to find their own typo.
    let text = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        text.contains("--nonexistent-flag"),
        "the refusal does not repeat the word the caller wrote, so they cannot correct it: {text}"
    );
}

/// The whole point: two declines that want opposite responses are told apart.
///
/// Held against each other rather than each against a constant. Asserting `decision_not_made`
/// alone would go on passing if the parser's code changed to match it, which is the failure.
#[test]
fn the_parser_and_the_registry_decline_under_codes_a_program_can_tell_apart() {
    let open_decision = plateforce(&a_run_with_one_open_decision());
    let mut mistyped = a_run_with_one_open_decision();
    mistyped.push("--nonexistent-flag");
    let unparsed = plateforce(&mistyped);

    let from_registry = code_of(&open_decision).expect("the registry's own refusal carries a code");
    let from_parser = code_of(&unparsed).expect("the parser's refusal carries a code");

    assert_eq!(from_registry, "decision_not_made");
    assert_eq!(from_parser, "command_line_not_parsed");
    assert_ne!(
        from_registry, from_parser,
        "both channels decline under one code, so a program cannot tell a missing operation \
         from an open decision and the two want opposite responses",
    );

    // The status is the same on purpose and is recorded here so nobody reads that as the
    // discriminator. 64 is EX_USAGE and both really are usage faults; the code is what
    // separates them, and it is what the contract tells a program to branch on.
    println!(
        "open decision exit={} code={from_registry}; unparsed line exit={} code={from_parser}",
        status(&open_decision),
        status(&unparsed),
    );
    assert_eq!(status(&open_decision), 64);
    assert_eq!(status(&unparsed), 64);
}

/// A required argument nobody wrote is the same class and reaches the same code.
///
/// It is the case that named the code: `argument_not_recognised` was written first and would
/// have mislabelled this one, because nothing here is unrecognised.
#[test]
fn a_required_argument_nobody_wrote_reaches_the_same_code() {
    let output = plateforce(&[
        "--registry",
        "../../registry",
        "batch",
        "/tmp",
        "--format",
        "json",
    ]);
    assert!(!output.status.success());
    assert_eq!(code_of(&output).as_deref(), Some("command_line_not_parsed"));
}

/// Asking for help is not a refusal, and a caller who asked for it gets it on stdout at zero.
///
/// The control for the whole file: without it, a change that turned every invocation into a
/// refusal would satisfy every assertion above.
#[test]
fn asking_what_this_program_does_is_not_a_refusal() {
    let output = plateforce(&["--help"]);
    assert!(
        output.status.success(),
        "asking for help exited {}",
        status(&output)
    );
    assert!(
        code_of(&output).is_none(),
        "help came back as a refusal, so the assertions above are about a binary that refuses \
         everything rather than about the parser",
    );
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(text.contains("analyse"), "help named no command: {text}");
}

/// A caller who did not ask for JSON still gets prose, because a person reading a terminal is
/// not helped by an envelope.
#[test]
fn a_line_that_asked_for_no_document_is_declined_in_prose() {
    let output = plateforce(&[
        "--registry",
        "../../registry",
        "batch",
        "/tmp",
        "--nonexistent-flag",
    ]);
    assert!(!output.status.success());
    assert!(
        code_of(&output).is_none(),
        "a caller who asked for no document got one anyway",
    );
    let text = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        text.contains("--nonexistent-flag"),
        "the prose decline names nothing the caller wrote: {text}"
    );
}
