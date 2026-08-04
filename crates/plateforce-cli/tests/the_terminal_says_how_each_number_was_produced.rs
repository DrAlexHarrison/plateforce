//! The account each number gives of itself reaches a reader at this end of the pipe.
//!
//! The document this surface writes carried the field and left it empty, so a terminal
//! reported eleven numbers and no account of any of them while an R session reported the same
//! eleven with one each. The record is now on both renderings: the parsed document always,
//! and the page under `--provenance`, which is the flag that already means the whole record
//! and is one interaction, as `describe()` is in R and in a notebook.
//!
//! Read out of the page and the document from the same trial, so this asserts the page agrees
//! with the record rather than that the page holds a sentence written here.
//!
//! The control is the page without the flag, and it can come back empty for the same reason
//! the real query would: both look for the same generated line in the same rendering.

use std::process::Command;

const TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// The heading the section sits under. Read here rather than assumed, because a reader
/// scanning for the accounts finds them by it.
const HEADING: &str = "How each number was produced";

fn analyse(format: &str, extra: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_plateforce"));
    command
        .args([
            "--registry",
            "../../registry",
            "--format",
            format,
            "analyse",
            TRIAL,
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
            "--delimiter",
            "\t",
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
        .args(extra)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    let output = command.output().expect("the built binary runs");
    String::from_utf8(output.stdout).expect("the document is UTF-8")
}

fn document() -> serde_json::Value {
    let parsed: serde_json::Value =
        serde_json::from_str(&analyse("json", &[])).expect("the document parses");
    parsed
        .get("ok")
        .cloned()
        .unwrap_or_else(|| panic!("the terminal returned a refusal: {parsed}"))
}

/// Every number this surface reports carries its own account in the document it writes.
#[test]
fn the_document_carries_an_account_of_every_number_it_reports() {
    let document = document();
    let metrics = document["metrics"].as_array().expect("metrics is a list");
    let accounts = document["descriptions"]
        .as_object()
        .expect("descriptions is a block");

    let valued: Vec<&str> = metrics
        .iter()
        .filter(|metric| !metric["value"].is_null())
        .map(|metric| metric["key"].as_str().expect("a metric names its quantity"))
        .collect();

    // The denominator the sentence below is over. A run reporting almost nothing would satisfy
    // the comparison having looked at almost nothing.
    assert!(
        valued.len() >= 8,
        "only {} of {} quantities carried a value",
        valued.len(),
        metrics.len()
    );

    let silent: Vec<&&str> = valued
        .iter()
        .filter(|key| !accounts.contains_key(**key))
        .collect();
    assert!(
        silent.is_empty(),
        "{} of {} quantities carrying a value gave no account of themselves: {silent:?}",
        silent.len(),
        valued.len()
    );
    println!(
        "{} of {} quantities carried a value and each gave an account",
        valued.len(),
        metrics.len()
    );
}

/// One string with every run of whitespace collapsed to a single space.
///
/// The column wraps a step too long for the width and keeps each step's own depth, so a line
/// of the record can reach the page as two lines with an indent between them. Collapsing both
/// sides compares the words in their order, which is what the record owes the page, and it
/// still fails on a word dropped, changed or moved.
fn flattened(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// And the page shows them, under the flag that already means the whole record.
///
/// The lines are the document's own. A column that composed its own would be the second home
/// this surface's readers were reading, so what is asserted is that every line of every
/// account the record holds appears on the page.
#[test]
fn the_page_shows_the_accounts_the_record_holds() {
    let document = document();
    let accounts = document["descriptions"]
        .as_object()
        .expect("descriptions is a block");
    let page = flattened(&analyse("text", &["--provenance"]));

    assert!(
        page.contains(HEADING),
        "the page never says {HEADING:?}: {page}"
    );

    // The population the comparison below runs over. A record holding no account at all
    // leaves that loop with nothing to look for, and a section printing a heading over
    // nothing would pass it.
    assert!(
        accounts.len() >= 8,
        "the record holds {} accounts, so the comparison below would look at almost nothing",
        accounts.len()
    );

    let mut missing: Vec<String> = Vec::new();
    let mut lines = 0usize;
    for (quantity, account) in accounts {
        for line in account.as_str().expect("an account is a sentence").lines() {
            lines += 1;
            if !page.contains(&flattened(line)) {
                missing.push(format!("{quantity}: {}", flattened(line)));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "{} of {lines} lines of the record never reach the page: {missing:?}",
        missing.len()
    );
    println!(
        "{} accounts shown on the page, all {lines} of their lines",
        accounts.len()
    );
}

/// The control. Without the flag the page carries none of it, so the case above is reading
/// the flag's own output rather than a section that was always there.
///
/// Both halves are asked: the heading, and a line only an account writes. The Rules section
/// prints the same rule under the same id and spells its values `name = value`, so a page
/// carrying the bound record still fails this if an account reached it.
#[test]
fn the_page_without_the_flag_carries_no_section_of_accounts() {
    let document = document();
    let accounts = document["descriptions"]
        .as_object()
        .expect("descriptions is a block");
    let page = flattened(&analyse("text", &[]));

    assert!(
        !page.contains(HEADING),
        "the section is printed whether or not it was asked for: {page}"
    );

    // The same floor the case above takes, and for the same reason: with no account in the
    // record there is nothing this could find on the page either.
    assert!(
        accounts.len() >= 8,
        "the record holds {} accounts, so this control would search for nothing",
        accounts.len()
    );

    let reached: Vec<&String> = accounts
        .iter()
        .filter(|(_, account)| {
            account
                .as_str()
                .expect("an account is a sentence")
                .lines()
                .any(|line| line.contains('{') && page.contains(&flattened(line)))
        })
        .map(|(quantity, _)| quantity)
        .collect();
    assert!(
        reached.is_empty(),
        "{} of {} accounts reach a page nobody asked for one: {reached:?}",
        reached.len(),
        accounts.len()
    );
    println!(
        "{} accounts held back and {} lines printed",
        accounts.len(),
        analyse("text", &[]).lines().count()
    );
}
