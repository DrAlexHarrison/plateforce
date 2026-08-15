//! What a result was asked to be written as is what arrives, or nothing arrives.
//!
//! One command writes Markdown. Every other one reports something a Markdown document of a
//! result cannot hold, and hands back the refusal that names it rather than the text or the
//! JSON it would have written anyway. A reader piping `--format markdown` into a chat and
//! receiving a column-aligned table has been answered, wrongly, with no way to tell.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// The heading the Markdown writer opens a result with, which is how a document that is
/// Markdown is told apart from one that is merely text.
const MARKDOWN_OPENS_WITH: &str = "# plateforce result:";

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// A trial under rules that leave no choice open, so a run reaches a result rather than the
/// refusal that meets an unstated one.
fn analysed(extra: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = [
        "--registry",
        "../../registry",
        "analyse",
        FIXTURE,
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
    ]
    .iter()
    .map(|word| (*word).to_string())
    .collect();
    line.extend(extra.iter().map(|word| (*word).to_string()));
    line
}

/// The one command that writes Markdown writes Markdown, so every refusal below is a
/// statement about the other commands rather than about a format nothing produces.
#[test]
fn the_command_that_writes_markdown_writes_a_markdown_document() {
    let line = analysed(&["--format", "markdown"]);
    let borrowed: Vec<&str> = line.iter().map(String::as_str).collect();
    let output = run(&borrowed);
    let document = String::from_utf8(output.stdout).expect("the document is UTF-8");
    println!("{}", document.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(0));
    assert!(document.starts_with(MARKDOWN_OPENS_WITH), "{document}");
    assert!(
        document.contains("| Quantity | Value | Unit | Rules |"),
        "{document}"
    );
}

/// Every command that reports something else, with the words that reach it, so each arrives
/// at its own body rather than at the parser.
///
/// A command reached with the wrong arguments is refused by clap before the format is looked
/// at, and that refusal carries exit 64 as well. Asserting on the sentence rather than on the
/// status is what tells the two apart: a table of half-built command lines would pass this
/// while proving nothing about any of them.
const COMMANDS_THAT_REPORT_SOMETHING_ELSE: [(&str, &[&str]); 11] = [
    ("capability", &["capability"]),
    ("methods", &["methods"]),
    ("plate", &["plate", "list"]),
    ("reach", &["reach"]),
    ("registry census", &["registry", "census"]),
    ("registry validate", &["registry", "validate"]),
    (
        "registry show",
        &["registry", "show", "onset.threshold.noise_relative"],
    ),
    ("version", &["version"]),
    ("man", &["man"]),
    ("completions", &["completions", "bash"]),
    (
        "batch",
        &[
            "batch",
            "../plateforce-conformance/fixtures",
            "--out-dir",
            "../../target/tmp/markdown-refused",
            "--trial-suffix",
            ".force.txt",
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
        ],
    ),
];

#[test]
fn a_command_that_reports_something_other_than_a_trial_refuses_markdown_by_name() {
    let mut faults = Vec::new();
    for (command, line) in COMMANDS_THAT_REPORT_SOMETHING_ELSE {
        let mut arguments: Vec<&str> = vec!["--format", "markdown"];
        arguments.extend_from_slice(line);
        let output = run(&arguments);
        let said = String::from_utf8_lossy(&output.stderr).to_string();
        let named = format!("`{command}`");
        if output.status.code() != Some(64) {
            faults.push(format!("{command} exited {:?}", output.status.code()));
        }
        if !output.stdout.is_empty() {
            faults.push(format!(
                "{command} wrote a document: {}",
                String::from_utf8_lossy(&output.stdout)
                    .chars()
                    .take(80)
                    .collect::<String>()
            ));
        }
        if !said.contains("--format markdown reports an analysed trial") || !said.contains(&named) {
            faults.push(format!("{command} declined with: {said}"));
        }
    }
    println!(
        "commands refusing Markdown by name: {} of {}",
        COMMANDS_THAT_REPORT_SOMETHING_ELSE.len() - faults.len(),
        COMMANDS_THAT_REPORT_SOMETHING_ELSE.len()
    );
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

/// A run refused for its shape leaves nothing behind, so the folder a run would have written
/// into is not made by a request that was never going to produce one.
#[test]
fn a_folder_run_refused_for_its_shape_makes_no_folder() {
    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/markdown-refused-before-the-folder");
    let _ = std::fs::remove_dir_all(&out_dir);
    let named = out_dir.display().to_string();

    let output = run(&[
        "--format",
        "markdown",
        "batch",
        "../plateforce-conformance/fixtures",
        "--out-dir",
        &named,
        "--trial-suffix",
        ".force.txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
    ]);
    println!(
        "exit {:?}, folder made: {}",
        output.status.code(),
        out_dir.exists()
    );
    assert_eq!(output.status.code(), Some(64));
    assert!(!out_dir.exists(), "{} was made", out_dir.display());
}

/// The commands this binary offers, read off its own help rather than from a list here, so a
/// command added without a ruling about Markdown fails this rather than shipping.
fn commands_offered() -> Vec<String> {
    let output = run(&["--help"]);
    let help = String::from_utf8(output.stdout).expect("the help is UTF-8");
    help.lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// Two commands answer no request for a document. `serve` holds the process and hands back
/// nothing to write, and `help` is written by the parser before a command is reached, which
/// is why `--format json` prints the same page.
const COMMANDS_THAT_HAND_BACK_NO_DOCUMENT: [&str; 2] = ["serve", "help"];

#[test]
fn every_command_this_binary_offers_has_been_ruled_on() {
    let offered = commands_offered();
    // A control first: a help page this walk failed to parse reports nothing offered, which
    // reads exactly like a binary whose every command is accounted for.
    assert!(
        offered.len() >= 12,
        "the walk read {} commands out of the help, so its verdict is about the walk",
        offered.len()
    );

    let ruled: Vec<&str> = COMMANDS_THAT_REPORT_SOMETHING_ELSE
        .into_iter()
        .map(|(command, _)| {
            // `registry census` and its siblings are ruled on under the parent a reader types.
            command.split_whitespace().next().unwrap_or(command)
        })
        .chain(COMMANDS_THAT_HAND_BACK_NO_DOCUMENT)
        .chain(["analyse", "spread"])
        .collect();
    let unruled: Vec<&String> = offered
        .iter()
        .filter(|command| !ruled.contains(&command.as_str()))
        .collect();
    println!(
        "commands offered {}, ruled on {} of {}",
        offered.len(),
        offered.len() - unruled.len(),
        offered.len()
    );
    assert!(unruled.is_empty(), "unruled: {unruled:?}");
}

/// The sweep is the twelfth command, and it costs an analysis to reach its own body, so it is
/// driven once rather than inside the table above.
#[test]
fn the_sweep_refuses_markdown_by_name() {
    let mut line = analysed(&["--format", "markdown"]);
    // The same trial and rules the analysis takes, under the command that sweeps them.
    let at = line
        .iter()
        .position(|word| word == "analyse")
        .expect("the command");
    line[at] = "spread".to_string();
    let borrowed: Vec<&str> = line.iter().map(String::as_str).collect();
    let output = run(&borrowed);
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty(), "the sweep wrote a document");
    assert!(said.contains("`spread`"), "{said}");
}
