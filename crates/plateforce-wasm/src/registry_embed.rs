//! The registry, compiled into the module.
//!
//! `plateforce_registry::Registry::load` reads a directory, and a browser tab has no
//! directory to read. The bytes come from `build.rs` instead and go through the same
//! schema types and the same validator, so the browser cannot load a registry the
//! desktop would reject.
//!
//! A file that does not parse is fatal, because there is nothing to show. A registry
//! that parses and fails validation is reported rather than swallowed: the interface
//! names every violation and refuses to attribute a number to the registry until they
//! are fixed.

use plateforce_registry::{
    validate, ConstructFile, MethodFile, ProtocolFile, Registry, Violation,
};

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

#[derive(Debug)]
pub struct ParseFailure {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ParseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parsing {}: {}", self.path, self.message)
    }
}

/// Parses every embedded file and runs the validator, returning both the registry and
/// whatever the validator found.
pub fn load() -> Result<LoadedRegistry, ParseFailure> {
    let mut registry = Registry::default();

    for (path, contents) in EMBEDDED_REGISTRY_FILES {
        let fail = |message: String| ParseFailure {
            path: (*path).to_string(),
            message,
        };
        if *path == "constructs.toml" {
            let file: ConstructFile = toml::from_str(contents).map_err(|e| fail(e.to_string()))?;
            for construct in file.constructs {
                registry.constructs.insert(construct.id.clone(), construct);
            }
        } else if path.starts_with("methods/") {
            let file: MethodFile = toml::from_str(contents).map_err(|e| fail(e.to_string()))?;
            for method in file.methods {
                registry.methods.insert(method.id.clone(), method);
            }
        } else if path.starts_with("protocols/") {
            let file: ProtocolFile = toml::from_str(contents).map_err(|e| fail(e.to_string()))?;
            for protocol in file.protocols {
                registry.protocols.insert(protocol.id.clone(), protocol);
            }
        }
    }

    let violations = validate::validate(&registry);
    Ok(LoadedRegistry {
        registry,
        digest: EMBEDDED_REGISTRY_DIGEST.to_string(),
        file_count: EMBEDDED_REGISTRY_FILES.len(),
        violations,
    })
}

#[cfg(test)]
mod tests {
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
}
