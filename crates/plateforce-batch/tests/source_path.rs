//! What a run writes about where its files came from.
//!
//! A directory of force traces is commonly organised by athlete, so the path is identifying
//! even when nothing inside the file is. A run that already carries the subject from a
//! declared pattern has no need of the path and does not write it.

mod common;

use common::{bound_request, declared_pattern, registry, synthetic_format, tempdir};
use plateforce_batch::{analyse, TrialIdentity, TrialSet};

/// The directory name stands in for the folder-per-athlete layout the corpus actually uses.
const NAMING_DIRECTORY: &str = "one-persons-name";

/// Its own directory per test, because these run in parallel and each one asserts against
/// what it put there.
fn corpus(test: &str) -> std::path::PathBuf {
    let directory = tempdir(&format!("{NAMING_DIRECTORY}-{test}"));
    plateforce_batch::synthetic::write_corpus(&directory, 2, 2, 7).unwrap();
    directory
}

#[test]
fn a_declared_pattern_carries_the_subject_and_writes_no_path() {
    let directory = corpus("no-path");
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("every choice was made");

    let written: Vec<&str> = result
        .results
        .iter()
        .map(|row| row.source_path.as_str())
        .filter(|path| !path.is_empty())
        .collect();
    println!(
        "{} of {} rows carry a path, and the run records its identity as {}",
        written.len(),
        result.results.len(),
        result.run.trial_identity
    );
    assert!(
        written.is_empty(),
        "the subject came from the name, so these paths were written for nothing: {written:?}"
    );
    assert!(
        result.run.trial_identity.starts_with("declared_pattern"),
        "and the omission is recorded rather than silent: {}",
        result.run.trial_identity
    );
    assert!(
        result
            .results
            .iter()
            .any(|row| !row.provenance_id.is_empty()),
        "over a run that produced numbers"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn nothing_a_declared_run_writes_to_disk_names_the_directory_it_read() {
    let directory = corpus("on-disk");
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let out = directory.join("out");
    let files = result.write_csv(&out).unwrap();
    let mut checked = 0;
    for path in &files {
        let body = std::fs::read_to_string(path).unwrap();
        assert!(
            !body.contains(NAMING_DIRECTORY),
            "{} carries the directory it read",
            path.display()
        );
        checked += 1;
    }
    println!("{checked} of {} written files carry no path", files.len());
    assert!(checked >= 4, "and the run wrote its whole set");
    std::fs::remove_dir_all(&directory).ok();
}

/// The other half, so the guard cannot be satisfied by never writing a path at all. Without a
/// pattern the file is the only thing naming the trial, and dropping it loses the provenance.
#[test]
fn without_a_pattern_the_path_is_the_identity_and_is_written() {
    let directory = corpus("file-stem");
    let set = TrialSet::walk(&directory, &synthetic_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();

    let missing = result
        .results
        .iter()
        .filter(|row| row.source_path.is_empty())
        .count();
    println!(
        "{} of {} rows name the file they came from, identity {}",
        result.results.len() - missing,
        result.results.len(),
        result.run.trial_identity
    );
    assert_eq!(missing, 0, "every row names the file it came from");
    assert_eq!(result.run.trial_identity, "file_stem");
    std::fs::remove_dir_all(&directory).ok();
}
