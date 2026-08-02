//! The interface inside the binary is the interface on disk, path for path and byte for
//! byte, minus the names the build script excluded on purpose.
//!
//! Compared against the directory rather than against a count. A test asserting five files
//! passes when a sixth is added and dropped; a test asserting everything except the names
//! excluded on purpose fails, which is the behaviour this crate exists to have.

use std::path::{Path, PathBuf};

fn web() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web")
}

fn file_names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_embedded_set_is_the_directory_minus_what_was_excluded() {
    let web = web();
    let excluded = plateforce_serve::not_part_of_the_interface();

    let walked = file_names_in(&web);
    let expected: Vec<String> = walked
        .iter()
        .filter(|name| !excluded.contains(&name.as_str()))
        .cloned()
        .collect();

    // Subtracting the binary's own exclusion list from both sides would compare the
    // directory against itself and pass under any list at all. So each excluded name is
    // held against what the page loads: every interface file is named by another one, and
    // repository documentation is named by none.
    let what_the_page_loads: String = plateforce_serve::assets()
        .iter()
        .filter(|asset| asset.content_type.starts_with("text/"))
        .map(|asset| String::from_utf8_lossy(asset.bytes).into_owned())
        .collect::<Vec<String>>()
        .join("\n");
    for name in excluded {
        assert!(
            !what_the_page_loads.contains(name),
            "{name} is excluded from the binary and the interface loads it by name"
        );
    }
    let embedded: Vec<String> = plateforce_serve::assets()
        .iter()
        .filter(|asset| !asset.path.contains('/'))
        .map(|asset| asset.path.to_string())
        .collect();
    assert_eq!(
        expected, embedded,
        "the interface on disk and the interface in the binary are different sets"
    );

    let bundle = web.join("pkg");
    let embedded_bundle: Vec<String> = plateforce_serve::assets()
        .iter()
        .filter_map(|asset| asset.path.strip_prefix("pkg/"))
        .map(str::to_string)
        .collect();
    let bundle_state = if bundle.is_dir() {
        assert_eq!(
            file_names_in(&bundle),
            embedded_bundle,
            "the browser bundle on disk and the one in the binary are different sets"
        );
        assert!(plateforce_serve::carries_the_browser_bundle());
        format!("plus {} build artefacts", embedded_bundle.len())
    } else {
        assert!(
            embedded_bundle.is_empty(),
            "the binary carries a browser bundle that is not on disk"
        );
        assert!(!plateforce_serve::carries_the_browser_bundle());
        "web/pkg absent".to_string()
    };

    for asset in plateforce_serve::assets() {
        let on_disk = std::fs::read(web.join(asset.path)).unwrap();
        assert_eq!(
            asset.bytes,
            on_disk.as_slice(),
            "{} in the binary is not the file on disk",
            asset.path
        );
    }

    // The log says what was left out, so a reader can tell a deliberate exclusion from a
    // file the build script never saw.
    println!(
        "walked {}, embedded {}, excluded {}; {bundle_state}",
        walked.len(),
        expected.len(),
        excluded.join(", ")
    );
}
