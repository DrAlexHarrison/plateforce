//! What this software can do, reported by whoever was asked.
//!
//! Every surface serialises this and its answer is recorded under its own name in one
//! committed file. Three of the fields are therefore not generated here: a surface passes the
//! operations it actually dispatches, the container formats it can actually write, and whether
//! its caller can state the acquisition block, because those are facts about that surface and
//! they differ between surfaces by design. The rest come from tables every surface links, so a
//! difference in one of them is a stale build rather than a capability.
//!
//! The acquisition block's members are the half of that field a surface does not pass. A
//! surface free to name them could publish four of five, and a reader told to go and find four
//! fills four and fingerprints as matching.
//!
//! Nothing here serialises. Each surface reaches for its own writer, and this crate stays
//! free of both a JSON dependency and any knowledge of which surface asked.

use serde::Serialize;

use plateforce_core::{exit_code, Acquisition, RefusalCode};

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

/// Whether a caller of this surface can state the acquisition block on a trial.
///
/// Named rather than a bare boolean, because the call site is what a reader of a surface
/// checks against that surface's intake, and `false` beside two arrays says nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionIntake {
    /// The surface takes a block from its caller and carries it into the fingerprint.
    StatedByCaller,
    /// The surface analyses without one, so every result it produces carries
    /// `acquisition_complete = false` and fingerprints as incomplete rather than as matching.
    AbsentFromThisSurface,
}

/// What a surface can be told about the plate and its settings.
///
/// `members` is read from the block itself rather than passed in by the surface, so a surface
/// cannot answer with a shorter list than the block holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcquisitionBlock {
    pub stated_by_caller: bool,
    /// In declaration order, which is the order the fingerprint is taken over.
    pub members: Vec<&'static str>,
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
    pub acquisition: AcquisitionBlock,
    pub methods: Vec<MethodRecord>,
    pub operations: Vec<Operation>,
    pub output_formats: Vec<OutputFormat>,
    pub refusal_codes: Vec<RefusalRecord>,
}

/// Sorted throughout, so two surfaces that can do the same things emit the same bytes and a
/// comparison is a plain diff.
pub fn capability(
    operations: &[Operation],
    output_formats: &[OutputFormat],
    acquisition_intake: AcquisitionIntake,
) -> Capability {
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

    // Generated from the enum rather than a list beside it. The vocabulary has gone from
    // nine values to fourteen while this file was being written, and every one arrived here
    // without an edit.
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
        // Read from the block rather than from a list beside it, so a member the block gains
        // is a member every surface publishes without an edit anywhere.
        acquisition: AcquisitionBlock {
            stated_by_caller: acquisition_intake == AcquisitionIntake::StatedByCaller,
            members: Acquisition::MEMBERS.to_vec(),
        },
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

    /// Every member the acquisition block declares, written out here rather than read from
    /// `Acquisition::MEMBERS`.
    ///
    /// Both sides of a comparison against the same const agree while five members become four,
    /// and a manifest naming four teaches a reader to go and find four. A member the block
    /// gains passes this list and is caught by the second direction below.
    const MEMBERS_A_READER_IS_TOLD_TO_FIND: [&str; 5] = [
        "filter_at_capture",
        "tare_state",
        "plate_natural_frequency_hz",
        "floor_surface",
        "firmware_version",
    ];

    #[test]
    fn the_manifest_names_every_member_of_the_acquisition_block() {
        let published = capability(
            &[Operation::Capability],
            &[OutputFormat::Json],
            AcquisitionIntake::StatedByCaller,
        )
        .acquisition
        .members;

        let unpublished: Vec<&&str> = MEMBERS_A_READER_IS_TOLD_TO_FIND
            .iter()
            .filter(|member| !published.contains(member))
            .collect();
        assert!(
            unpublished.is_empty(),
            "{} of {} members are named nowhere in the manifest, so a reader filling what it lists fingerprints as matching on an incomplete block: {unpublished:?}",
            unpublished.len(),
            MEMBERS_A_READER_IS_TOLD_TO_FIND.len(),
        );

        // The other direction, against the block rather than against the list above, so a
        // member the block gains and the manifest drops is a failure rather than a name
        // nobody looked for.
        let undeclared: Vec<&&str> = Acquisition::MEMBERS
            .iter()
            .filter(|member| !published.contains(member))
            .collect();
        assert!(
            undeclared.is_empty(),
            "{} of {} members the block holds are missing from the manifest: {undeclared:?}",
            undeclared.len(),
            Acquisition::MEMBERS.len(),
        );

        let invented: Vec<&&str> = published
            .iter()
            .filter(|member| !Acquisition::MEMBERS.contains(member))
            .collect();
        assert!(
            invented.is_empty(),
            "{} of {} members the manifest publishes are not members of the block, so a caller filling them states something no fingerprint reads: {invented:?}",
            invented.len(),
            published.len(),
        );

        println!(
            "acquisition members published: {} of {} the block declares",
            published.len(),
            Acquisition::MEMBERS.len()
        );
    }

    /// The field a surface passes moves, and the field it does not pass does not.
    #[test]
    fn a_surface_answers_for_its_own_intake_and_never_for_the_members() {
        let taking = capability(&[], &[], AcquisitionIntake::StatedByCaller).acquisition;
        let without = capability(&[], &[], AcquisitionIntake::AbsentFromThisSurface).acquisition;
        assert!(taking.stated_by_caller);
        assert!(!without.stated_by_caller);
        assert_eq!(taking.members, without.members);
    }

    #[test]
    fn the_manifest_carries_one_record_per_binding() {
        let manifest = capability(
            &[Operation::Capability],
            &[OutputFormat::Json],
            AcquisitionIntake::StatedByCaller,
        );
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
            AcquisitionIntake::StatedByCaller,
        );
        assert_eq!(manifest.operations.len(), 2);
        assert_eq!(manifest.output_formats.len(), 1);
    }
}
