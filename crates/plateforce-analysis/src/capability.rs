//! What this software can do, reported by whoever was asked.
//!
//! Every surface serialises this and its answer is recorded under its own name in one
//! committed file. Two of the arrays are therefore not generated here: a surface passes the
//! operations it actually dispatches and the container formats it can actually write, because
//! those are facts about that surface and they differ between surfaces. The rest
//! come from tables every surface links, so a difference in one of them is a stale build
//! rather than a capability.
//!
//! Nothing here serialises. Each surface reaches for its own writer, and this crate stays
//! free of both a JSON dependency and any knowledge of which surface asked.

use serde::Serialize;

use plateforce_core::{exit_code, RefusalCode};

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

/// One way this software can decline, and what a shell learns from it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RefusalRecord {
    pub code: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Capability {
    pub schema: &'static str,
    pub plateforce_version: &'static str,
    pub methods: Vec<MethodRecord>,
    pub operations: Vec<Operation>,
    pub output_formats: Vec<OutputFormat>,
    pub refusal_codes: Vec<RefusalRecord>,
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

    // Generated from the enum rather than a list beside it, so a code added to the vocabulary
    // arrives here without an edit.
    let mut refusal_codes: Vec<RefusalRecord> = RefusalCode::ALL
        .iter()
        .map(|code| RefusalRecord {
            code: spelling(*code),
            exit_code: exit_code(*code),
        })
        .collect();
    refusal_codes.sort();

    Capability {
        schema: SCHEMA,
        plateforce_version: env!("CARGO_PKG_VERSION"),
        methods,
        operations,
        output_formats,
        refusal_codes,
    }
}

/// The registry and every binding spell a code in snake_case, and this crate carries no JSON
/// writer, so the spelling is derived from the variant name rather than transcribed.
fn spelling(code: RefusalCode) -> String {
    let named = format!("{code:?}");
    let mut out = String::with_capacity(named.len() + 4);
    for (index, character) in named.char_indices() {
        if character.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        } else {
            out.push(character);
        }
    }
    out
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
