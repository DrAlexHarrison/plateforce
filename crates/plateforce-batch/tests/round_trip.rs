//! What went out comes back the same.

mod common;

use common::{bound_request, committed_format, copy_committed_fixtures, registry, tempdir};
use plateforce_batch::{analyse, read_csv, BatchResult, TrialIdentity, TrialSet};

fn run_over_fixtures(name: &str) -> (std::path::PathBuf, BatchResult, usize) {
    let directory = tempdir(name);
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(result.coverage.computed, copied, "every trial computed");
    (directory, result, copied)
}

#[test]
fn envelope_carries_one_key_and_survives_the_round_trip() {
    let (directory, result, copied) = run_over_fixtures("round-trip-json");
    let text = result.to_json();

    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(
        value.get("ok").is_some(),
        "a run that produced numbers is ok"
    );
    assert!(value.get("refusal").is_none(), "and never both keys");

    let back = BatchResult::from_json(&text).expect("the envelope reads back");
    println!(
        "json: {} of {copied} result rows, {} provenance rows, {} refusals",
        back.results.len(),
        back.provenance.len(),
        back.refusals.len()
    );
    assert_eq!(back.results, result.results);
    assert_eq!(back.provenance, result.provenance);
    assert_eq!(back.refusals, result.refusals);
    assert_eq!(back.run, result.run);
    assert_eq!(back.quantities, result.quantities);

    // The same result rendered twice is the same string, which is what makes a comparison
    // across surfaces byte for byte rather than approximate.
    assert_eq!(text, result.to_json());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_refused_run_carries_the_other_key_and_never_both() {
    let directory = tempdir("round-trip-refusal");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let request = plateforce_batch::BatchRequest::new(common::analysis_request(1.0));
    let refusal = analyse(&set, &request, &registry()).unwrap_err();

    let text = refusal.to_json();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(value.get("refusal").is_some());
    assert!(value.get("ok").is_none());
    println!("{text}");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_csv_set_rebuilds_the_run_from_the_record_beside_the_table() {
    let (directory, result, copied) = run_over_fixtures("round-trip-csv");
    let out = directory.join("out");
    result.write_csv(&out).unwrap();

    let back = read_csv(&out).expect("the set reads back");
    println!(
        "csv: {} of {copied} result rows, {} provenance rows, {} refusals",
        back.results.len(),
        back.provenance.len(),
        back.refusals.len()
    );
    assert_eq!(back.run, result.run, "the run row came out of run.json");
    assert_eq!(back.quantities, result.quantities);
    assert_eq!(back.provenance, result.provenance);
    assert_eq!(back.refusals, result.refusals);
    assert_eq!(back.results.len(), result.results.len());
    for (read, original) in back.results.iter().zip(result.results.iter()) {
        assert_eq!(read.trial_id, original.trial_id);
        assert_eq!(read.provenance_id, original.provenance_id);
        assert_eq!(read.refusal_code, original.refusal_code);
        for quantity in &result.quantities {
            assert_eq!(
                read.values.get(quantity),
                original.values.get(quantity),
                "{quantity} on {}",
                original.trial_id
            );
        }
    }
    std::fs::remove_dir_all(&directory).ok();
}

#[cfg(feature = "parquet")]
#[test]
fn the_parquet_set_carries_the_run_in_its_own_schema_metadata() {
    use plateforce_batch::write_parquet::{read_relation, read_run};

    let (directory, result, copied) = run_over_fixtures("round-trip-parquet");
    let out = directory.join("out");
    let written = result.write_parquet(&out).unwrap();

    let run = read_run(&written[0]).expect("the record survives the container");
    println!(
        "parquet: {} files, run over {copied} trials, fingerprint {:?}",
        written.len(),
        run.run_fingerprint
    );
    assert_eq!(run, result.run);

    let batches = read_relation(&out.join("results.parquet")).unwrap();
    let rows: usize = batches.iter().map(|batch| batch.num_rows()).sum();
    assert_eq!(rows, result.results.len(), "every row came back");
    let columns = batches[0].num_columns();
    assert_eq!(
        columns,
        plateforce_batch::relations::ResultRow::header(&result.quantities).len(),
        "and every column, with the quantities still numbers"
    );
    std::fs::remove_dir_all(&directory).ok();
}
