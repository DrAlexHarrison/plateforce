//! Whether a reader who runs a folder is handed the account each number gives of itself.
//!
//! A folder run wrote the rules behind every number as rows and no number's own account, so a
//! reader who analysed two hundred trials held every fact an account is made of and could not
//! read one. Only an R session could.
//!
//! The expected side comes from the engine's own generator over the same recording rather
//! than from reading the run back, so the two sides are two paths over one trial and not one
//! path compared with itself.

mod common;

use std::collections::BTreeSet;

use plateforce_analysis::{accounts_of, run};
use plateforce_batch::{analyse, Relation, TrialIdentity, TrialSet};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::read_trial_from_path;

use common::{bound_request, committed_format, registry, tempdir, FIXTURES};

const TRIAL_FILE: &str = "subject01_trial1.force.txt";
const TRIAL_ID: &str = "subject01_trial1";
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// What the engine says about this trial, reached without going through the batch at all.
fn accounts_the_engine_wrote() -> std::collections::BTreeMap<String, String> {
    let (trial, _) = read_trial_from_path(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        '\t',
        0,
        CORPUS_SAMPLE_RATE_HZ,
    )
    .expect("the committed trace reads");
    let request = bound_request();
    let response = run(&trial, &request.analysis).expect("the trial computes");
    // The stamp the run writes its records against, so the two sides describe one registry.
    let loaded = registry();
    accounts_of(
        &response,
        &RegistryStamp {
            version: None,
            declared_version: loaded.declared_version.clone(),
            digest: Some(loaded.content_digest.clone()),
        },
        false,
    )
}

fn run_over_one_trial() -> plateforce_batch::BatchResult {
    let directory = tempdir("descriptions");
    std::fs::copy(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        directory.join(TRIAL_FILE),
    )
    .expect("the fixture copies");

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem)
        .expect("the folder walks");
    analyse(&set, &bound_request(), &registry()).expect("the run produces a result")
}

/// Every account the engine wrote reaches the run, word for word.
#[test]
fn every_account_the_engine_wrote_reaches_the_run() {
    let written = accounts_the_engine_wrote();
    assert!(
        written.len() >= 8,
        "the engine described only {} quantities on this trial, so this guard could not fail",
        written.len()
    );

    let result = run_over_one_trial();
    println!(
        "the engine wrote {} accounts, the run carries {} rows",
        written.len(),
        result.descriptions.len()
    );
    assert_eq!(result.descriptions.len(), written.len());

    for row in &result.descriptions {
        assert_eq!(row.trial_id, TRIAL_ID);
        assert_eq!(
            written.get(&row.quantity),
            Some(&row.account),
            "the run's account of {} is not the one the engine wrote",
            row.quantity
        );
        // The join back to the rules as rows. A blank here leaves a reader with the sentence
        // and no way to reach the same decisions as data.
        assert!(!row.provenance_id.is_empty(), "{row:?}");
    }

    // Every account belongs to a column the table holds, or a reader joining the two finds
    // nothing and concludes the account is about something else.
    let quantities: BTreeSet<&str> = result.quantities.iter().map(String::as_str).collect();
    for row in &result.descriptions {
        assert!(
            quantities.contains(row.quantity.as_str()),
            "the run describes {}, which results does not carry: {quantities:?}",
            row.quantity
        );
    }
}

/// A relation nobody writes is a relation nobody reads, and this is the one whose cells hold
/// the record separator.
///
/// So it is read back off disk rather than counted: a writer that emitted the account's lines
/// as bare newlines would produce a file whose row count looks right to a reader splitting on
/// them and whose every row after the first is wrong.
#[test]
fn a_run_writes_its_accounts_beside_its_numbers_and_reads_them_back_whole() {
    let result = run_over_one_trial();
    let out = tempdir("descriptions-out");
    let written = result.write_csv(&out).expect("the tables write");

    let names: BTreeSet<String> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    println!("{names:?}");
    assert!(names.contains("descriptions.csv"), "{names:?}");
    assert_eq!(Relation::Descriptions.file_name(), "descriptions.csv");

    let text =
        std::fs::read_to_string(out.join("descriptions.csv")).expect("the relation is on disk");
    assert!(
        text.starts_with("trial_id,quantity,provenance_id,account\n"),
        "{}",
        text.lines().next().unwrap_or_default()
    );
    // The account states its chain over several lines, so the file holds more lines than rows.
    // Reading it as one row per line is the mistake this relation invites.
    assert!(
        text.lines().count() > 1 + result.descriptions.len(),
        "{} lines for {} rows, so no account carried its chain",
        text.lines().count(),
        result.descriptions.len()
    );

    let read = plateforce_batch::read_csv(&out).expect("the tables read back");
    assert_eq!(read.descriptions, result.descriptions);
    println!(
        "{} rows over {} lines, read back whole",
        result.descriptions.len(),
        text.lines().count()
    );
}

/// The envelope every non-terminal surface returns. An account that travels in the CSV and
/// not in the JSON reaches a reader who opened the folder and not one who called the library.
#[test]
fn the_envelope_carries_the_accounts_the_tables_carry() {
    let result = run_over_one_trial();
    let envelope: serde_json::Value =
        serde_json::from_str(&result.to_json()).expect("the envelope parses");
    let rows = envelope["ok"]["descriptions"]
        .as_array()
        .expect("the envelope carries the relation");

    // The population the comparison runs over. Two empty lists match perfectly, and a run
    // carrying no account would pass a comparison against its own emptiness.
    assert!(
        result.descriptions.len() >= 8,
        "the run carries {} accounts, so this would compare almost nothing",
        result.descriptions.len()
    );
    assert_eq!(rows.len(), result.descriptions.len());
    for (row, written) in rows.iter().zip(&result.descriptions) {
        assert_eq!(row["quantity"].as_str(), Some(written.quantity.as_str()));
        assert_eq!(row["account"].as_str(), Some(written.account.as_str()));
    }
    println!("{} rows in the envelope", rows.len());
}
