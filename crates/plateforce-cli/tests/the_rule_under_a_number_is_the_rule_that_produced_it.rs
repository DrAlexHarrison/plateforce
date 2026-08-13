//! The rule printed under a number is the rule that produced that number, not another rule
//! its answer happens to rest on.
//!
//! A quantity that names an arithmetic entry owes that entry. A landmark quantity names none
//! and is still some rule's answer, and the rule whose answer it is fills that landmark's own
//! slot: the onset time is the onset rule's answer whatever else fed it. An onset search
//! bounded by takeoff puts the takeoff rule into the onset time's chain, so a name read off
//! that chain by position rather than by slot reports a takeoff rule under the onset.
//!
//! The slot each rule fills is read from `methods`, so this file holds which slot a quantity
//! belongs to and nothing about how the page picks a name. A rule that records under another
//! entry reaches the page under that entry, which `records_under` states and this reads.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

const TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// The four quantities that name no arithmetic entry, each beside the slot whose rule
/// produced it.
const LANDMARKS: [(&str, &str); 4] = [
    ("system_weight_newtons", "weighing"),
    ("system_mass_kilograms", "weighing"),
    ("onset_time_seconds", "onset"),
    ("takeoff_time_seconds", "takeoff"),
];

/// One pipeline to run, and the values its rules are published more than one way for.
struct Pipeline {
    name: String,
    args: Vec<String>,
}

fn flags(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

/// The rules this file sweeps, each with the value its entry leaves open.
fn pipelines() -> Vec<Pipeline> {
    let onsets = [
        ("onset.threshold.noise_relative", vec!["--set", "onset.k=5"]),
        (
            "onset.threshold.relative_to_system_weight",
            vec!["--set", "onset.pct=2.5"],
        ),
        (
            "onset.threshold.absolute_force",
            vec!["--set", "onset.threshold_n=20"],
        ),
        ("onset.threshold.last_within_band", Vec::new()),
        (
            "onset.threshold.adaptive_trailing_window",
            vec!["--set", "onset.k=5"],
        ),
    ];
    let takeoffs = [
        (
            "takeoff.threshold.absolute_force",
            vec!["--set", "takeoff.threshold_n=20"],
        ),
        (
            "takeoff.threshold.flight_noise_k_sd",
            vec!["--set", "takeoff.k=5"],
        ),
    ];

    // The published pipeline first, because it is the one a reader runs without choosing
    // anything, and it is the one whose onset search reads takeoff.
    let mut pipelines = vec![Pipeline {
        name: "preset sams".to_string(),
        args: flags(&["--preset", "sams"]),
    }];
    for (onset, onset_values) in &onsets {
        for (takeoff, takeoff_values) in &takeoffs {
            let mut args = flags(&[
                "--weighing",
                "bwepoch.adaptive_lowest_variance",
                "--set",
                "weighing.window_seconds=1",
                "--onset",
                onset,
                "--takeoff",
                takeoff,
            ]);
            args.extend(flags(onset_values));
            args.extend(flags(takeoff_values));
            pipelines.push(Pipeline {
                name: format!("{onset} with {takeoff}"),
                args,
            });
        }
    }
    pipelines
}

/// The command's own output, and its refusal where it declined.
///
/// A run this file could not make is not evidence about what a page names, so the refusal
/// travels with the output rather than reaching an assertion as an empty document.
fn plateforce(args: &[&str]) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["--registry", "../../registry"])
        .args(args)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    (
        String::from_utf8(output.stdout).expect("the document is UTF-8"),
        String::from_utf8(output.stderr).expect("the refusal is UTF-8"),
    )
}

fn analyse(format: &str, pipeline: &Pipeline) -> String {
    let mut args = vec![
        "--format".to_string(),
        format.to_string(),
        "analyse".to_string(),
        TRIAL.to_string(),
        "--column".to_string(),
        "0".to_string(),
        "--sample-rate-hz".to_string(),
        "1200".to_string(),
        "--sentinel".to_string(),
        "none".to_string(),
        "--delimiter".to_string(),
        "\t".to_string(),
    ];
    args.extend(pipeline.args.iter().cloned());
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let (page, refusal) = plateforce(&borrowed);
    assert!(
        !page.trim().is_empty(),
        "{} produced no {format}: {refusal}",
        pipeline.name
    );
    page
}

fn document(pipeline: &Pipeline) -> serde_json::Value {
    let written = analyse("json", pipeline);
    let parsed: serde_json::Value = serde_json::from_str(&written)
        .unwrap_or_else(|error| panic!("{} wrote no document: {error}", pipeline.name));
    parsed.get("ok").cloned().unwrap_or_else(|| {
        panic!("{} was refused: {parsed}", pipeline.name);
    })
}

/// Every rule filling each slot, as the software itself groups them, plus the entry a rule
/// records under where it records under another.
fn rules_per_slot() -> BTreeMap<String, BTreeSet<String>> {
    let (written, refusal) = plateforce(&["--format", "json", "methods"]);
    let parsed: serde_json::Value = serde_json::from_str(&written)
        .unwrap_or_else(|error| panic!("the rule list did not arrive: {error} {refusal}"));
    let steps = parsed["ok"]["steps"]
        .as_array()
        .expect("the rule list groups rules into steps")
        .clone();
    let mut per_slot: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for step in steps {
        let slot = step["slot"]
            .as_str()
            .expect("a step names its slot")
            .to_string();
        let entry = per_slot.entry(slot).or_default();
        for rule in step["rules"].as_array().expect("a step lists its rules") {
            entry.insert(
                rule["method_id"]
                    .as_str()
                    .expect("a rule has an id")
                    .to_string(),
            );
            if let Some(under) = rule["records_under"].as_str() {
                entry.insert(under.to_string());
            }
        }
    }
    per_slot
}

/// What the page names under each quantity, keyed by the quantity, read as a reader reads it:
/// the indented line under the row carrying the label and the value.
fn named_under_each_number(page: &str, document: &serde_json::Value) -> BTreeMap<String, String> {
    let metrics = document["metrics"].as_array().expect("metrics is a list");
    let lines: Vec<&str> = page.lines().collect();
    let mut named = BTreeMap::new();
    let mut from = 0usize;
    for metric in metrics {
        let key = metric["key"].as_str().expect("a metric names its quantity");
        let label = metric["label"].as_str().expect("a metric carries a label");
        for index in from..lines.len() {
            let Some(rest) = lines[index].strip_prefix("  ") else {
                continue;
            };
            if rest.starts_with(' ') {
                continue;
            }
            let Some(after) = rest.strip_prefix(label) else {
                continue;
            };
            let value = after.trim_start();
            // `Takeoff` opens `Takeoff velocity`, and a value row is told from a label by what
            // follows it: a figure, or one of the two words a quantity without one reads as.
            let carries_a_value = value.starts_with(|first: char| first.is_ascii_digit())
                || value.starts_with('-')
                || value.starts_with("no value")
                || value.starts_with("not a number");
            if !carries_a_value {
                continue;
            }
            if let Some(next) = lines.get(index + 1) {
                named.insert(key.to_string(), next.trim().to_string());
            }
            from = index + 1;
            break;
        }
    }
    named
}

/// The rule the record's own account of a quantity opens with, which is the same claim the
/// page makes in one word.
fn rule_the_account_opens_with(document: &serde_json::Value, key: &str) -> Option<String> {
    let account = document["descriptions"][key].as_str()?;
    let opening = account.lines().nth(1)?.trim();
    opening.split_whitespace().next().map(str::to_string)
}

/// Under every landmark the page names a rule that fills that landmark's own slot.
///
/// The claim is about the slot rather than about a particular id, so a pipeline is free to
/// bind whichever rule it likes and the assertion still has only one right answer.
#[test]
fn a_landmark_names_a_rule_that_fills_its_own_slot() {
    let per_slot = rules_per_slot();
    let mut checked = 0usize;
    let mut wrong: Vec<String> = Vec::new();

    for pipeline in pipelines() {
        let document = document(&pipeline);
        let named = named_under_each_number(&analyse("text", &pipeline), &document);
        for (key, slot) in LANDMARKS {
            let Some(name) = named.get(key) else {
                wrong.push(format!(
                    "{}: {key} named nothing on the page",
                    pipeline.name
                ));
                continue;
            };
            checked += 1;
            let population = per_slot
                .get(slot)
                .unwrap_or_else(|| panic!("{slot} is a slot the software groups rules under"));
            if !population.contains(name) {
                let elsewhere = per_slot
                    .iter()
                    .find(|(_, rules)| rules.contains(name))
                    .map(|(other, _)| other.as_str())
                    .unwrap_or("no slot");
                wrong.push(format!(
                    "{}: {key} names {name}, which fills {elsewhere} and not {slot}",
                    pipeline.name
                ));
            }
        }
    }

    // The denominator, so a sweep that read almost nothing cannot pass by having looked.
    assert!(
        checked >= 40,
        "only {checked} landmark rows were read across the pipelines"
    );
    assert!(
        wrong.is_empty(),
        "{} of {checked} landmark rows name a rule from another slot:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

/// Changing the takeoff rule moves the name under the takeoff and leaves the onset alone.
///
/// Run over an onset rule whose search is bounded by takeoff, so the takeoff rule is among the
/// contributors to the onset time and a name taken from where they are listed follows it. A
/// pipeline refuses a rule it does not take, so the two runs are stated rather than adopted.
#[test]
fn the_takeoff_rule_does_not_move_the_name_under_the_onset() {
    let named = |takeoff: &str, values: &[&str]| -> BTreeMap<String, String> {
        let mut args = flags(&[
            "--weighing",
            "bwepoch.adaptive_lowest_variance",
            "--set",
            "weighing.window_seconds=1",
            "--onset",
            "onset.threshold.last_within_band",
            "--takeoff",
            takeoff,
        ]);
        args.extend(flags(values));
        let pipeline = Pipeline {
            name: "an onset search bounded by takeoff".to_string(),
            args,
        };
        let document = document(&pipeline);
        named_under_each_number(&analyse("text", &pipeline), &document)
    };
    let under = |named: &BTreeMap<String, String>, key: &str| -> String {
        named
            .get(key)
            .unwrap_or_else(|| panic!("{key} named nothing on the page"))
            .clone()
    };

    let first = named(
        "takeoff.threshold.absolute_force",
        &["--set", "takeoff.threshold_n=20"],
    );
    let second = named(
        "takeoff.threshold.flight_noise_k_sd",
        &["--set", "takeoff.k=5"],
    );

    assert_eq!(
        under(&first, "takeoff_time_seconds"),
        "takeoff.threshold.absolute_force"
    );
    assert_eq!(
        under(&second, "takeoff_time_seconds"),
        "takeoff.threshold.flight_noise_k_sd"
    );

    let onset_first = under(&first, "onset_time_seconds");
    let onset_second = under(&second, "onset_time_seconds");
    assert_eq!(
        onset_first, onset_second,
        "the onset named {onset_first} and then {onset_second}, so it followed the takeoff rule"
    );
    assert_ne!(
        onset_first,
        under(&first, "takeoff_time_seconds"),
        "the onset and the takeoff named one rule between them"
    );

    // The other half: the weighing rule did not move, so this is reading each number's own
    // record rather than whichever rule the run last changed.
    assert_eq!(
        under(&first, "system_weight_newtons"),
        under(&second, "system_weight_newtons")
    );
}

/// Three landmarks bound to three rules name three rules.
///
/// A membership test on the chain passes when one rule is printed under several numbers, and
/// this is the half of the claim that does not.
#[test]
fn three_landmarks_under_three_rules_name_three_rules() {
    let pipeline = Pipeline {
        name: "preset sams".to_string(),
        args: flags(&["--preset", "sams"]),
    };
    let document = document(&pipeline);
    let named = named_under_each_number(&analyse("text", &pipeline), &document);

    let landmarks = [
        "system_weight_newtons",
        "onset_time_seconds",
        "takeoff_time_seconds",
    ];
    let names: Vec<String> = landmarks
        .iter()
        .map(|key| {
            named
                .get(*key)
                .unwrap_or_else(|| panic!("{key} named nothing on the page"))
                .clone()
        })
        .collect();
    let distinct: BTreeSet<&String> = names.iter().collect();
    assert_eq!(
        distinct.len(),
        3,
        "three landmarks ran under three rules and named {} between them: {names:?}",
        distinct.len()
    );
}

/// The account the record writes for a landmark opens with the rule the page names under it.
///
/// **This case is green on a build that names the wrong rule, and that is what it is for.** It
/// catches the fix nobody should make: a renderer that picks the right rule for the terminal
/// and leaves the record wrong. The page's one word and the account's opening line are the
/// same claim, and a reader in a notebook, an R session or a browser tab reads the second, so
/// the two agreeing is a property worth holding whichever rule they agree on. The cases above
/// are what hold them to the right one. Deleting this as redundant removes the only guard on
/// the terminal and the record staying one answer.
#[test]
fn the_account_a_landmark_gives_of_itself_opens_with_the_rule_the_page_names() {
    let mut checked = 0usize;
    let mut disagreeing: Vec<String> = Vec::new();

    for pipeline in pipelines() {
        let document = document(&pipeline);
        let named = named_under_each_number(&analyse("text", &pipeline), &document);
        for (key, _) in LANDMARKS {
            let (Some(page), Some(account)) =
                (named.get(key), rule_the_account_opens_with(&document, key))
            else {
                continue;
            };
            checked += 1;
            if page != &account {
                disagreeing.push(format!(
                    "{}: {key} reads {page} on the page and {account} in the account",
                    pipeline.name
                ));
            }
        }
    }

    assert!(
        checked >= 40,
        "only {checked} accounts were read across the pipelines"
    );
    assert!(
        disagreeing.is_empty(),
        "{} of {checked} accounts name a different rule from the page:\n  {}",
        disagreeing.len(),
        disagreeing.join("\n  ")
    );
}
