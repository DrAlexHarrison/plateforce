//! The account printed in the Python package's README is the account this engine writes.
//!
//! That block is the product's own shop window: it is on the package page a reader installs
//! from, and what it shows is a number carrying the method that produced it, which is the whole
//! claim. It had drifted. It named a step the tree does not have, left off four choices the
//! root records, spelled one rule's two named alternatives under two different names, and gave
//! two parameters values the rules do not use.
//!
//! Nothing failed to compile while that was true, and no reader could tell. A worked example
//! that a reader copies is a claim about what the software does, so it is checked by running
//! the software.
//!
//! The request is the README's own, on the one trial this repository commits, which is subject
//! 01's and the only athlete whose data or derived data is ever public.
//!
//! The `registry declaring` line is skipped, and only that line: it carries the content digest,
//! which `crates/plateforce-cli/tests/digests_in_prose.rs` already holds to what this registry
//! answers. Checking it here too would be one fact with two guards, free to disagree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use plateforce_analysis::{accounts_of, run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::read_trial_from_path;

mod common;

const README: &str = "crates/plateforce-python/README.md";
const TRIAL: &str = "crates/plateforce-conformance/fixtures/subject01_trial1.force.txt";
const SAMPLE_RATE_HZ: f64 = 1200.0;
const QUANTITY: &str = "jump_height_from_takeoff_meters";
const REGISTRY_LINE: &str = "registry declaring";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate sits inside the repository")
}

/// The account this engine writes for the README's request on the README's trial.
fn the_account_this_engine_writes() -> String {
    let (trial, _) = read_trial_from_path(
        repository().join(TRIAL).to_string_lossy().to_string(),
        '\t',
        0,
        SAMPLE_RATE_HZ,
    )
    .expect("the committed trace reads");
    let request = common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    });
    let response = run(&trial, &request).expect("the committed trial computes");
    // The README's block ends by saying the acquisition block could not be filled, which is
    // what a reader loading a bare column of newtons gets.
    let stamp = RegistryStamp {
        version: None,
        declared_version: None,
        digest: None,
    };
    accounts_of(&response, &stamp, false)
        .remove(QUANTITY)
        .unwrap_or_else(|| panic!("this trial reported no {QUANTITY}"))
}

/// The block the README prints under the example, read out of the file.
///
/// Found by its first line rather than by counting fences, so a paragraph added above it does
/// not silently move which block is checked.
fn the_block_the_readme_prints() -> Vec<String> {
    let text = std::fs::read_to_string(repository().join(README)).expect("the README reads");
    let mut lines = text.lines();
    let opened = lines
        .by_ref()
        .find(|line| line.trim_end().ends_with(" meters"))
        .unwrap_or_else(|| panic!("{README} prints no account: no line ends in a unit"));
    let mut block = vec![opened.to_string()];
    for line in lines {
        if line.starts_with("```") {
            return block;
        }
        block.push(line.to_string());
    }
    panic!("{README}'s account block is never closed")
}

fn without_the_registry_line(lines: &[String]) -> Vec<&str> {
    lines
        .iter()
        .map(|line| line.as_str())
        .filter(|line| !line.trim_start().starts_with(REGISTRY_LINE))
        .collect()
}

/// Line for line, the same account.
#[test]
fn the_readme_prints_the_account_this_engine_writes() {
    let written: Vec<String> = the_account_this_engine_writes()
        .lines()
        .map(|line| line.to_string())
        .collect();
    let printed = the_block_the_readme_prints();

    // The control. A block that read as three lines, or an account that did, would compare
    // almost nothing and pass while doing it.
    assert!(
        written.len() >= 20 && printed.len() >= 20,
        "the engine wrote {} lines and the README prints {}, so this compares almost nothing",
        written.len(),
        printed.len()
    );

    let expected = without_the_registry_line(&written);
    let found = without_the_registry_line(&printed);
    if expected != found {
        let mismatched = expected
            .iter()
            .zip(found.iter())
            .enumerate()
            .find(|(_, (one, other))| one != other);
        let told = match mismatched {
            Some((at, (one, other))) => {
                format!("line {at} reads\n      {other}\n    and the engine writes\n      {one}")
            }
            None => format!(
                "the engine writes {} lines and the README prints {}",
                expected.len(),
                found.len()
            ),
        };
        panic!("{README} prints an account this engine does not write: {told}\n\nthe engine's own account:\n{}", written.join("\n"));
    }
    println!(
        "{} lines compared, one line skipped for digests_in_prose",
        expected.len()
    );
}

/// The half above cannot see: that the README quotes the registry line at all.
///
/// Skipping it is safe only while another guard holds it, and that guard reads the files that
/// quote a digest. A README that stopped quoting one would leave the skip covering nothing.
#[test]
fn the_readme_still_names_the_registry_behind_the_number() {
    let printed = the_block_the_readme_prints();
    assert!(
        printed
            .iter()
            .any(|line| line.trim_start().starts_with(REGISTRY_LINE)),
        "{README} names no registry behind its worked example, so the line this file skips is \
         held by nothing"
    );
}
