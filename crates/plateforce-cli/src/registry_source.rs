//! Which registry a run reads, and where those bytes came from.
//!
//! Naming no directory reads the registry this build carries, rather than a relative
//! `registry` path, which would read a different set of methods depending on the directory
//! the operator was standing in.
//!
//! Naming a directory reads that directory, and a directory that does not load is a refusal
//! rather than a fall back to the compiled copy: a caller who silently receives other bytes
//! has been told a result came from a registry it did not come from.

use std::path::Path;

use plateforce_registry::{assemble, AssemblyError, Registry, RegistryError};

include!(concat!(env!("OUT_DIR"), "/embedded_registry.rs"));

/// What `registry validate` reports as the origin of the entries it read.
pub const CARRIED_BY_THIS_BUILD: &str = "the registry compiled into plateforce";

/// The registry for this run: the directory named, or the one this build carries.
pub fn load(named: Option<&Path>) -> Result<Registry, RegistryError> {
    match named {
        Some(directory) => Registry::load(directory),
        None => carried(),
    }
}

/// Where the entries came from, as an origin a machine reads: a path, or the phrase that
/// stands for this binary's own copy.
pub fn describe(named: Option<&Path>) -> String {
    match named {
        Some(directory) => directory.display().to_string(),
        None => CARRIED_BY_THIS_BUILD.to_string(),
    }
}

/// The same fact as a noun phrase, so a sentence reads the same either way.
pub fn in_prose(named: Option<&Path>) -> String {
    match named {
        Some(directory) => format!("the registry at {}", directory.display()),
        None => CARRIED_BY_THIS_BUILD.to_string(),
    }
}

/// The registry compiled in, assembled through the call the directory loader makes, and
/// strict in the same places: a set of files that loader refuses is refused here too.
fn carried() -> Result<Registry, RegistryError> {
    let assembled =
        assemble(EMBEDDED_REGISTRY_FILES.iter().copied()).map_err(|error| match error {
            AssemblyError::Parse { path, source } => RegistryError::Parse {
                path: path.into(),
                source,
            },
            AssemblyError::Unplaced { path } => RegistryError::Unplaced { path: path.into() },
            AssemblyError::NoMethods => RegistryError::Absent {
                path: CARRIED_BY_THIS_BUILD.into(),
                reason: "it holds no methods".to_string(),
            },
            AssemblyError::Duplicated(violations) => RegistryError::Invalid(violations),
        })?;
    if !assembled.violations.is_empty() {
        return Err(RegistryError::Invalid(assembled.violations));
    }
    let mut registry = assembled.registry;
    // The walk filters on the toml extension, so the revision the registry names itself is
    // not among the files and build.rs carries it separately.
    registry.declared_version = EMBEDDED_REGISTRY_VERSION.map(str::to_string);
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use plateforce_registry::{content_digest, read_sources, Registry, Source};

    fn repository_registry() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry")
    }

    /// The digest this binary reports names the bytes this binary carries, so a fingerprint
    /// quoted in a methods section is checkable by downloading the release it names.
    #[test]
    fn the_registry_in_the_binary_is_the_registry_in_the_repository() {
        let on_disk = read_sources(repository_registry()).unwrap();

        let embedded: Vec<&str> = super::EMBEDDED_REGISTRY_FILES
            .iter()
            .map(|(path, _)| *path)
            .collect();
        let from_disk: Vec<&str> = on_disk.iter().map(|source| source.path.as_str()).collect();
        assert_eq!(from_disk, embedded);

        assert_eq!(
            super::load(None).unwrap().content_digest,
            content_digest(on_disk.iter().map(Source::pair))
        );
    }

    /// Naming no directory and naming the repository's directory reach the same registry,
    /// which is what says the compiled copy is the shipped one rather than a stale build.
    #[test]
    fn naming_nothing_and_naming_the_directory_agree() {
        let named = super::load(Some(&repository_registry())).unwrap();
        let carried = super::load(None).unwrap();
        assert_eq!(named.content_digest, carried.content_digest);
        assert_eq!(named.declared_version, carried.declared_version);
        assert_eq!(named.methods.len(), carried.methods.len());
    }

    /// A named directory that does not load is a refusal. Falling back to the compiled
    /// copy would hand back numbers attributed to a registry the caller did not name.
    #[test]
    fn a_named_directory_that_is_absent_refuses_rather_than_falling_back() {
        let absent = Path::new("plateforce-no-such-registry-directory");
        let error = super::load(Some(absent)).unwrap_err();
        assert!(
            matches!(error, plateforce_registry::RegistryError::Absent { .. }),
            "an absent named directory reported: {error}"
        );
        assert!(format!("{error}").contains("plateforce-no-such-registry-directory"));
    }

    /// The revision the registry names itself travels with the entries, so a document
    /// written in a terminal cites the name the browser and the wheel write.
    #[test]
    fn the_binary_names_the_revision_the_registry_declares() {
        let on_disk = Registry::declared_version_at(repository_registry());
        assert_eq!(super::load(None).unwrap().declared_version, on_disk);
        assert!(on_disk.is_some());
    }

    /// Where the entries came from, said plainly enough that a reader can find them again.
    #[test]
    fn the_origin_is_reported_either_way() {
        assert_eq!(super::describe(None), super::CARRIED_BY_THIS_BUILD);
        assert_eq!(super::describe(Some(Path::new("elsewhere"))), "elsewhere");
    }
}
