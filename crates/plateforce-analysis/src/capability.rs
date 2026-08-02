//! What this software can do, reported by whoever was asked.
//!
//! Every surface serialises this and the results are compared against one committed file, so
//! a surface that cannot do what the file lists fails the build. Two of the arrays are
//! therefore not generated here: a surface passes the operations it actually dispatches and
//! the container formats it can actually write, because those are facts about that surface.
//! The rest come from tables every surface links, so they agree by construction and the
//! comparison is about the surfaces rather than about the build.
//!
//! Nothing here serialises. Each surface reaches for its own writer, and this crate stays
//! free of both a JSON dependency and any knowledge of which surface asked.

use serde::Serialize;

use crate::binding::BINDINGS;

/// The manifest's shape, so a reader of a committed file knows which shape it is.
pub const SCHEMA: &str = "plateforce.capability/1";

/// What a surface can be asked to do.
///
/// Closed, so a surface cannot invent a name or misspell one. Declared in the order the
/// spellings sort, which is the order every surface emits them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Analyse,
    Batch,
    Capability,
    /// Batch's second entry point, which loops the sweep and returns paired variants in
    /// place of results. A manifest carrying `batch` alone would claim one operation where
    /// the software has two, and a surface reaching only one of them would pass.
    Compare,
    ParseForceFile,
    Reach,
    RegistryCensus,
    RegistryShow,
    RegistryValidate,
    Spread,
    Version,
}

/// What a surface can write a result into.
///
/// A container format is a capability of a surface and not an interaction, so a surface that
/// ships one writer where another ships two is a difference this file holds rather than a
/// difference nobody can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Csv,
    Json,
    Parquet,
    Text,
}

/// One rule every surface can run, and where it sits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MethodRecord {
    pub id: &'static str,
    pub slot: &'static str,
    pub construct: &'static str,
    /// The registry row this id binds an operator on, where it binds one.
    pub composed_from: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Capability {
    pub schema: &'static str,
    pub plateforce_version: &'static str,
    pub methods: Vec<MethodRecord>,
    pub operations: Vec<Operation>,
    pub output_formats: Vec<OutputFormat>,
}

/// Sorted throughout, so two surfaces that can do the same things emit the same bytes and a
/// comparison is a plain diff.
pub fn capability(operations: &[Operation], output_formats: &[OutputFormat]) -> Capability {
    let mut methods: Vec<MethodRecord> = BINDINGS
        .iter()
        .map(|binding| MethodRecord {
            id: binding.id,
            slot: binding.slot,
            construct: binding.construct,
            composed_from: binding.composed_from,
        })
        .collect();
    methods.sort();

    let mut operations = operations.to_vec();
    operations.sort();
    operations.dedup();

    let mut output_formats = output_formats.to_vec();
    output_formats.sort();
    output_formats.dedup();

    Capability {
        schema: SCHEMA,
        plateforce_version: env!("CARGO_PKG_VERSION"),
        methods,
        operations,
        output_formats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_carries_one_record_per_binding() {
        let manifest = capability(&[Operation::Capability], &[OutputFormat::Json]);
        println!(
            "methods in the manifest: {} of {} bindings",
            manifest.methods.len(),
            BINDINGS.len()
        );
        assert_eq!(manifest.methods.len(), BINDINGS.len());
        let ids: Vec<&str> = manifest.methods.iter().map(|record| record.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "a surface emits the records in one order");
    }

    /// A surface reporting the same operation twice must not produce a longer array than a
    /// surface reporting it once, or the comparison fails on a fact about nobody's code.
    #[test]
    fn a_repeated_operation_is_reported_once() {
        let manifest = capability(
            &[Operation::Analyse, Operation::Analyse, Operation::Batch],
            &[OutputFormat::Json, OutputFormat::Json],
        );
        assert_eq!(manifest.operations.len(), 2);
        assert_eq!(manifest.output_formats.len(), 1);
    }
}
