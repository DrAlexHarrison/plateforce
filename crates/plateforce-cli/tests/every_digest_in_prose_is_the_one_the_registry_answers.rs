//! A digest written into prose says which registry a result was produced under.
//!
//! A digest copied into documentation and compared against nothing goes on naming a registry
//! that no longer exists.
//!
//! One file states a digest of something else. The revision of a saved plate comes off the same
//! digest function as the registry's, so it is `content-` and sixteen hex digits too and no
//! scan by shape can tell them apart. It is held to the members the plate's own request states,
//! by `every_digest_in_the_parity_plate_record_is_the_one_its_members_hash_to` in
//! `digests_in_prose.rs`, and passed over here rather than held to a registry it says nothing
//! about.

use std::path::{Path, PathBuf};
use std::process::Command;

use plateforce_registry::Registry;

/// The one committed file quoting a digest that is not the registry's, held elsewhere to the
/// members it is a digest of.
const HELD_TO_ITS_OWN_MEMBERS: &str = "tests/golden/result-parity-plate.json";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed files, asked of git rather than found by a walk, so a build artefact holding
/// an older copy cannot fail a gate about what the repository publishes.
fn committed_files(root: &Path) -> Vec<PathBuf> {
    let listed = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .expect("git lists the committed files");
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    String::from_utf8_lossy(&listed.stdout)
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(|path| root.join(path))
        .collect()
}

/// Every `content-` followed by sixteen hex digits, which is the shape the digest prints in.
fn digests_in(text: &str) -> Vec<String> {
    text.match_indices("content-")
        .filter_map(|(at, _)| {
            let hex: String = text[at + "content-".len()..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .collect();
            (hex.len() == 16).then(|| format!("content-{hex}"))
        })
        .collect()
}

#[test]
fn every_digest_written_into_prose_is_the_one_the_registry_answers() {
    let root = repository_root();
    let answered = Registry::load(root.join("registry"))
        .expect("the registry loads")
        .content_digest;

    let mut seen = 0;
    let mut passed_over = 0;
    let mut disagreeing = Vec::new();
    for file in committed_files(&root) {
        if file.ends_with(HELD_TO_ITS_OWN_MEMBERS) {
            passed_over += 1;
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for written in digests_in(&text) {
            seen += 1;
            if written != answered {
                disagreeing.push(format!("{}: {written}", file.display()));
            }
        }
    }

    println!("digest literals in committed files: {seen}, registry answers {answered}");
    assert!(
        seen > 0,
        "no committed file states a digest, so this gate is reading nothing"
    );
    // The file passed over is passed over because another guard holds it, so it has to be
    // there to be passed over. Gone or renamed, this reads as coverage and covers nothing.
    assert_eq!(
        passed_over, 1,
        "{HELD_TO_ITS_OWN_MEMBERS} is passed over here because \
         every_digest_in_the_parity_plate_record_is_the_one_its_members_hash_to holds it, and \
         {passed_over} committed files matched that name"
    );
    assert!(
        disagreeing.is_empty(),
        "the registry answers {answered} and these say otherwise: {disagreeing:#?}"
    );
}
