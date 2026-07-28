//! One assembly of a registry from its source files, whatever door they arrived through.
//!
//! A directory on disk and a set of strings compiled into a browser build are the same
//! registry seen from two places. Both come through `assemble`, so no surface can accept
//! a registry another surface refuses, and no surface can count something the files do
//! not declare.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::schema::{ConstructFile, MethodFile, ProtocolFile};
use crate::{format_violations, validate, Registry, RegistryError, Violation, ViolationKind};

/// One registry file: its path relative to the registry root, and its text.
pub struct Source {
    pub path: String,
    pub contents: String,
}

impl Source {
    pub fn pair(&self) -> (&str, &str) {
        (&self.path, &self.contents)
    }
}

/// What assembly produced: the registry, and everything the validator said about it.
/// Surfaces differ over what to do with violations, never over what counts as one.
pub struct Assembled {
    pub registry: Registry,
    pub violations: Vec<Violation>,
}

/// The ways a set of files fails to describe one registry. Distinct from a violation,
/// which is a fact about entries that did assemble.
#[derive(Debug, thiserror::Error)]
pub enum AssemblyError {
    #[error("parsing {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("no population owns {path}: entries live in constructs.toml, methods/ or protocols/")]
    Unplaced { path: String },
    #[error("the registry holds no methods")]
    NoMethods,
    #[error("{} duplicate id(s):\n{}", .0.len(), format_violations(.0))]
    Duplicated(Vec<Violation>),
}

/// Parse every file, index it by canonical dotted id, then validate.
///
/// A second definition of an id would replace the first, leaving a map that counts fewer
/// entries than the files declare, so it is refused rather than counted.
///
/// The digest is taken over the same bytes rather than left to the caller, so a registry
/// cannot reach a result under the identity of a set of files it was not assembled from.
pub fn assemble<'a>(
    files: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<Assembled, AssemblyError> {
    let files: Vec<(&str, &str)> = files.into_iter().collect();
    let mut registry = Registry {
        content_digest: content_digest(files.iter().copied()),
        ..Registry::default()
    };
    let mut duplicated: Vec<String> = Vec::new();

    for (path, contents) in files {
        let parse_failed = |source| AssemblyError::Parse {
            path: path.to_string(),
            source,
        };
        if path == "constructs.toml" || path.starts_with("constructs.toml/") {
            let file: ConstructFile = toml::from_str(contents).map_err(parse_failed)?;
            for construct in file.constructs {
                let id = construct.id.clone();
                if registry.constructs.insert(id.clone(), construct).is_some() {
                    duplicated.push(id);
                }
            }
        } else if path.starts_with("methods/") {
            let file: MethodFile = toml::from_str(contents).map_err(parse_failed)?;
            for method in file.methods {
                let id = method.id.clone();
                if registry.methods.insert(id.clone(), method).is_some() {
                    duplicated.push(id);
                }
            }
        } else if path.starts_with("protocols/") {
            let file: ProtocolFile = toml::from_str(contents).map_err(parse_failed)?;
            for protocol in file.protocols {
                let id = protocol.id.clone();
                if registry.protocols.insert(id.clone(), protocol).is_some() {
                    duplicated.push(id);
                }
            }
        } else {
            return Err(AssemblyError::Unplaced {
                path: path.to_string(),
            });
        }
    }

    // Methods rather than either population. A set holding constructs alone reports zero
    // entries and no violations, which reads as a registry that passed.
    if registry.methods.is_empty() {
        return Err(AssemblyError::NoMethods);
    }
    if !duplicated.is_empty() {
        return Err(AssemblyError::Duplicated(
            duplicated
                .into_iter()
                .map(|entry| Violation {
                    entry,
                    kind: ViolationKind::DuplicateId,
                })
                .collect(),
        ));
    }

    let violations = validate::validate(&registry);
    Ok(Assembled {
        registry,
        violations,
    })
}

/// FNV-1a over path and contents in path order. Identifies which registry a number came
/// from without claiming to be a declared version, which the registry does not carry.
pub fn content_digest<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut ordered: Vec<(&str, &str)> = files.into_iter().collect();
    ordered.sort_by(|left, right| left.0.cmp(right.0));

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (path, contents) in ordered {
        for byte in path.as_bytes().iter().chain(contents.as_bytes()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("content-{hash:016x}")
}

/// Every TOML file under `root`, read, sorted, and named by its path relative to `root`
/// with `/` separators on every platform so a directory and a browser build present the
/// same names to `assemble`.
pub fn read_sources(root: impl AsRef<Path>) -> Result<Vec<Source>, RegistryError> {
    let root = root.as_ref();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut found: Vec<Source> = Vec::new();
    read_directory(root, root, &mut visited, &mut found)?;
    found.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(found)
}

fn read_directory(
    root: &Path,
    directory: &Path,
    visited: &mut HashSet<PathBuf>,
    found: &mut Vec<Source>,
) -> Result<(), RegistryError> {
    // Links are followed, so a registry can point at a shared tree. Resolving each
    // directory to one identity is what stops a link back up the tree from recursing.
    let identity = std::fs::canonicalize(directory).map_err(|source| RegistryError::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    if !visited.insert(identity) {
        return Err(RegistryError::Cycle {
            path: directory.to_path_buf(),
        });
    }

    let entries = std::fs::read_dir(directory).map_err(|source| RegistryError::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    for entry in entries {
        // An entry that cannot be read is an error rather than one fewer method.
        let path = entry
            .map_err(|source| RegistryError::Io {
                path: directory.to_path_buf(),
                source,
            })?
            .path();
        if path.is_dir() {
            read_directory(root, &path, visited, found)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            let contents = std::fs::read_to_string(&path).map_err(|source| RegistryError::Io {
                path: path.clone(),
                source,
            })?;
            found.push(Source {
                path: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                contents,
            });
        }
    }
    Ok(())
}
