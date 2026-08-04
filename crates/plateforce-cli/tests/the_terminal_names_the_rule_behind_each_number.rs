//! Every number the terminal prints names the rule behind it, on the screen a reader gets
//! without asking for anything.
//!
//! The page used to print eleven numbers and then a flat list of sixteen bound rules, so a
//! reader holding `Jump height, flight time 0.4402 m` could not tell which of the sixteen
//! produced it. The account under `--provenance` answered it and a reader who does not know
//! to ask for the record is the reader that record exists for.
//!
//! Read out of the page and the document from the same request, so this asserts the page
//! agrees with the record rather than that the page holds a name written here. Where the
//! record names an arithmetic in `computed_by`, that is the name the page owes; where it
//! names none, the number is a landmark rule's own answer and the page owes one of the rules
//! the record says it rests on.

use std::process::Command;

const TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// The takeoff rule the cases below run unless they are varying it.
const TAKEOFF: [&str; 4] = [
    "--takeoff",
    "takeoff.threshold.absolute_force",
    "--set",
    "takeoff.threshold_n=20",
];

/// The heading of the section behind `--provenance`, read here so the cases can assert the
/// names reach the page without it.
const ACCOUNTS: &str = "How each number was produced";

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
        ])
        .args(extra)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    let output = command.output().expect("the built binary runs");
    String::from_utf8(output.stdout).expect("the document is UTF-8")
}

fn document(extra: &[&str]) -> serde_json::Value {
    let parsed: serde_json::Value =
        serde_json::from_str(&analyse("json", extra)).expect("the document parses");
    parsed
        .get("ok")
        .cloned()
        .unwrap_or_else(|| panic!("the terminal returned a refusal: {parsed}"))
}

/// The labels the record reports, in the order the page draws them.
fn labels(document: &serde_json::Value) -> Vec<String> {
    document["metrics"]
        .as_array()
        .expect("metrics is a list")
        .iter()
        .map(|metric| {
            metric["label"]
                .as_str()
                .expect("a metric carries a label")
                .to_string()
        })
        .collect()
}

/// What the page names under each number, read as a reader reads it: the line under the one
/// carrying the label and the value.
///
/// Walked forward through the page in the record's own order, so a label that opens another
/// label cannot take the other's row. `Takeoff` and `Takeoff velocity` are told apart by what
/// follows the label: a value row carries the figure next, never a word.
fn named_under_each_number(page: &str, labels: &[String]) -> Vec<Option<String>> {
    let lines: Vec<&str> = page.lines().collect();
    let mut named = Vec::new();
    let mut from = 0usize;
    for label in labels {
        let mut found = None;
        for index in from..lines.len() {
            let Some(rest) = lines[index].strip_prefix("  ") else {
                continue;
            };
            if rest.starts_with(' ') {
                continue;
            }
            let Some(after) = rest.strip_prefix(label.as_str()) else {
                continue;
            };
            let value = after.trim_start();
            let carries_a_value = value.starts_with(|first: char| first.is_ascii_digit())
                || value.starts_with('-')
                || value.starts_with("no value")
                || value.starts_with("not a number");
            if !carries_a_value {
                continue;
            }
            found = lines.get(index + 1).map(|next| next.trim().to_string());
            from = index + 1;
            break;
        }
        named.push(found);
    }
    named
}

/// Every number on the page names the rule the record roots it at.
#[test]
fn every_number_names_the_rule_the_record_roots_it_at() {
    let document = document(&TAKEOFF);
    let page = analyse("text", &TAKEOFF);
    let metrics = document["metrics"].as_array().expect("metrics is a list");

    // The denominator every count below is over. A run reporting almost nothing would satisfy
    // the comparisons having looked at almost nothing.
    assert!(
        metrics.len() >= 8,
        "the run reported {} quantities, so this case would read almost nothing",
        metrics.len()
    );

    // The claim this file exists for: the names are on the page nobody asked a flag for.
    assert!(
        !page.contains(ACCOUNTS),
        "the page carries the section behind --provenance, so it was not the default one"
    );

    let named = named_under_each_number(&page, &labels(&document));
    let silent: Vec<&str> = named
        .iter()
        .zip(metrics)
        .filter(|(name, _)| name.is_none())
        .map(|(_, metric)| metric["key"].as_str().expect("a metric names its quantity"))
        .collect();
    assert!(
        silent.is_empty(),
        "{} of {} numbers named no rule: {silent:?}\n{page}",
        silent.len(),
        metrics.len()
    );

    let mut arithmetic = 0usize;
    let mut landmarks: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    for (metric, name) in metrics.iter().zip(&named) {
        let name = name.as_deref().expect("every number named something");
        let key = metric["key"].as_str().expect("a metric names its quantity");
        let resting_on: Vec<&str> = metric["contributing_method_ids"]
            .as_array()
            .expect("a metric lists what it rests on")
            .iter()
            .map(|id| id.as_str().expect("a contributing id is a name"))
            .collect();
        match metric["computed_by"].as_str() {
            Some(computed_by) => {
                arithmetic += 1;
                if name != computed_by {
                    wrong.push(format!(
                        "{key} names {name}, and the record says {computed_by}"
                    ));
                }
            }
            // No arithmetic entry describes this number, so it is a landmark rule's own
            // answer and the page owes one of the rules the record says it rests on.
            None => {
                landmarks.push(name.to_string());
                if !resting_on.contains(&name) {
                    wrong.push(format!(
                        "{key} names {name}, which is not among the {} rules it rests on",
                        resting_on.len()
                    ));
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} numbers name a rule the record does not: {wrong:?}",
        wrong.len(),
        metrics.len()
    );

    // The landmark half is a membership test, which one rule repeated under every number
    // would pass. The rules those numbers root at are distinct on this trial.
    let mut distinct = landmarks.clone();
    distinct.sort();
    distinct.dedup();
    assert!(
        distinct.len() >= 2,
        "{} numbers name no arithmetic and root at {} rule: {distinct:?}",
        landmarks.len(),
        distinct.len()
    );
    println!(
        "{arithmetic} of {} numbers name the arithmetic the record does, and the other {} root \
         at {} distinct rules: {distinct:?}",
        metrics.len(),
        landmarks.len(),
        distinct.len()
    );
}

/// The name moves when the rule moves, and only under the numbers that rule reached.
///
/// A page naming rules off a list written beside it would print the same names here. Read
/// through the same parser as the case above, so it can come back empty for the same reason.
#[test]
fn the_name_under_a_number_moves_when_the_run_changes_the_rule() {
    const OTHER: [&str; 2] = ["--takeoff", "takeoff.threshold.flight_noise_k_sd"];

    let first = document(&TAKEOFF);
    let second = document(&OTHER);
    let named_first = named_under_each_number(&analyse("text", &TAKEOFF), &labels(&first));
    let named_second = named_under_each_number(&analyse("text", &OTHER), &labels(&second));

    let under = |named: &[Option<String>], document: &serde_json::Value, key: &str| -> String {
        let position = document["metrics"]
            .as_array()
            .expect("metrics is a list")
            .iter()
            .position(|metric| metric["key"].as_str() == Some(key))
            .unwrap_or_else(|| panic!("{key} is reported"));
        named[position]
            .clone()
            .unwrap_or_else(|| panic!("{key} named no rule"))
    };

    assert_eq!(
        under(&named_first, &first, "takeoff_time_seconds"),
        TAKEOFF[1]
    );
    assert_eq!(
        under(&named_second, &second, "takeoff_time_seconds"),
        OTHER[1]
    );

    // The other half of the same property: a number the changed rule did not produce keeps
    // the rule it had, so the page is reading each number's own record rather than echoing
    // whatever the line last named.
    let weighing = under(&named_first, &first, "system_weight_newtons");
    assert_eq!(
        weighing,
        under(&named_second, &second, "system_weight_newtons")
    );
    assert_ne!(weighing, OTHER[1]);
    println!(
        "takeoff named {} and then {}, and system weight named {weighing} under both",
        TAKEOFF[1], OTHER[1]
    );
}
