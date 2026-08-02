//! One analysis over a set of trials.
//!
//! Nothing here computes a quantity. The loop reads each trial under one declared format,
//! hands it to `plateforce_analysis::run`, and places the outcome in flat relations that
//! carry the methods that produced every number. A trial that cannot be read or that a rule
//! declines is named in `refusals` and stays in the denominator, so a run over fifty files
//! never reports forty-seven answers and says nothing about the other three.

pub mod decisions;
pub mod engine;
pub mod fingerprint;
pub mod identity;
pub mod relations;
pub mod synthetic;

pub use decisions::{unresolved, UnresolvedDecision};
pub use engine::{analyse, BatchRequest, BatchResult, Coverage, RunRefusal};
pub use identity::{
    Session, SourceFormat, SubjectKey, TrialEntry, TrialIdentity, TrialSet, TrialSource, WalkError,
};
pub use relations::{
    AggregateRow, ProvenanceRow, RefusalRow, ResultRow, RunRow, WarningRow,
};
