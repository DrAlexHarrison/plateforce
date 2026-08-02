//! What the run reads, and what it calls each thing it read.

mod common;

use std::collections::BTreeSet;

use common::{tempdir, FIXTURES};
use plateforce_batch::identity::{UnidentifiedReason, WalkError};
use plateforce_batch::{Session, SourceFormat, TrialIdentity, TrialSet};

fn committed_format() -> SourceFormat {
    SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["force.txt".to_string()],
        sentinel: None,
    }
}

#[test]
fn reads_every_committed_fixture() {
    let format = committed_format();
    let set = TrialSet::walk(
        std::path::Path::new(FIXTURES),
        &format,
        &TrialIdentity::FileStem,
    )
    .expect("the committed fixtures are on disk");

    println!(
        "{} of {} committed fixtures read",
        set.len(),
        set.files_found
    );
    // The count comes from the directory rather than from a literal here. Other workstreams
    // land fixtures, and a number written into this file goes stale into a green pass.
    let traces = std::fs::read_dir(FIXTURES)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with("force.txt"))
        .count();
    assert!(traces >= 6, "the committed set only grows");
    assert_eq!(
        set.files_found, traces,
        "every trace carries the declared suffix"
    );
    assert_eq!(set.len(), traces, "and every one of them is named");
    assert!(set.unidentified.is_empty());

    // The files that are not traces sit outside the declared set rather than being refused as
    // data failures, and the narrowing is itself what `files_found` records.
    let on_disk = std::fs::read_dir(FIXTURES).unwrap().count();
    println!(
        "{} of {on_disk} files in the directory are declared trials",
        set.files_found
    );
    assert!(on_disk > set.files_found);

    for (trial_id, entry) in set.iter() {
        let (trial, report, _) = entry
            .source
            .read(&format)
            .unwrap_or_else(|error| panic!("{trial_id}: {error}"));
        assert!(trial.len() > 1000, "{trial_id} carries a trace");
        assert_eq!(report.column_index, 0);
    }
}

#[test]
fn a_declared_pattern_parses_what_the_conformance_crate_parses() {
    let format = SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["txt".to_string()],
        sentinel: None,
    };
    let identity = TrialIdentity::DeclaredPattern {
        template: "AT{subject}_{trial}".to_string(),
    };
    let directory = tempdir("pattern-parses");
    for name in ["AT01_6.txt", "AT13_3.txt", "notes.txt"] {
        std::fs::write(directory.join(name), "600.0\n600.0\n").unwrap();
    }
    std::fs::write(directory.join("AT01_6.csv"), "600.0\n").unwrap();

    let set = TrialSet::walk(&directory, &format, &identity).unwrap();

    // The conformance crate accepts the first two and rejects the last two, and so does this.
    let named: Vec<&String> = set.iter().map(|(id, _)| id).collect();
    println!(
        "named {} of {} declared trials",
        named.len(),
        set.files_found
    );
    assert_eq!(named, vec!["AT01_6", "AT13_3"]);

    let subjects: BTreeSet<String> = set
        .iter()
        .filter_map(|(_, entry)| entry.subject.as_ref().map(|key| key.subject.clone()))
        .collect();
    assert_eq!(
        subjects,
        BTreeSet::from(["01".to_string(), "13".to_string()])
    );

    // `AT01_6.csv` carries no declared suffix, so it is not a trial and not a failure.
    assert_eq!(set.files_found, 3, "three names carry the declared suffix");
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_file_the_pattern_does_not_match_is_refused_by_name_rather_than_skipped() {
    let format = SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["txt".to_string()],
        sentinel: None,
    };
    let identity = TrialIdentity::DeclaredPattern {
        template: "AT{subject}_{trial}".to_string(),
    };
    let directory = tempdir("pattern-refuses");
    std::fs::write(directory.join("AT01_6.txt"), "600.0\n").unwrap();
    std::fs::write(directory.join("notes.txt"), "600.0\n").unwrap();

    let set = TrialSet::walk(&directory, &format, &identity).unwrap();

    println!(
        "named {} of {} declared trials, unidentified {} of {}",
        set.len(),
        set.files_found,
        set.unidentified.len(),
        set.files_found
    );
    assert_eq!(set.files_found, 2);
    assert_eq!(set.len(), 1);
    assert_eq!(
        set.len() + set.unidentified.len(),
        set.files_found,
        "every file the run found is either named or refused by name"
    );

    let refused = &set.unidentified[0];
    assert_eq!(refused.file_name, "notes.txt");
    assert!(
        refused.message().contains("AT{subject}_{trial}"),
        "{}",
        refused.message()
    );
    assert!(matches!(
        refused.reason,
        UnidentifiedReason::PatternDidNotMatch { .. }
    ));
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn two_files_resolving_to_one_id_are_both_named_and_neither_is_preferred() {
    let format = committed_format();
    let directory = tempdir("duplicate-ids");
    std::fs::create_dir_all(directory.join("monday")).unwrap();
    std::fs::create_dir_all(directory.join("tuesday")).unwrap();
    std::fs::write(directory.join("monday/a.force.txt"), "600.0\n").unwrap();
    std::fs::write(directory.join("tuesday/a.force.txt"), "600.0\n").unwrap();

    let set = TrialSet::walk(&directory, &format, &TrialIdentity::FileStem).unwrap();

    println!(
        "named {} of {}, unidentified {} of {}",
        set.len(),
        set.files_found,
        set.unidentified.len(),
        set.files_found
    );
    assert_eq!(set.files_found, 2);
    assert_eq!(set.len(), 0, "neither file keys a row");
    assert_eq!(set.unidentified.len(), 2, "both are named");
    assert!(set.unidentified[0].message().contains("trial id a"));
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_run_walked_twice_lists_its_trials_in_the_same_order() {
    let format = committed_format();
    let first = TrialSet::walk(
        std::path::Path::new(FIXTURES),
        &format,
        &TrialIdentity::FileStem,
    )
    .unwrap();
    let second = TrialSet::walk(
        std::path::Path::new(FIXTURES),
        &format,
        &TrialIdentity::FileStem,
    )
    .unwrap();
    assert_eq!(first.trial_ids(), second.trial_ids());
    println!(
        "{} of {} trials in one order",
        first.len(),
        first.files_found
    );
}

#[test]
fn grouping_by_subject_is_unavailable_when_no_pattern_was_declared() {
    let format = committed_format();
    let set = TrialSet::walk(
        std::path::Path::new(FIXTURES),
        &format,
        &TrialIdentity::FileStem,
    )
    .unwrap();
    assert!(
        Session::group(&set).is_none(),
        "a run with no declared grouping has no subject to group by"
    );
}

#[test]
fn a_declared_pattern_groups_one_subjects_trials_from_one_occasion() {
    let format = SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["txt".to_string()],
        sentinel: None,
    };
    let identity = TrialIdentity::DeclaredPattern {
        template: "AT{subject}_{trial}".to_string(),
    };
    let directory = tempdir("grouping");
    let generated = plateforce_batch::synthetic::write_corpus(&directory, 5, 4, 7).unwrap();

    let set = TrialSet::walk(&directory, &format, &identity).unwrap();
    let sessions = Session::group(&set).expect("a declared pattern groups");

    println!(
        "{} of {} files parsed into {} subjects x {} trials",
        set.len(),
        generated.len(),
        sessions.len(),
        sessions.first().map(|s| s.trial_ids.len()).unwrap_or(0)
    );
    assert_eq!(set.len(), 20);
    assert_eq!(sessions.len(), 5);
    assert!(sessions.iter().all(|session| session.trial_ids.len() == 4));
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_run_that_declares_no_trial_names_is_refused_rather_than_given_a_default() {
    let format = SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: Vec::new(),
        sentinel: None,
    };
    let error = TrialSet::walk(
        std::path::Path::new(FIXTURES),
        &format,
        &TrialIdentity::FileStem,
    )
    .unwrap_err();
    assert!(matches!(error, WalkError::NoTrialFileSuffixes), "{error}");
}

/// The browser has no filesystem, so it hands the engine named text rather than a directory.
/// One engine serves both, and the two entry points have to agree: a person who drops a
/// folder on the page and a person who points the terminal at it are asking one question.
#[test]
fn the_same_folder_read_from_memory_and_from_disk_gives_one_answer() {
    let directory = common::tempdir("memory-versus-disk");
    plateforce_batch::synthetic::write_corpus(&directory, 3, 3, 7).unwrap();
    // A file that is not a trial, so both paths have a narrowing to agree about rather than
    // agreeing because there was nothing to exclude.
    std::fs::write(directory.join("README.md"), "not a trace\n").unwrap();
    let format = common::synthetic_format();
    let identity = common::declared_pattern();

    let mut sources: Vec<(String, String)> = std::fs::read_dir(&directory)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                std::fs::read_to_string(entry.path()).unwrap(),
            )
        })
        .collect();
    sources.sort();

    let walked = TrialSet::walk(&directory, &format, &identity).unwrap();
    let handed = TrialSet::from_sources(sources, &format, &identity).unwrap();
    println!(
        "walked {} of {} named, handed {} of {} named",
        walked.len(),
        walked.files_found,
        handed.len(),
        handed.files_found
    );
    assert_eq!(walked.len(), 9, "three subjects of three");
    assert_eq!(
        walked.files_found, 9,
        "and the file that is not a trial is outside the denominator, not refused"
    );
    assert_eq!(
        walked.trial_ids(),
        handed.trial_ids(),
        "same trials, same order"
    );
    assert_eq!(walked.files_found, handed.files_found);

    let request = common::bound_request();
    let registry = common::registry();
    let from_disk = plateforce_batch::analyse(&walked, &request, &registry).unwrap();
    let from_memory = plateforce_batch::analyse(&handed, &request, &registry).unwrap();

    // A declared pattern writes no path, so the two envelopes have nothing left to differ by.
    assert_eq!(
        from_disk.to_json(),
        from_memory.to_json(),
        "the browser and the terminal answer the same folder the same way"
    );
    println!(
        "envelopes agree byte for byte, {} bytes, fingerprint {}",
        from_disk.to_json().len(),
        from_disk.run.run_fingerprint
    );
    std::fs::remove_dir_all(&directory).ok();
}
