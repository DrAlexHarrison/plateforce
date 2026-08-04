//! What reaches the other end of the pipe, under the environments a terminal arrives in.
//!
//! Two properties, and neither is about the numbers. Whether an escape byte survives is
//! decided in two places, the renderer that emits it and the stream that carries it, and an
//! operator who answered `always` had the answer honoured by the first and discarded by the
//! second. And a document asked for by path is read by a parser, so it carries none at all
//! whatever the operator answered, because that answer was about a terminal.
//!
//! The golden bytes here are the interface's own: `--help` and a refusal. Output derived
//! from the registry is not frozen, because a golden file over it asserts that
//! the registry has not changed, which is a different property from the one this file is
//! about. A golden file is regenerated deliberately and its diff audited, the discipline
//! `crates/plateforce-analysis/tests/resolved-values-baseline.txt` already carries.

use std::process::{Command, Output};

const ESCAPE: char = '\x1b';

/// The 24-bit form. `render.rs` caps the palette at four bits because a sequence in this
/// form is dropped or mangled inside tmux and screen, so it is asserted absent under every
/// environment rather than under the one where it would bite.
const TWENTY_FOUR_BIT: &str = "\x1b[38;2;";

const PAINTED: [&str; 3] = ["registry", "show", "onset.threshold.absolute_force"];

/// Every variable that decides colour is cleared first, so a case asserts what it names
/// rather than what the developer's own shell happens to export.
fn run(arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_plateforce"));
    command
        .args(["--registry", "../../registry"])
        .args(arguments)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR")
        .env_remove("CLICOLOR_FORCE")
        .env("TERM", "xterm-256color");
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("the built binary runs")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("the document is UTF-8")
}

// The environment matrix. Each case names the property it asserts.

#[test]
fn the_answer_the_operator_gave_reaches_the_stream() {
    let painted = stdout_of(&run(
        &[PAINTED[0], PAINTED[1], PAINTED[2], "--color", "always"],
        &[],
    ));
    assert!(
        painted.contains(ESCAPE),
        "--color always was answered and then discarded on the way out"
    );
}

#[test]
fn a_redirected_document_carries_no_escape_byte() {
    let plain = stdout_of(&run(&PAINTED, &[]));
    assert!(!plain.contains(ESCAPE), "{plain:?}");
}

#[test]
fn never_beats_the_environment() {
    let plain = stdout_of(&run(
        &[PAINTED[0], PAINTED[1], PAINTED[2], "--color", "never"],
        &[("CLICOLOR_FORCE", "1")],
    ));
    assert!(!plain.contains(ESCAPE), "{plain:?}");
}

#[test]
fn a_shell_that_forces_colour_gets_it() {
    let painted = stdout_of(&run(&PAINTED, &[("CLICOLOR_FORCE", "1")]));
    assert!(painted.contains(ESCAPE));
}

#[test]
fn no_color_is_honoured() {
    let plain = stdout_of(&run(&PAINTED, &[("NO_COLOR", "1")]));
    assert!(!plain.contains(ESCAPE), "{plain:?}");
}

#[test]
fn a_terminal_that_reports_no_capability_gets_none() {
    let plain = stdout_of(&run(&PAINTED, &[("TERM", "dumb")]));
    assert!(!plain.contains(ESCAPE), "{plain:?}");
}

/// A file is read by a parser rather than by a terminal, and an escape byte inside a JSON
/// document is rejected by every reader `out.rs` writes for.
#[test]
fn a_document_asked_for_by_path_carries_none_whatever_the_operator_answered() {
    let directory =
        std::env::temp_dir().join(format!("plateforce-survival-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a scratch directory");

    // Both formats, because only one of them paints. Asserting this over JSON alone passes
    // whatever the colour policy does, since nothing on that path was ever painted.
    for (format, name) in [("text", "entry.txt"), ("json", "entry.json")] {
        let path = directory.join(name);
        let named = path.display().to_string();
        run(
            &[
                PAINTED[0], PAINTED[1], PAINTED[2], "--color", "always", "--format", format,
                "--out", &named,
            ],
            &[("CLICOLOR_FORCE", "1")],
        );
        let written = std::fs::read_to_string(&path).expect("the document was written");
        assert!(!written.contains(ESCAPE), "{format}: {written:?}");
    }

    let json = std::fs::read_to_string(directory.join("entry.json")).expect("it was written");
    serde_json::from_str::<serde_json::Value>(&json).expect("a parser reads it");

    // The control: the same invocation without a destination does paint, so the assertion
    // above is about the destination rather than about a run that had no colour to lose.
    let to_a_pipe = stdout_of(&run(
        &[PAINTED[0], PAINTED[1], PAINTED[2], "--color", "always"],
        &[("CLICOLOR_FORCE", "1")],
    ));
    assert!(
        to_a_pipe.contains(ESCAPE),
        "nothing was painted to begin with"
    );

    std::fs::remove_dir_all(&directory).expect("the scratch directory goes");
}

/// The ceiling, asserted everywhere rather than under the one terminal that would mangle it.
/// The renderer has no path that emits this form, so a case that ever produced one would be
/// a change in the policy rather than in a terminal.
#[test]
fn no_environment_produces_a_twenty_four_bit_sequence() {
    let environments: [&[(&str, &str)]; 4] = [
        &[("CLICOLOR_FORCE", "1")],
        &[("TERM", "screen-256color")],
        &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")],
        &[("TERM", "screen-256color"), ("CLICOLOR_FORCE", "1")],
    ];
    let mut painted = 0;
    for environment in environments {
        let document = stdout_of(&run(
            &[PAINTED[0], PAINTED[1], PAINTED[2], "--color", "always"],
            environment,
        ));
        assert!(!document.contains(TWENTY_FOUR_BIT), "{environment:?}");
        if document.contains(ESCAPE) {
            painted += 1;
        }
    }
    // The control. Asserting the absence of one sequence over documents that carry no colour
    // at all would pass on four empty hands.
    println!(
        "environments that painted at all: {painted} of {}",
        environments.len()
    );
    assert_eq!(painted, environments.len());
}

// The interface's golden bytes.

fn golden(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/golden/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|error| panic!("tests/golden/{name} is committed: {error}"))
}

#[test]
fn the_help_a_reader_meets_first_is_the_committed_one() {
    let printed = stdout_of(&run(&["--help"], &[]));
    assert_eq!(
        printed,
        golden("help.stdout"),
        "regenerate tests/golden/help.stdout deliberately and audit the diff"
    );
}

/// A refusal is output too, and it is the line most likely to drift without anybody reading
/// it, because nothing downstream of a failure is usually looked at.
#[test]
fn the_refusal_for_an_id_that_resolves_nowhere_is_the_committed_one() {
    let output = run(&["registry", "show", "onset.threshold.invented"], &[]);
    let printed = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    assert_eq!(
        printed,
        golden("unknown-id.stderr"),
        "regenerate tests/golden/unknown-id.stderr deliberately and audit the diff"
    );
    assert_eq!(output.status.code(), Some(64));
}
