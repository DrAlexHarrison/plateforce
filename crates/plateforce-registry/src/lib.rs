//! The registry is data. Adding a method is a file edit, never a code change.
//!
//! Loading is strict: a registry that violates any rule in `validate` does not load at
//! all. The failure mode this whole project exists to document is a number whose
//! provenance nobody checked, so an unvalidated registry is worse than no registry.

pub mod schema;
pub mod validate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
}

fn format_violations(violations: &[Violation]) -> String {
    violations
        .iter()
        .map(|v| format!("  {v}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A loaded registry, indexed by canonical dotted id.
///
/// The two populations are held separately and are never summed into one total.
/// Both of this project's headline counts turned out to be assertions rather than
/// queries, so every count here is a query with its denominator attached.
#[derive(Debug, Default)]
pub struct Registry {
    pub constructs: BTreeMap<String, Construct>,
    pub methods: BTreeMap<String, Method>,
    pub protocols: BTreeMap<String, Protocol>,
}

/// Counts, each carrying the population it counts over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    pub constructs: usize,
    pub computation_entries: usize,
    pub protocol_entries: usize,
}

impl Registry {
    /// Load every TOML file under `root`, then validate. Returns the violations rather
    /// than a partial registry when validation fails.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let root = root.as_ref();
        // An absent registry has no violations, so without this it loads empty and passes.
        if !root.is_dir() {
            return Err(RegistryError::Absent {
                path: root.to_path_buf(),
                reason: "there is no directory there".to_string(),
            });
        }
        let mut registry = Registry::default();

        for path in toml_files_under(&root.join("constructs.toml"))? {
            let file: ConstructFile = read_toml(&path)?;
            for construct in file.constructs {
                registry.constructs.insert(construct.id.clone(), construct);
            }
        }
        for path in toml_files_under(&root.join("methods"))? {
            let file: MethodFile = read_toml(&path)?;
            for method in file.methods {
                registry.methods.insert(method.id.clone(), method);
            }
        }
        for path in toml_files_under(&root.join("protocols"))? {
            let file: ProtocolFile = read_toml(&path)?;
            for protocol in file.protocols {
                registry.protocols.insert(protocol.id.clone(), protocol);
            }
        }

        if registry.constructs.is_empty() && registry.methods.is_empty() {
            return Err(RegistryError::Absent {
                path: root.to_path_buf(),
                reason: "the directory holds no constructs.toml and no methods".to_string(),
            });
        }

        let violations = validate::validate(&registry);
        if violations.is_empty() {
            Ok(registry)
        } else {
            Err(RegistryError::Invalid(violations))
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

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, RegistryError> {
    let text = std::fs::read_to_string(path).map_err(|source| RegistryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| RegistryError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Accepts either a single file or a directory, so the layout can grow a directory
/// where it currently has one file without a code change.
fn toml_files_under(path: &Path) -> Result<Vec<PathBuf>, RegistryError> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(path).map_err(|source| RegistryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_with_no_directory_is_not_an_empty_registry() {
        let error = Registry::load("/plateforce-no-such-directory").unwrap_err();
        assert!(matches!(error, RegistryError::Absent { .. }), "{error}");
    }

    #[test]
    fn a_directory_holding_no_entries_is_not_a_registry() {
        let empty = std::env::temp_dir().join("plateforce-empty-registry-test");
        std::fs::create_dir_all(&empty).unwrap();
        let error = Registry::load(&empty).unwrap_err();
        std::fs::remove_dir_all(&empty).ok();
        assert!(matches!(error, RegistryError::Absent { .. }), "{error}");
    }

    #[test]
    fn a_status_prints_the_way_the_registry_spells_it() {
        assert_eq!(Status::Accepted.to_string(), "accepted");
        assert_eq!(Confidence::High.to_string(), "high");
        assert_eq!(Detectability::Silent.to_string(), "silent");
    }
}
