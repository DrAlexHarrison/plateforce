//! A table of numbers and the record of what produced them travel together or not at all.

mod common;

use common::{bound_request, committed_format, copy_committed_fixtures, registry, tempdir};
use plateforce_batch::{analyse, Relation, TrialIdentity, TrialSet, WriteRefusal};

fn run_over_fixtures(name: &str) -> (std::path::PathBuf, plateforce_batch::BatchResult) {
    let directory = tempdir(name);
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    (directory, result)
}

#[test]
fn four_files_or_none() {
    let (directory, result) = run_over_fixtures("csv-four-files");
    let out = directory.join("out");
    let written = result.write_csv(&out).expect("the directory takes them");

    let names: Vec<String> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    println!("wrote {} files: {}", names.len(), names.join(", "));

    for required in ["run.json", "results.csv", "provenance.csv", "refusals.csv"] {
        assert!(
            out.join(required).exists(),
            "{required} is beside the others"
        );
    }
    // No aggregation was bound, so the fifth relation is absent rather than empty.
    assert!(!out.join("aggregates.csv").exists());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_table_without_its_record_is_refused_and_says_where_the_record_goes() {
    let (directory, result) = run_over_fixtures("csv-refuses");
    let out = directory.join("out");

    let refusal = result
        .write_csv_selection(&out, &[Relation::Results])
        .expect_err("a bare table of numbers is the artefact that starts the problem");

    let message = refusal.to_string();
    println!("{message}");
    assert!(matches!(refusal, WriteRefusal::RecordNotRequested { .. }));
    assert!(
        message.contains(out.to_str().unwrap()),
        "it names the directory"
    );
    assert!(
        message.contains("run.json"),
        "and what goes beside the table"
    );
    assert!(
        !out.join("results.csv").exists(),
        "and nothing was written before the refusal"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_record_lands_before_any_table() {
    let (directory, result) = run_over_fixtures("csv-record-first");
    let out = directory.join("out");
    let written = result.write_csv(&out).unwrap();
    assert_eq!(
        written[0].file_name().unwrap(),
        "run.json",
        "a directory that cannot take the record costs no half-written set"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_table_carries_the_join_back_to_the_record() {
    let (directory, result) = run_over_fixtures("csv-join");
    let out = directory.join("out");
    result.write_csv(&out).unwrap();

    let results = std::fs::read_to_string(out.join("results.csv")).unwrap();
    let header = results.lines().next().unwrap();
    println!("{header}");
    assert!(
        header.starts_with("trial_id,subject,source_path,provenance_id,refusal_code,"),
        "{header}"
    );
    assert!(
        header.contains("jump_height_from_takeoff_meters"),
        "the quantity columns come from the response rather than a list here"
    );
    std::fs::remove_dir_all(&directory).ok();
}
