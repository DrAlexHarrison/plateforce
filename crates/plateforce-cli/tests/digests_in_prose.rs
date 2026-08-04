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

use std::path::{Path, PathBuf};
use std::process::Command;

/// Files a digest is allowed to appear in, each because it is a worked example a reader acts
/// on. Listed rather than discovered: a new file quoting a digest is a decision somebody
/// makes, and the point of a list is that it is read in review.
const FILES_THAT_QUOTE_A_DIGEST: [&str; 2] = [
    "crates/plateforce-core/src/provenance.rs",
    "crates/plateforce-python/README.md",
];

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
    for name in FILES_THAT_QUOTE_A_DIGEST {
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
        checked >= FILES_THAT_QUOTE_A_DIGEST.len(),
        "only {checked} digests were read across {} files",
        FILES_THAT_QUOTE_A_DIGEST.len()
    );

    assert!(
        wrong.is_empty(),
        "this registry answers {answered}, and {} of {checked} quoted digests do not:\n    {}",
        wrong.len(),
        wrong.join("\n    ")
    );
}

/// Every file in the tree that quotes a real digest is on the list above.
///
/// Without this the guard is only as wide as whoever last edited the list, and a new worked
/// example carrying a stale digest passes by not being mentioned.
#[test]
fn no_committed_file_quotes_a_digest_from_outside_the_list() {
    let root = repository();
    let tracked = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .expect("git lists the tracked files");
    assert!(tracked.status.success());

    let mut unlisted = Vec::new();
    let mut scanned = 0;
    for name in String::from_utf8_lossy(&tracked.stdout).split('\0') {
        if name.is_empty() || FILES_THAT_QUOTE_A_DIGEST.contains(&name) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(name)) else {
            continue; // Binary, or a path this checkout does not hold.
        };
        scanned += 1;
        for (line, digest) in digests_in(&text) {
            unlisted.push(format!("{name}:{line} says {digest}"));
        }
    }

    assert!(scanned > 100, "only {scanned} tracked files were read");
    assert!(
        unlisted.is_empty(),
        "these quote a registry digest and are not held to it:\n    {}",
        unlisted.join("\n    ")
    );
}
