//! A digest written into prose says which registry a result was produced under.
//!
//! A digest copied into documentation and compared against nothing goes on naming a registry
//! that no longer exists.

use std::path::{Path, PathBuf};
use std::process::Command;

use plateforce_registry::Registry;

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
    let mut disagreeing = Vec::new();
    for file in committed_files(&root) {
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
    assert!(
        disagreeing.is_empty(),
        "the registry answers {answered} and these say otherwise: {disagreeing:#?}"
    );
}
