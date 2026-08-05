//! The fingerprint rides in the file, under a key that is not private to one language.

#![cfg(feature = "parquet")]

mod common;

use common::{
    bound_request_describing_the_plate, committed_format, copy_committed_fixtures, registry,
    tempdir,
};
use plateforce_batch::write_parquet::{read_run, ParquetError, RUN_METADATA_KEY};
use plateforce_batch::{analyse, TrialIdentity, TrialSet};

#[test]
fn the_run_block_survives_a_parquet_round_trip_under_our_own_key() {
    let directory = tempdir("parquet-metadata");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    // The plate is described, so the record this file is asserted to carry holds a published
    // fingerprint. Over an unfilled block the digest is withheld and the assertion below would
    // be met by a container carrying nothing.
    let result = analyse(&set, &bound_request_describing_the_plate(), &registry()).unwrap();
    assert_eq!(result.coverage.computed, copied, "every trial computed");

    let out = directory.join("out");
    let written = result.write_parquet(&out).unwrap();
    println!("wrote {} relations to parquet", written.len());

    for path in &written {
        let run = read_run(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            run,
            result.run,
            "{} carries the same record as every other file in the set",
            path.display()
        );
        assert!(
            run.run_fingerprint.is_some(),
            "the run described its plate, so the record carries a fingerprint"
        );
        assert_eq!(run.trial_count, copied);
    }
    println!(
        "run over {} of {copied} trials, digest {}, fingerprint {:?}",
        result.run.trial_count, result.run.registry_digest, result.run.run_fingerprint
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_key_is_ours_rather_than_a_language_private_block() {
    // `r` and `pandas` are read by one language each, so a fingerprint inside either is
    // invisible to the other. The key is asserted by name rather than left to a convention.
    assert_eq!(RUN_METADATA_KEY, "plateforce");
    assert_ne!(RUN_METADATA_KEY, "r");
    assert_ne!(RUN_METADATA_KEY, "pandas");
}

#[test]
fn a_file_with_no_record_is_named_rather_than_read_as_if_it_had_one() {
    let directory = tempdir("parquet-no-record");
    let path = directory.join("not-ours.parquet");
    std::fs::write(&path, b"not a parquet file").unwrap();
    let error = read_run(&path).unwrap_err();
    println!("{error}");
    assert!(
        matches!(
            error,
            ParquetError::Arrow { .. } | ParquetError::RecordAbsent { .. }
        ),
        "{error}"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// The two containers describe one table, so their column names agree.
///
/// The parquet schema is written out by hand and the CSV header is generated, so the two are
/// free to drift and a reader gets a different shape depending on which file they open. The
/// subject column was added to the CSV first and would have been absent here.
#[test]
fn the_parquet_results_schema_names_what_the_csv_header_names() {
    use plateforce_batch::relations::ResultRow;

    let directory = tempdir("parquet-schema");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request_describing_the_plate(), &registry()).unwrap();

    let out = directory.join("out");
    result.write_parquet(&out).unwrap();

    let path = out.join("results.parquet");
    let file = std::fs::File::open(&path).unwrap();
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let in_parquet: Vec<String> = reader
        .schema()
        .fields()
        .iter()
        .map(|field| field.name().clone())
        .collect();

    let in_csv = ResultRow::header(&result.quantities);
    assert_eq!(
        in_parquet, in_csv,
        "the two containers describe one table under different column names, so a reader gets \
         a different shape depending on which file they open",
    );
    println!(
        "both containers name {} columns, in one order",
        in_csv.len()
    );
}
