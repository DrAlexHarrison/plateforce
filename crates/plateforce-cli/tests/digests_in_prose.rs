//! Every registry digest written into a committed file is the one this registry answers.
//!
//! A digest in prose is a provenance figure a reader checks the software against, and it is
//! the one figure nobody notices going stale: a registry data edit moves it, the files that
//! quote it do not fail to compile, and the next reader downloads the release the README
//! names and gets different bytes.
//!
//! Scoped to the real shape, `content-` and sixteen hex digits, so the stand-ins that tests
//! use on purpose (`content-abc`, `content-test`, `fnv1a-deadbeef`) are outside it. Those say
//! nothing about any registry and holding them to one would force a fixture to be edited
//! every time the data changes.
//!
//! Not everything of that shape is a digest of the registry. One digest function serves the
//! whole product, so the revision of a saved plate is `content-` and sixteen hex digits as
//! well, and a scan by shape cannot tell the two apart. The one file quoting such a digest is
//! held to what its digest is a digest of, measured by this build from the members its own
//! request states. An allow-list in its place would be somewhere a stale registry digest could
//! sit.
//!
//! The figure prose is held to is the one the shipped command reports, which is the registry
//! compiled into the binary a reader runs. That the binary carries the repository's `registry/`
//! bytes is a second rule and has one guard of its own,
//! `registry_source::tests::the_registry_in_the_binary_is_the_registry_in_the_repository`.
//! Reading the directory again here would be a second implementation of that rule inside a
//! file about a different one, which is what a scan over the whole tree used to do beside this
//! one until it was collapsed in: that scan passed a correct digest written into an unlisted
//! committed file, where `registry_digest_files_are_exactly_the_files_that_quote_one` below fails
//! it, so the wider-looking guard was the narrower one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The exact population of worked examples that quote this registry's digest.
///
/// The tree scan below is held equal to this set. A file appearing fails as an unreviewed
/// example, and a file leaving fails as a stale population member.
const REGISTRY_DIGEST_FILES: [&str; 2] = [
    "crates/plateforce-core/src/provenance.rs",
    "crates/plateforce-python/README.md",
];

/// The parity record of the request that states a saved plate, and the request that states it.
///
/// The record carries the revision the plate's members hash to, which is a fact about those
/// members and moves with no registry edit at all. Held to the members rather than to the
/// registry, and neither figure is written here: the request states the members, this build
/// hashes them, and the two meet.
const PLATE_RECORD: &str = "tests/golden/result-parity-plate.json";
const PLATE_REQUEST: &str = "tests/golden/result-parity-request-plate.json";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root resolves")
}

/// What this build's registry answers when asked, rather than a literal written here. A
/// constant would be one more digest in prose, checked by nothing.
fn the_digest_this_registry_answers() -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["--format", "json", "registry", "validate"])
        .current_dir(repository())
        .output()
        .expect("the built binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answered: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the report parses");
    answered["ok"]["registry_digest"]
        .as_str()
        .expect("the report names a digest")
        .to_string()
}

/// Every `content-` followed by exactly sixteen hex digits, with the line it sits on.
fn digests_in(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(at) = rest.find("content-") {
            let after = &rest[at + "content-".len()..];
            let hex: String = after
                .chars()
                .take_while(|character| character.is_ascii_hexdigit())
                .collect();
            // Exactly sixteen: a longer run is not this shape either, and reading its first
            // sixteen would compare a prefix of something else against a whole digest.
            let ends_here = after[hex.len()..]
                .chars()
                .next()
                .is_none_or(|character| !character.is_ascii_alphanumeric());
            if hex.len() == 16 && ends_here {
                found.push((number + 1, format!("content-{hex}")));
            }
            rest = &rest[at + "content-".len()..];
        }
    }
    found
}

#[test]
fn every_digest_in_prose_is_the_one_the_registry_answers() {
    let answered = the_digest_this_registry_answers();
    let root = repository();

    let mut checked = 0;
    let mut wrong = Vec::new();
    for name in REGISTRY_DIGEST_FILES {
        let path = root.join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{name} is listed here and unreadable: {error}"));
        let quoted = digests_in(&text);
        assert!(
            !quoted.is_empty(),
            "{name} is listed as quoting a digest and quotes none, so this guard is watching \
             a file that has nothing to watch"
        );
        for (line, digest) in quoted {
            checked += 1;
            if digest != answered {
                wrong.push(format!("{name}:{line} says {digest}"));
            }
        }
    }

    // The control. A guard over no digests passes every assertion above it.
    assert!(
        checked >= REGISTRY_DIGEST_FILES.len(),
        "only {checked} digests were read across {} files",
        REGISTRY_DIGEST_FILES.len()
    );

    assert!(
        wrong.is_empty(),
        "this registry answers {answered}, and {} of {checked} quoted digests do not:\n    {}",
        wrong.len(),
        wrong.join("\n    ")
    );
}

/// The revision the plate the parity request states hashes to, from this build.
///
/// Saved through the command a reader saves a plate with, into a folder this test owns, so the
/// figure comes from the same path a run gets it from rather than from a second hash taken
/// here. The machine's own saved plates are never touched.
fn the_revision_the_parity_plate_hashes_to(root: &Path) -> String {
    let text = std::fs::read_to_string(root.join(PLATE_REQUEST))
        .expect("the parity request that states a plate is readable");
    let asked: serde_json::Value = serde_json::from_str(&text).expect("the request parses");
    let plate = &asked["capture"]["plate"];
    let name = plate["name"].as_str().expect("the request names the plate");
    let members: BTreeMap<String, String> =
        serde_json::from_value(plate["members"].clone()).expect("the request states its members");

    let folder =
        std::env::temp_dir().join(format!("plateforce-digest-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);

    let mut command = Command::new(env!("CARGO_BIN_EXE_plateforce"));
    command.args([
        "--plates",
        &folder.display().to_string(),
        "--format",
        "json",
        "plate",
        "save",
        name,
    ]);
    for (member, value) in &members {
        command.args(["--acquisition", &format!("{member}={value}")]);
    }
    let output = command
        .current_dir(root)
        .output()
        .expect("the built binary runs");
    let _ = std::fs::remove_dir_all(&folder);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the save report parses");
    saved["ok"]["revision"]
        .as_str()
        .expect("the save report names a revision")
        .to_string()
}

/// Every digest in the parity record is the one the plate's own members hash to.
///
/// The record is not held to the registry, because the figure it quotes is not the registry's:
/// it is what a reader's saved plate hashes to, and it moves when a member of that plate moves
/// and at no other time. Read by the same scan as the registry's digests, so a second digest
/// appearing in that file is held here rather than passed over on the file's name.
#[test]
fn every_digest_in_the_parity_plate_record_is_the_one_its_members_hash_to() {
    let root = repository();
    let hashed = the_revision_the_parity_plate_hashes_to(&root);
    let text = std::fs::read_to_string(root.join(PLATE_RECORD))
        .expect("the parity record of the plate request is readable");

    let quoted = digests_in(&text);
    assert!(
        !quoted.is_empty(),
        "{PLATE_RECORD} is held to the revision its members hash to and quotes no digest at \
         all, so this guard is watching a file that has nothing to watch"
    );

    // Read off the record as well as scanned for, because the scan finds a shape and this
    // names the field. A revision that moved out of `plate_profile` into some other key would
    // still be found by the scan and would no longer be the thing this guard is about.
    let record: serde_json::Value = serde_json::from_str(&text).expect("the record parses");
    assert_eq!(
        record["carried_by_some"]["plate_profile"]["revision"].as_str(),
        Some(hashed.as_str()),
        "the record's plate revision is not what this build hashes those members to"
    );

    let wrong: Vec<String> = quoted
        .iter()
        .filter(|(_, digest)| *digest != hashed)
        .map(|(line, digest)| format!("{PLATE_RECORD}:{line} says {digest}"))
        .collect();
    assert!(
        wrong.is_empty(),
        "the plate these members describe hashes to {hashed}, and {} of {} quoted digests do \
         not:\n    {}",
        wrong.len(),
        quoted.len(),
        wrong.join("\n    ")
    );
}

/// The files that quote this registry's digest are exactly the stated population.
#[test]
fn registry_digest_files_are_exactly_the_files_that_quote_one() {
    let root = repository();
    let tracked = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .expect("git lists the tracked files");
    assert!(tracked.status.success());

    let expected: BTreeSet<String> = REGISTRY_DIGEST_FILES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let mut found = BTreeSet::new();
    let mut scanned = 0;
    for name in String::from_utf8_lossy(&tracked.stdout).split('\0') {
        // The plate record is held too, by the test above, to the members its own request
        // states rather than to the registry. Skipped here and asserted there, never exempt.
        if name.is_empty() || name == PLATE_RECORD {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(name)) else {
            continue; // Binary, or a path this checkout does not hold.
        };
        scanned += 1;
        if !digests_in(&text).is_empty() {
            found.insert(name.to_string());
        }
    }

    assert!(scanned > 100, "only {scanned} tracked files were read");
    assert_eq!(
        found, expected,
        "the files quoting a registry-shaped digest differ from the stated population"
    );
}
