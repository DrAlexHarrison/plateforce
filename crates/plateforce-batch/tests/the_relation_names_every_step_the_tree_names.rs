//! The folder run's provenance relation names what the chain behind each number names, and
//! nothing else.
//!
//! This surface read the chain for its shape and then took each row's parameters from
//! `bound_methods`, which agreed for as long as a step carried nothing a rule's own row could
//! not. The analysis gravity belongs to the analysis and no registry entry declares it, so no
//! rule may record it and the derivation carries it instead. Four numbers then moved between two
//! folder runs while the record of what produced them stayed identical.
//!
//! A source scan cannot see this: the call to the derivation was there the whole time. What the
//! consumer did with the tree after it had it is a value, so it is measured here.
//!
//! The expected side comes from the engine over the same recording rather than from reading the
//! run back, so the two sides are two paths over one trial and not one path compared with
//! itself.

mod common;

use std::collections::BTreeSet;

use plateforce_analysis::{chains_of, run};
use plateforce_batch::{analyse, TrialIdentity, TrialSet};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::read_trial_from_path;

use common::{bound_request, committed_format, registry, tempdir, FIXTURES};

const TRIAL_FILE: &str = "subject01_trial1.force.txt";
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// One step's row: the number it stands under, the depth it stands at, the rule, and one name
/// it says produced the number. An empty name is a step that read nothing, which the relation
/// writes as a row of its own so the rules above it are not left standing under nothing.
type Row = (String, usize, String, String);

/// What the engine says about this trial, reached without going through the batch at all.
fn the_tree_the_engine_derived() -> BTreeSet<Row> {
    let (trial, _) = read_trial_from_path(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        '\t',
        0,
        CORPUS_SAMPLE_RATE_HZ,
    )
    .expect("the committed trace reads");
    let request = bound_request();
    let response = run(&trial, &request.analysis).expect("the trial computes");
    let loaded = registry();
    let chains = chains_of(
        &response,
        &RegistryStamp {
            version: None,
            declared_version: loaded.declared_version.clone(),
            digest: Some(loaded.content_digest.clone()),
        },
        false,
    );

    let mut rows = BTreeSet::new();
    for derived in &chains {
        // The relation carries a number and not a quantity that produced none, so a number
        // this trial does not report is outside both sides rather than missing from one.
        let reported = response
            .metrics
            .iter()
            .any(|metric| metric.key == derived.quantity && metric.value.is_some());
        if !reported {
            continue;
        }
        let mut pending = vec![(0usize, &derived.chain)];
        while let Some((depth, step)) = pending.pop() {
            let named: Vec<String> = step
                .provenance
                .parameters
                .iter()
                .map(|parameter| parameter.name.clone())
                .chain(
                    step.provenance
                        .choices
                        .iter()
                        .map(|choice| choice.name.clone()),
                )
                .collect();
            let method_id = step.provenance.method_id.clone();
            if named.is_empty() {
                rows.insert((
                    derived.quantity.clone(),
                    depth,
                    method_id.clone(),
                    String::new(),
                ));
            }
            for name in named {
                rows.insert((derived.quantity.clone(), depth, method_id.clone(), name));
            }
            pending.extend(step.depends_on.iter().map(|below| (depth + 1, below)));
        }
    }
    rows
}

fn the_relation_the_run_wrote() -> BTreeSet<Row> {
    let directory = tempdir("provenance-relation");
    std::fs::copy(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        directory.join(TRIAL_FILE),
    )
    .expect("the fixture copies");

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem)
        .expect("the folder walks");
    let result = analyse(&set, &bound_request(), &registry()).expect("the run produces a result");
    result
        .provenance
        .iter()
        .map(|row| {
            (
                row.quantity.clone(),
                row.depth,
                row.method_id.clone(),
                row.parameter.clone(),
            )
        })
        .collect()
}

/// One tree, one relation, the same steps and the same names on each of them.
#[test]
fn the_relation_names_what_the_tree_names_and_nothing_else() {
    let derived = the_tree_the_engine_derived();
    let written = the_relation_the_run_wrote();
    println!(
        "the engine derived {} rows, the run wrote {}",
        derived.len(),
        written.len()
    );

    // The control on the comparison. Two empty sets are equal, and a build that reported
    // nothing would read as agreement.
    assert!(
        derived.len() >= 100,
        "the engine derived {} rows on this trial, so this guard could not fail",
        derived.len()
    );
    let deepest = derived.iter().map(|row| row.1).max().unwrap_or(0);
    assert!(
        deepest >= 2,
        "the deepest step is at {deepest}, so this compares flat lists rather than trees"
    );

    let dropped: Vec<&Row> = derived.difference(&written).collect();
    let invented: Vec<&Row> = written.difference(&derived).collect();
    assert!(
        dropped.is_empty(),
        "the tree names {} rows the relation does not, first of them: {:?}",
        dropped.len(),
        first_few(&dropped)
    );
    assert!(
        invented.is_empty(),
        "the relation names {} rows the tree does not, first of them: {:?}",
        invented.len(),
        first_few(&invented)
    );
}

/// Enough of a difference to name it, with the count carrying the rest. One name repeated
/// under every quantity is one fault, and printing it ninety-six times buries the count.
fn first_few<'a>(rows: &[&'a Row]) -> Vec<&'a Row> {
    rows.iter().take(6).copied().collect()
}
