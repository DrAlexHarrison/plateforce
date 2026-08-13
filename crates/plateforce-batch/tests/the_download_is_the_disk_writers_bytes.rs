//! The archive a tab downloads holds the files the disk writer writes, byte for byte.
//!
//! The browser's route is longer than the terminal's: the result crosses into JavaScript as
//! the envelope, comes back through `from_json`, and leaves as a zip. Each hop is a place
//! the two files could stop being the same file, so the whole route is held to the disk
//! writer here, without a browser.

mod common;

use common::{bound_request, committed_format, copy_committed_fixtures, registry, tempdir};
use plateforce_batch::{
    analyse, read_archive, BatchResult, TrialIdentity, TrialSet, EVERY_RELATION,
};

fn run_over_fixtures(name: &str) -> (std::path::PathBuf, BatchResult) {
    let directory = tempdir(name);
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(result.coverage.computed, copied, "every trial computed");
    (directory, result)
}

#[test]
fn the_archive_after_the_json_hop_matches_the_directory_byte_for_byte() {
    let (directory, result) = run_over_fixtures("archive-parity");
    let out = directory.join("out");
    let written = result.write_csv(&out).unwrap();

    // The tab's route: the envelope it holds, read back, archived.
    let envelope = result.to_json();
    let returned = BatchResult::from_json(&envelope).expect("the envelope reads back");
    let entries = read_archive(&returned.zip_archive()).expect("the archive reads back");

    assert_eq!(
        entries.len(),
        written.len(),
        "the archive and the directory hold different file sets"
    );
    assert_eq!(entries[0].0, "run.json", "the record leads the archive");

    let mut compared = 0usize;
    for (name, bytes) in &entries {
        let on_disk = std::fs::read(out.join(name)).expect("the directory holds the entry");
        assert_eq!(
            bytes.len(),
            on_disk.len(),
            "{name}: archive {} bytes, directory {} bytes",
            bytes.len(),
            on_disk.len()
        );
        assert_eq!(
            *bytes, on_disk,
            "{name} differs between archive and directory"
        );
        compared += 1;
    }
    println!(
        "{compared} of {} relations byte-identical across the JSON hop, {} archive bytes",
        EVERY_RELATION.len(),
        returned.zip_archive().len()
    );
    // A run with no reduction writes no aggregates.csv, so the comparison above covered
    // every relation but that one, and covering none would have compared nothing.
    assert_eq!(compared, EVERY_RELATION.len() - 1);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn one_result_archives_to_one_byte_sequence() {
    let (directory, result) = run_over_fixtures("archive-deterministic");
    assert_eq!(
        result.zip_archive(),
        result.zip_archive(),
        "two archives of one run are different files"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// The independent reader, so the container is not proven only by the code that wrote it.
/// Skipped silently nowhere: a machine without python3 fails loudly here.
#[test]
fn a_standard_unzip_opens_the_archive_and_agrees_on_every_checksum() {
    let (directory, result) = run_over_fixtures("archive-python");
    let path = directory.join("run.zip");
    std::fs::write(&path, result.zip_archive()).unwrap();

    let listing = std::process::Command::new("python3")
        .arg("-c")
        .arg(
            "import sys, zipfile\n\
             archive = zipfile.ZipFile(sys.argv[1])\n\
             bad = archive.testzip()\n\
             assert bad is None, f'checksum failed on {bad}'\n\
             print('\\n'.join(entry.filename for entry in archive.infolist()))",
        )
        .arg(&path)
        .output()
        .expect("python3 runs");
    assert!(
        listing.status.success(),
        "python3 zipfile rejected the archive: {}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let names: Vec<&str> = std::str::from_utf8(&listing.stdout)
        .unwrap()
        .lines()
        .collect();
    println!(
        "python3 zipfile read {} entries: {}",
        names.len(),
        names.join(", ")
    );
    assert_eq!(names.first(), Some(&"run.json"));
    assert!(names.contains(&"results.csv"));
    std::fs::remove_dir_all(&directory).ok();
}
