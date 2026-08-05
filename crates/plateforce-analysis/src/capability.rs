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

use std::collections::BTreeMap;

use serde::Serialize;

use plateforce_core::{exit_code, Acquisition, RefusalCode};
use plateforce_registry::Registry;

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
    /// Batch's third entry point, which reduces an athlete's trials to one number under a
    /// named registry rule.
    ///
    /// Separate from `Batch` by the same argument that separates `Compare`: it is reached by
    /// its own request, it binds a registry entry publishing three incompatible rules, and it
    /// refuses rather than taking a mean. A surface that loops the analysis and cannot reduce
    /// does a different thing from one that can, and without a value here the manifest reports
    /// the two as identical. One surface reduces today, so this is the value that says so.
    Aggregate,
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

/// One value a caller may state, read from the registry entry that publishes it.
///
/// A projection rather than a second home. Every field here is copied off the `Registry` the
/// caller passed in, so the registry remains the one place a rule's text lives and this is the
/// same bytes reshaped for a reader who asked one question instead of a hundred and eight.
///
/// `states` is the token a caller writes, not the bare name, and that is the whole point of
/// carrying this. An agent holding `name: "k"` still has to work out that the flag wants
/// `onset.k`, and the one thing a manifest for a program must never do is leave the caller to
/// derive the string it is about to be refused for mis-spelling.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParameterRecord {
    /// `<slot>.<name>`, the token `--set` takes and every surface's request map is keyed by.
    pub states: String,
    /// The name the registry entry gives it, which is what a reader looks up.
    pub name: String,
    pub unit: Option<String>,
    /// Every number the literature states for it, which is how a caller learns that a rule
    /// published at six values is a choice rather than a setting.
    pub published_values: Vec<f64>,
    /// The keys of a parameter whose options are names rather than numbers, sorted.
    pub named_values: Vec<String>,
    pub default: Option<f64>,
    pub default_key: Option<String>,
    /// Whether the rule can produce a result without a value for it. Required with no default
    /// is the shape that refuses a request by name, and it is the one an agent has to read
    /// before it builds a call rather than after.
    pub required: bool,
}

/// One rule every surface can run, and where it sits.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MethodRecord {
    pub id: &'static str,
    pub slot: &'static str,
    pub construct: &'static str,
    /// The registry row this id binds an operator on, where it binds one.
    pub composed_from: Option<&'static str>,
    /// The names this rule declines without, sorted, and empty for a rule that runs on a
    /// request stating nothing.
    ///
    /// A different fact from `parameters` below, and the two answer different questions. This
    /// is the half only the build knows: a value the registry publishes no default for is a
    /// value the rule will not run without, and no reader of the registry alone can tell that
    /// from a value it simply does not mention. A chooser reading it can build a request that
    /// is not refused.
    pub requires: Vec<&'static str>,
    /// Every value a caller may state on this rule, read from its registry entry.
    ///
    /// Empty for a rule whose entry publishes none, which is a statement that there is nothing
    /// to state rather than an absence of information. A rule this build runs whose id names no
    /// registry entry would report empty too, so `every_rule_this_build_runs_resolves_in_the_registry`
    /// holds that case at zero rather than leaving the two indistinguishable on the wire.
    pub parameters: Vec<ParameterRecord>,
}

/// One operator entry a rule composes, and the names a caller states to reach it.
///
/// On the wire because nothing else can carry it. The registry declares the entry and what it
/// publishes; only the build knows which of its names a rule actually reads and which entry
/// each reaches, so a chooser reading the registry alone cannot learn that stating `selection`
/// on a takeoff rule reaches `takeoff.op.crossing_selection` rather than the onset one.
///
/// Keyed by the construct rather than by the rule. Which operators a run ends up recording
/// depends on what the caller states, so a per-rule list would be a claim about one request
/// and this is a claim about what may be stated.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OperatorRecord {
    pub construct: &'static str,
    pub entry: &'static str,
    /// Sorted, and more than one where two names reach one entry.
    pub states: Vec<&'static str>,
    /// What the names above accept, read from the operator's own registry entry.
    ///
    /// The names alone say a caller may state `selection` on a takeoff rule; these say it takes
    /// `first` or `longest_run` and that it is required with no default. Which of those two runs
    /// moves takeoff 843 ms on 155 of 244 trials, so a caller told the name and not the values
    /// has been handed the more dangerous half of the fact.
    pub parameters: Vec<ParameterRecord>,
}

/// One way this software can decline, and what a shell learns from it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RefusalRecord {
    pub code: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Capability {
    pub schema: &'static str,
    pub plateforce_version: &'static str,
    pub acquisition: AcquisitionBlock,
    pub methods: Vec<MethodRecord>,
    /// The operator entries the rules above compose, which a caller never names and always
    /// reaches through one of them.
    pub operators: Vec<OperatorRecord>,
    pub operations: Vec<Operation>,
    pub output_formats: Vec<OutputFormat>,
    pub refusal_codes: Vec<RefusalRecord>,
}

/// Every value a registry entry publishes, as the tokens a caller of `slot` writes.
///
/// Empty for an id the registry does not carry. That case is a defect rather than a shape,
/// and it is held at zero by a test rather than reported here, because a manifest is read by
/// programs and a row saying "this rule may be unknown to the registry" is not something a
/// caller can act on.
fn parameters_of(registry: &Registry, id: &str, slot: &str) -> Vec<ParameterRecord> {
    let Some(entry) = registry.methods.get(id) else {
        return Vec::new();
    };
    let mut published: Vec<ParameterRecord> = entry
        .parameters
        .iter()
        .map(|parameter| {
            let mut named_values: Vec<String> = parameter
                .named_values
                .iter()
                .map(|value| value.key.clone())
                .collect();
            named_values.sort();
            ParameterRecord {
                states: format!("{slot}.{}", parameter.name),
                name: parameter.name.clone(),
                unit: parameter.unit.clone(),
                published_values: parameter.published_values.clone(),
                named_values,
                default: parameter.default,
                default_key: parameter.default_key.clone(),
                required: parameter.required,
            }
        })
        .collect();
    published.sort_by(|left, right| left.name.cmp(&right.name));
    published
}

/// The slot a caller writes to reach a construct, read off the bindings rather than written
/// down twice.
///
/// An operator is keyed by construct, and the token `--set` takes is keyed by slot, so the two
/// have to be joined somewhere. Here rather than in each surface, so four surfaces cannot
/// disagree about the spelling of one flag.
fn slot_for_construct(construct: &str) -> Option<&'static str> {
    BINDINGS
        .iter()
        .find(|binding| binding.construct == construct)
        .map(|binding| binding.slot)
}

/// Sorted throughout, so two surfaces that can do the same things emit the same bytes and a
/// comparison is a plain diff.
///
/// The registry is passed in rather than reached for, because it is the one home for what a
/// rule accepts and a surface that read its own copy would be a second one. Every surface has
/// one already: the terminal loads the directory it was pointed at or the copy it carries, the
/// browser loads the copy compiled into the bundle, and the two language bindings take a root
/// from their caller.
pub fn capability(
    operations: &[Operation],
    output_formats: &[OutputFormat],
    acquisition_intake: AcquisitionIntake,
    registry: &Registry,
) -> Capability {
    let mut methods: Vec<MethodRecord> = BINDINGS
        .iter()
        .map(|binding| {
            // Both lists, because a rule can decline for want of a name and for want of a
            // number, and the anthropometric jump-height rules need two or three at once. A
            // chooser handed one of them meets a refusal naming the next.
            let mut requires: Vec<&'static str> = crate::binding::required_options(binding.id)
                .iter()
                .map(|(name, _)| *name)
                .chain(
                    crate::binding::required_numbers(binding.id)
                        .iter()
                        .map(|(name, _)| *name),
                )
                .collect();
            requires.sort();
            requires.dedup();
            MethodRecord {
                id: binding.id,
                slot: binding.slot,
                construct: binding.construct,
                composed_from: binding.composed_from,
                requires,
                // The entry a redirected rule records under, because a redirect exists
                // precisely where the registry carries no row of the rule's own name, and
                // reading its own id would report that it takes nothing. Two rules are
                // redirected today and both are compositions whose parameters live on the
                // rule they compose.
                parameters: parameters_of(
                    registry,
                    binding.records_under.unwrap_or(binding.id),
                    binding.slot,
                ),
            }
        })
        .collect();
    // By id, which is what the derived order was before a published value made the record
    // hold an `f64` and took `Ord` off it. The test below holds the order rather than the
    // derive, so the two cannot drift.
    methods.sort_by(|left, right| left.id.cmp(right.id));

    // Every construct a rule in this build fills, crossed with the operator entries its rules
    // compose, so a construct that grows a rule composing one more entry publishes it without
    // an edit here.
    let mut by_entry: BTreeMap<(&'static str, &'static str), Vec<&'static str>> = BTreeMap::new();
    for construct in crate::binding::executable_constructs() {
        for routed in crate::binding::operator_names_for_construct(construct) {
            by_entry
                .entry((construct, routed.entry))
                .or_default()
                .push(routed.name);
        }
    }
    let operators: Vec<OperatorRecord> = by_entry
        .into_iter()
        .map(|((construct, entry), mut states)| {
            states.sort();
            let slot = slot_for_construct(construct).unwrap_or(construct);
            OperatorRecord {
                construct,
                entry,
                states,
                parameters: parameters_of(registry, entry, slot),
            }
        })
        .collect();

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
        operators,
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

    /// The registry this repository carries, read from disk rather than assembled here.
    ///
    /// The manifest's parameter half is a projection of these bytes, so a test against an
    /// invented registry would prove the projection runs and nothing about what it says.
    fn the_registry() -> Registry {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
        Registry::load(&root).expect("the registry this repository carries")
    }

    fn manifest_over_the_registry(registry: &Registry) -> Capability {
        capability(
            &[Operation::Capability],
            &[OutputFormat::Json],
            AcquisitionIntake::StatedByCaller,
            registry,
        )
    }

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
        let published = manifest_over_the_registry(&the_registry())
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
        let registry = the_registry();
        let taking = capability(&[], &[], AcquisitionIntake::StatedByCaller, &registry).acquisition;
        let without = capability(
            &[],
            &[],
            AcquisitionIntake::AbsentFromThisSurface,
            &registry,
        )
        .acquisition;
        assert!(taking.stated_by_caller);
        assert!(!without.stated_by_caller);
        assert_eq!(taking.members, without.members);
    }

    #[test]
    fn the_manifest_carries_one_record_per_binding() {
        let manifest = manifest_over_the_registry(&the_registry());
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
            &the_registry(),
        );
        assert_eq!(manifest.operations.len(), 2);
        assert_eq!(manifest.output_formats.len(), 1);
    }

    /// A rule this build runs whose entry cannot be found reports an empty parameter list,
    /// which on the wire is indistinguishable from a rule that takes nothing.
    ///
    /// So the case is held at zero here rather than described in the field's documentation, and
    /// it found two the first time it ran: `onset.threshold.last_within_band` and
    /// `takeoff.threshold.longest_run` are redirected, meaning the registry deliberately carries
    /// no row of their own name, so reading their own id reported that they take nothing while
    /// each is a composition whose parameters sit on the rule it composes.
    ///
    /// The control is the second assertion: rules that really do carry parameters are counted, so
    /// a registry that failed to load and returned nothing for everything cannot pass this by
    /// agreeing with itself.
    #[test]
    fn every_rule_this_build_runs_resolves_in_the_registry() {
        let registry = the_registry();
        let unresolved: Vec<&str> = BINDINGS
            .iter()
            .filter(|binding| {
                !registry
                    .methods
                    .contains_key(binding.records_under.unwrap_or(binding.id))
            })
            .map(|binding| binding.id)
            .collect();
        assert!(
            unresolved.is_empty(),
            "{} of {} rules this build runs reach no registry entry, so each reports an empty \
             parameter list that reads as a rule taking nothing: {unresolved:?}",
            unresolved.len(),
            BINDINGS.len(),
        );

        // The redirected rules specifically, because they are the ones the first version of
        // this test read as unresolved and they are the ones a later edit would break again.
        let redirected: Vec<&str> = BINDINGS
            .iter()
            .filter(|binding| binding.records_under.is_some())
            .map(|binding| binding.id)
            .collect();
        assert!(
            !redirected.is_empty(),
            "no rule is redirected, so the branch that follows a redirect is asserting nothing",
        );

        let manifest = manifest_over_the_registry(&registry);
        let carrying = manifest
            .methods
            .iter()
            .filter(|record| !record.parameters.is_empty())
            .count();
        assert!(
            carrying > 0,
            "no rule in the manifest carries a parameter, so this test would pass against a \
             registry that loaded nothing at all",
        );
        let redirected_carrying = manifest
            .methods
            .iter()
            .filter(|record| redirected.contains(&record.id) && !record.parameters.is_empty())
            .count();
        assert_eq!(
            redirected_carrying,
            redirected.len(),
            "{} of {} redirected rules report no parameter, so the redirect is not being \
             followed and those rows say a rule takes nothing when it takes what it composes",
            redirected.len() - redirected_carrying,
            redirected.len(),
        );

        println!(
            "rules reaching a registry entry: {} of {}, of which {} publish a value a caller \
             may state; {} of those reach it through a redirect",
            BINDINGS.len(),
            BINDINGS.len(),
            carrying,
            redirected.len(),
        );
    }

    /// The token a caller writes is on the wire, rather than a name they have to qualify.
    ///
    /// Taken over the rule the acceptance fixture states, because a manifest that spelled the
    /// token differently from the flag would send an agent to a refusal while reading as a
    /// complete answer.
    #[test]
    fn a_parameter_carries_the_token_a_caller_writes_rather_than_its_bare_name() {
        let manifest = manifest_over_the_registry(&the_registry());
        let record = manifest
            .methods
            .iter()
            .find(|record| record.id == "onset.threshold.noise_relative")
            .expect("a rule the acceptance fixture states a value on");
        let k = record
            .parameters
            .iter()
            .find(|parameter| parameter.name == "k")
            .expect("the rule publishes the value that fixture states");
        assert_eq!(
            k.states, "onset.k",
            "the manifest spells the token differently from the flag that takes it",
        );
        assert!(
            k.published_values.len() > 1,
            "the registry publishes this rule at several values and the manifest reports {:?}, \
             so a caller reading it cannot see that stating one is a choice",
            k.published_values,
        );
    }
}
