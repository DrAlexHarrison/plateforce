//! Binding registry entries to the core, and running one analysis.
//!
//! No quantity is computed here. Every number comes back from `plateforce_core`; this
//! crate decides which core function a registry id names, what parameters it was given,
//! and what provenance travels with the answer.
//!
//! Every surface links this one. A browser, a command line, a Python package and an R
//! package that each decided for themselves which rules to expose and what to pass when
//! the user says nothing would disagree with each other, which is the finding this
//! project exists to publish.
//!
//! Where an id below is absent from the registry it is still offered, flagged. Hiding an
//! executable rule because its paperwork is unfinished is the silent exclusion this
//! project exists to document.
//!
//! Two kinds of absence, and they are not the same claim. A composition is a registry
//! method bound with an operator, so it inherits that row's citations and needs no row of
//! its own. An unfiled rule is one the registry files elsewhere or does not carry.
//!
//! The layout: `binding` says which ids run, `request` and `response` are what a caller
//! sends and receives, `resolution` records what each rule read, `slots` holds one
//! directory per construct with one module per bound rule, and `pipeline` runs them.

pub mod binding;
pub mod capability;
pub(crate) mod centre_of_mass;
pub mod derived;
pub mod document;
pub mod method_set;
pub mod pipeline;
pub mod quality;
pub mod request;
pub mod resolution;
pub mod response;
pub mod slots;
pub mod spread;

pub use binding::{
    bindings_for, Binding, BINDINGS, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
pub use pipeline::run;
pub use request::{AnalysisRequest, MethodChoice, WeighingChoice};
pub use resolution::{BoundMethod, DeclinedRule, RuleRefusal};
pub use response::{AnalysisResponse, Levels, Metric};
pub use slots::movement_onset::ONSET_OPERATOR_IDS;
pub use slots::system_weight::weighing_epoch_at;
