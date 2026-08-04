//! The registry, compiled into the module.
//!
//! `plateforce_registry::Registry::load` reads a directory, and a browser tab has no
//! directory to read. The bytes come from `build.rs` instead and go through
//! `plateforce_registry::assemble`, the call the desktop loader makes, so a set of files
//! the desktop cannot assemble is one this cannot assemble either.
//!
//! Validation is where the two surfaces part. The desktop hands back no registry and
//! prints the violations. Here the registry and its violations both reach the interface,
//! which names every one and refuses to attribute a number until they are fixed.

use plateforce_registry::{assemble, AssemblyError, Registry, Violation};

include!(concat!(env!("OUT_DIR"), "/embedded_registry.rs"));

pub struct LoadedRegistry {
    pub registry: Registry,
    pub digest: String,
    pub file_count: usize,
    pub violations: Vec<Violation>,
}

impl LoadedRegistry {
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn violation_messages(&self) -> Vec<String> {
        self.violations.iter().map(|v| v.to_string()).collect()
    }
}

/// Assembles the registry this build carries, with the digest that identifies it.
pub fn load() -> Result<LoadedRegistry, AssemblyError> {
    load_files(EMBEDDED_REGISTRY_FILES)
}

fn load_files(files: &[(&str, &str)]) -> Result<LoadedRegistry, AssemblyError> {
    let assembled = assemble(files.iter().copied())?;
    let mut registry = assembled.registry;
    registry.declared_version = EMBEDDED_REGISTRY_VERSION.map(str::to_string);
    Ok(LoadedRegistry {
        digest: registry.content_digest.clone(),
        registry,
        file_count: files.len(),
        violations: assembled.violations,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use plateforce_registry::{content_digest, read_sources, Registry, RegistryError, Source};

    /// The browser reports the revision the registry directory names, so a method-set
    /// document written in a tab cites the same name the terminal and the wheel write.
    #[test]
    fn the_browser_names_the_revision_the_registry_declares() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
        let on_disk = Registry::declared_version_at(&root);
        assert_eq!(super::load().unwrap().registry.declared_version, on_disk);
        assert!(
            on_disk.is_some(),
            "the shipped registry names a revision, so there is one to report"
        );
    }

    /// One registry, in the two forms the two surfaces receive it: strings for the browser,
    /// and the same strings written to a directory for the desktop.
    const DUPLICATED_ID: &[(&str, &str)] = &[
        (
            "constructs.toml",
            "[[construct]]\nid = \"movement_onset\"\ntitle = \"Onset\"\nunit = \"seconds\"\n",
        ),
        (
            "methods/first.toml",
            "[[method]]\nid = \"onset.threshold.twice\"\nconstruct = \"movement_onset\"\n\
             title = \"A rule\"\nrule = \"Something.\"\nstatus = \"accepted\"\n\
             confidence = \"high\"\n",
        ),
        (
            "methods/second.toml",
            "[[method]]\nid = \"onset.threshold.twice\"\nconstruct = \"movement_onset\"\n\
             title = \"The same id again\"\nrule = \"Something else.\"\nstatus = \"accepted\"\n\
             confidence = \"high\"\n",
        ),
    ];

    struct ScratchDirectory {
        path: PathBuf,
    }

    impl ScratchDirectory {
        fn holding(files: &[(&str, &str)]) -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static TAKEN: AtomicU32 = AtomicU32::new(0);
            let unique = TAKEN.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "plateforce-cross-path-{}-{unique}",
                std::process::id()
            ));
            std::fs::remove_dir_all(&path).ok();
            for (relative, contents) in files {
                let file = path.join(relative);
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(file, contents).unwrap();
            }
            Self { path }
        }
    }

    impl Drop for ScratchDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).ok();
        }
    }

    #[test]
    fn every_embedded_file_parses_against_the_schema_types() {
        let loaded = super::load().expect("a registry file did not parse");
        assert!(loaded.file_count > 0, "build.rs embedded no registry files");
        assert!(!loaded.registry.constructs.is_empty());
        assert!(loaded.digest.starts_with("content-"));
    }

    /// Not an assertion that the registry is valid, which is the registry's own business.
    /// This asserts that whatever the validator says survives the trip to the interface,
    /// because a violation the browser cannot see is a violation nobody fixes.
    #[test]
    fn validator_output_reaches_the_caller() {
        let loaded = super::load().unwrap();
        assert_eq!(loaded.is_valid(), loaded.violation_messages().is_empty());
    }

    /// The same registry through both doors. A duplicate id is a hard refusal on the
    /// desktop, and this is what keeps it a hard refusal in the browser.
    #[test]
    fn both_surfaces_refuse_the_same_duplicated_id() {
        let scratch = ScratchDirectory::holding(DUPLICATED_ID);

        let desktop = Registry::load(&scratch.path).unwrap_err();
        let Err(browser) = super::load_files(DUPLICATED_ID) else {
            panic!("the browser loader accepted a duplicated id");
        };

        let RegistryError::Invalid(from_desktop) = desktop else {
            panic!("the desktop loader refused for another reason: {desktop}");
        };
        let plateforce_registry::AssemblyError::Duplicated(from_browser) = browser else {
            panic!("the browser loader refused for another reason: {browser}");
        };
        assert_eq!(from_desktop, from_browser);
        assert_eq!(from_desktop.len(), 1);
        assert_eq!(from_desktop[0].entry, "onset.threshold.twice");
    }

    /// The digest the interface prints is the digest of the directory the desktop reads,
    /// so a number attributed to `content-xxxx` names one registry on every surface.
    #[test]
    fn the_embedded_registry_is_the_directory_the_desktop_reads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
        let on_disk = read_sources(&root).unwrap();

        let embedded_paths: Vec<&str> = super::EMBEDDED_REGISTRY_FILES
            .iter()
            .map(|(path, _)| *path)
            .collect();
        let disk_paths: Vec<&str> = on_disk.iter().map(|source| source.path.as_str()).collect();
        assert_eq!(disk_paths, embedded_paths);

        let from_disk = content_digest(on_disk.iter().map(Source::pair));
        assert_eq!(super::load().unwrap().digest, from_disk);
    }
}
