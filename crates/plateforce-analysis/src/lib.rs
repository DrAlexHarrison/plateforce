//! Binding registry entries to the core, and running one analysis.
//!
//! No quantity is computed here. Every number comes back from `plateforce_core`; this
//! crate decides which core function a registry id names, what parameters it was given,
//! and what provenance travels with the answer.
//!
//! Every surface links this one. A browser, a command line, a Python package and an R
//! package that each decided for themselves which rules to expose and what to pass when
//! the user says nothing would disagree with each other.
//!
//! Where an id below is absent from the registry it is still offered, flagged. Hiding an
//! executable rule because its paperwork is unfinished would be a silent exclusion.
//!
//! Two kinds of absence, and they are not the same claim. A composition is a registry
//! method bound with an operator, so it inherits that row's citations and needs no row of
//! its own. An unfiled rule is one the registry files elsewhere or does not carry.
//!
//! The layout: `binding` says which ids run, `request` and `response` are what a caller
//! sends and receives, `resolution` records what each rule read, `slots` holds one
//! directory per construct with one module per bound rule, and `pipeline` runs them.

pub mod binding;
pub mod boundaries;
pub mod capability;
pub(crate) mod centre_of_mass;
pub mod chain;
pub mod conditioning;
pub mod derived;
pub mod document;
pub mod markdown;
pub mod method_set;
pub mod pipeline;
pub mod quality;
pub mod request;
pub mod resolution;
pub mod response;
pub mod slots;
pub mod spread;

pub use binding::{
    bindings_for, records_under, Binding, BINDINGS, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT,
    WEIGHING_CONSTRUCT,
};
pub use chain::{
    accounts_of, chain_names, chain_of, chains_of, descriptions_of, metrics_resting_on, MetricChain,
};
pub use pipeline::run;
pub use request::{
    gravity_stated, AnalysisRequest, DeclaredDefaults, EntryDefaults, MethodChoice, WeighingChoice,
    BODY_MASS_GLOBAL, GRAVITY_GLOBAL, TOUCHDOWN_GLOBAL,
};
pub use resolution::{BoundMethod, DeclinedRule, RuleRefusal};

/// How a number reads in a record: a value somebody stated, and a value a rule produced.
///
/// One spelling for both, because the two meet in one column. A batch writes a rule's
/// threshold beside the value it declined on, and a swept setting beside the number it moved,
/// so a record spelling the stated one and the produced one differently spells one value two
/// ways. Five reads as `5`, the way somebody states it, rather than as `5.0`, and anything
/// else reads at the digits that read back as the same binary64, never at display precision:
/// a sweep over 9.80665 and 9.8070 rendered at two decimals labels two runs identically and
/// leaves a reader unable to tell which produced which number.
pub fn recorded_number_text(value: f64) -> String {
    resolution::format_number(value)
}
pub use response::{AnalysisResponse, BoundGlobal, Levels, Metric};
pub use slots::movement_onset::ONSET_OPERATOR_IDS;
pub use slots::system_weight::weighing_epoch_at;
pub use slots::takeoff::TAKEOFF_OPERATOR_IDS;
