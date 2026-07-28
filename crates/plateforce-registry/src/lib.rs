//! The registry is data. Adding a method is a file edit, never a code change.
//!
//! Loading is strict: a registry that violates any rule in `validate` does not load at
//! all. The failure mode this whole project exists to document is a number whose
//! provenance nobody checked, so an unvalidated registry is worse than no registry.

pub mod assembly;
pub mod schema;
pub mod validate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use assembly::{assemble, content_digest, read_sources, Assembled, AssemblyError, Source};
pub use schema::*;
pub use validate::{Violation, ViolationKind};

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("registry failed validation with {} violation(s):\n{}", .0.len(), format_violations(.0))]
    Invalid(Vec<Violation>),
    #[error("no registry at {path}: {reason}")]
    Absent { path: PathBuf, reason: String },
    #[error("no population owns {path}: entries live in constructs.toml, methods/ or protocols/")]
    Unplaced { path: PathBuf },
    #[error("a link under the registry root leads back to {path}")]
    Cycle { path: PathBuf },
}

pub(crate) fn format_violations(violations: &[Violation]) -> String {
    violations
        .iter()
        .map(|v| format!("  {v}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A loaded registry, indexed by canonical dotted id, carrying the digest of the files
/// it was assembled from.
///
/// The two populations are held separately and are never summed into one total.
/// Both of this project's headline counts turned out to be assertions rather than
/// queries, so every count here is a query with its denominator attached.
#[derive(Debug, Default)]
pub struct Registry {
    pub constructs: BTreeMap<String, Construct>,
    pub methods: BTreeMap<String, Method>,
    pub protocols: BTreeMap<String, Protocol>,
    /// Which registry this is, measured from the bytes that were assembled rather than
    /// declared alongside them, so it never disagrees with what was loaded.
    pub content_digest: String,
}

/// Counts, each carrying the population it counts over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    pub constructs: usize,
    pub computation_entries: usize,
    pub protocol_entries: usize,
}

impl Registry {
    /// Read every TOML file under `root` and assemble it. Returns the violations rather
    /// than a partial registry when validation fails.
    ///
    /// The filesystem and the errors it raises are this function's business; what makes a
    /// set of files a registry belongs to `assemble`, which every other surface calls.
    ///
    /// The registry names itself by the bytes read here, so a caller never re-reads the
    /// tree to say what it holds and never names a tree that changed in between.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let root = root.as_ref();
        // An absent registry has no violations, so without this it loads empty and passes.
        // Reading the metadata rather than asking `is_dir`, which answers false for a
        // directory it could not stat and would report a locked registry as missing.
        match std::fs::metadata(root) {
            Ok(found) if found.is_dir() => {}
            Ok(_) => {
                return Err(RegistryError::Absent {
                    path: root.to_path_buf(),
                    reason: "that path is a file, and a registry is a directory".to_string(),
                })
            }
            Err(source) => {
                return Err(RegistryError::Absent {
                    path: root.to_path_buf(),
                    reason: source.to_string(),
                })
            }
        }

        let sources = read_sources(root)?;
        let assembled =
            assemble(sources.iter().map(Source::pair)).map_err(|error| match error {
                AssemblyError::Parse { path, source } => RegistryError::Parse {
                    path: root.join(path),
                    source,
                },
                AssemblyError::Unplaced { path } => RegistryError::Unplaced {
                    path: root.join(path),
                },
                AssemblyError::NoMethods => RegistryError::Absent {
                    path: root.to_path_buf(),
                    reason: "the directory holds no methods".to_string(),
                },
                AssemblyError::Duplicated(violations) => RegistryError::Invalid(violations),
            })?;

        if assembled.violations.is_empty() {
            Ok(assembled.registry)
        } else {
            Err(RegistryError::Invalid(assembled.violations))
        }
    }

    pub fn census(&self) -> Census {
        Census {
            constructs: self.constructs.len(),
            computation_entries: self.methods.len(),
            protocol_entries: self.protocols.len(),
        }
    }

    /// Every method whose choice materially moves the number and on which the field
    /// is genuinely split. These are the rows the interface must not decide silently.
    pub fn genuine_debates(&self) -> impl Iterator<Item = &Method> {
        self.methods
            .values()
            .filter(|m| m.debate == Some(Debate::Genuine))
    }

    /// Methods that can find the wrong event rather than merely finding it late.
    /// Reporting a bias for one of these without its failure rate averages working
    /// with not working.
    pub fn methods_that_can_fail(&self) -> impl Iterator<Item = &Method> {
        self.methods.values().filter(|m| m.failure.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own. A fixed path under the system temporary directory
    /// gets deleted out from under one run by another whenever two builds overlap.
    struct ScratchDirectory {
        path: PathBuf,
    }

    impl ScratchDirectory {
        fn new(name: &str) -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static TAKEN: AtomicU32 = AtomicU32::new(0);
            let unique = TAKEN.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "plateforce-{name}-{}-{unique}",
                std::process::id()
            ));
            std::fs::remove_dir_all(&path).ok();
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    /// Cleanup on the failing path too, where a line at the end of the test never runs.
    impl Drop for ScratchDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).ok();
        }
    }

    #[test]
    fn a_path_with_no_directory_is_not_an_empty_registry() {
        let error = Registry::load("/plateforce-no-such-directory").unwrap_err();
        assert!(matches!(error, RegistryError::Absent { .. }), "{error}");
    }

    #[test]
    fn a_directory_holding_no_entries_is_not_a_registry() {
        let empty = ScratchDirectory::new("empty-registry");
        let error = Registry::load(&empty.path).unwrap_err();
        assert!(matches!(error, RegistryError::Absent { .. }), "{error}");
    }

    fn write_minimal_registry(root: &Path, method_file: &Path, id: &str) {
        std::fs::create_dir_all(method_file.parent().unwrap()).unwrap();
        std::fs::write(
            root.join("constructs.toml"),
            "[[construct]]\nid = \"movement_onset\"\ntitle = \"Onset\"\nunit = \"seconds\"\n",
        )
        .unwrap();
        std::fs::write(
            method_file,
            format!(
                "[[method]]\nid = \"{id}\"\nconstruct = \"movement_onset\"\n\
                 title = \"A rule\"\nrule = \"Something.\"\nstatus = \"accepted\"\n\
                 confidence = \"high\"\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn a_method_file_in_a_subdirectory_is_loaded_the_way_the_browser_embeds_it() {
        let root = ScratchDirectory::new("nested-registry");
        write_minimal_registry(
            &root.path,
            &root.path.join("methods").join("extras").join("nested.toml"),
            "onset.threshold.nested",
        );
        let registry = Registry::load(&root.path).unwrap();
        assert!(registry.methods.contains_key("onset.threshold.nested"));
    }

    #[test]
    fn two_definitions_of_one_id_are_a_violation_rather_than_a_census_of_one() {
        let root = ScratchDirectory::new("duplicate-registry");
        let methods = root.path.join("methods");
        write_minimal_registry(&root.path, &methods.join("aaa.toml"), "onset.threshold.twice");
        write_minimal_registry(&root.path, &methods.join("zzz.toml"), "onset.threshold.twice");
        let error = Registry::load(&root.path).unwrap_err();
        assert!(error.to_string().contains("more than one entry"), "{error}");
    }

    #[test]
    fn a_misspelt_key_is_refused_rather_than_dropped() {
        let root = ScratchDirectory::new("typo-registry");
        let method_file = root.path.join("methods").join("typo.toml");
        write_minimal_registry(&root.path, &method_file, "onset.threshold.typo");
        let mut text = std::fs::read_to_string(&method_file).unwrap();
        text.push_str("\n[[method.citaton]]\nkey = \"nobody\"\nrole = \"proposes\"\n");
        std::fs::write(&method_file, text).unwrap();
        let error = Registry::load(&root.path).unwrap_err();
        assert!(matches!(error, RegistryError::Parse { .. }), "{error}");
    }

    #[test]
    fn a_directory_holding_constructs_and_no_methods_is_not_a_registry() {
        let partial = ScratchDirectory::new("constructs-only");
        std::fs::write(
            partial.path.join("constructs.toml"),
            "[[construct]]\nid = \"system_weight\"\ntitle = \"Weight\"\nunit = \"newtons\"\n",
        )
        .unwrap();
        let error = Registry::load(&partial.path).unwrap_err();
        assert!(matches!(error, RegistryError::Absent { .. }), "{error}");
    }

    #[test]
    fn a_file_belonging_to_no_population_is_refused_rather_than_walked_past() {
        let root = ScratchDirectory::new("unplaced-file");
        write_minimal_registry(
            &root.path,
            &root.path.join("methods").join("real.toml"),
            "onset.threshold.real",
        );
        std::fs::write(
            root.path.join("draft.toml"),
            "[[method]]\nid = \"onset.threshold.draft\"\n",
        )
        .unwrap();
        let error = Registry::load(&root.path).unwrap_err();
        assert!(matches!(error, RegistryError::Unplaced { .. }), "{error}");
    }

    /// The whole claim behind the digest: it moves when the registry's content moves and
    /// stays put when it does not. A declared version can promise neither.
    #[test]
    fn the_digest_follows_the_content_and_nothing_else() {
        let root = ScratchDirectory::new("digest-follows-content");
        let method_file = root.path.join("methods").join("rule.toml");
        write_minimal_registry(&root.path, &method_file, "onset.threshold.measured");

        let first = Registry::load(&root.path).unwrap().content_digest;
        assert_eq!(first, Registry::load(&root.path).unwrap().content_digest);

        let text = std::fs::read_to_string(&method_file).unwrap();
        std::fs::write(&method_file, text.replace("Something.", "Something else.")).unwrap();
        let after_the_edit = Registry::load(&root.path).unwrap().content_digest;
        assert_ne!(
            first, after_the_edit,
            "an edited rule left the registry naming itself the same"
        );
    }

    /// `load` walks the files, so it is the one place that can name what was read. A
    /// caller re-reading the tree to ask gets an answer about whatever is there now.
    #[test]
    fn a_loaded_registry_names_the_files_the_loader_read() {
        let root = ScratchDirectory::new("digest-names-sources");
        write_minimal_registry(
            &root.path,
            &root.path.join("methods").join("rule.toml"),
            "onset.threshold.read",
        );
        let registry = Registry::load(&root.path).unwrap();
        let sources = read_sources(&root.path).unwrap();
        assert_eq!(
            registry.content_digest,
            content_digest(sources.iter().map(Source::pair))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_pointing_back_up_the_tree_is_refused_rather_than_walked_forever() {
        let root = ScratchDirectory::new("looping-registry");
        let methods = root.path.join("methods");
        write_minimal_registry(&root.path, &methods.join("real.toml"), "onset.threshold.real");
        std::os::unix::fs::symlink(&root.path, methods.join("upwards")).unwrap();
        let error = Registry::load(&root.path).unwrap_err();
        assert!(matches!(error, RegistryError::Cycle { .. }), "{error}");
    }

    /// Every variant against what serde writes, rather than against a literal, so a
    /// mistyped spelling fails here instead of sending a user to search the files for a
    /// string that is not in them.
    #[test]
    fn every_variant_prints_the_way_the_registry_spells_it() {
        fn serde_spelling<T: serde::Serialize>(value: &T) -> String {
            #[derive(serde::Serialize)]
            struct Wrapper<'a, T> {
                field: &'a T,
            }
            toml::to_string(&Wrapper { field: value })
                .unwrap()
                .trim()
                .trim_start_matches("field = ")
                .trim_matches('"')
                .to_string()
        }

        for status in [
            Status::Recommended,
            Status::Accepted,
            Status::Contested,
            Status::Legacy,
            Status::Deprecated,
        ] {
            assert_eq!(status.as_registry_str(), serde_spelling(&status));
        }
        for confidence in [Confidence::High, Confidence::Medium, Confidence::Low] {
            assert_eq!(confidence.as_registry_str(), serde_spelling(&confidence));
        }
        for detectability in [
            Detectability::Silent,
            Detectability::Loud,
            Detectability::Guarded,
        ] {
            assert_eq!(detectability.as_registry_str(), serde_spelling(&detectability));
        }
    }
}
