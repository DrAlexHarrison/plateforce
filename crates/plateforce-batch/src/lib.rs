//! One analysis over a set of trials.
//!
//! Nothing here computes a quantity. The loop reads each trial under one declared format,
//! hands it to `plateforce_analysis::run`, and places the outcome in flat relations that
//! carry the methods that produced every number. A trial that cannot be read or that a rule
//! declines is named in `refusals` and stays in the denominator, so a run over fifty files
//! never reports forty-seven answers and says nothing about the other three.

pub mod aggregate;
pub mod agreement;
pub mod decisions;
pub mod derive;
pub mod engine;
pub mod exclusions;
pub mod fingerprint;
pub mod identity;
pub mod relations;
pub mod render;
pub mod synthetic;
pub mod write_csv;
pub mod write_json;
#[cfg(feature = "parquet")]
pub mod write_parquet;

pub use aggregate::{
    aggregate, with_aggregates, AggregationRefusal, AggregationRequest, AggregationRule, GroupKind,
};
pub use agreement::{
    bind_statistic, bound_statistic_ids, compare, AgreementRefusal, BatchCompareRequest,
    BatchCompareResult, LimitsRequest, PairedRow, ReliabilityInterval, UnitOfAnalysis,
};
pub use decisions::{unresolved, UnresolvedDecision};
pub use derive::DeriveRefusal;
pub use engine::{analyse, BatchRequest, BatchResult, Coverage, RunRefusal};
pub use exclusions::{GateFinding, GateRegistry, GateTally, PopulationExclusion, ValidityGate};
pub use identity::{
    Session, SourceFormat, SubjectKey, TrialEntry, TrialIdentity, TrialSet, TrialSource, WalkError,
};
pub use relations::{
    AggregateRow, ProvenanceRow, RefusalRow, ResultRow, RunRow, SignalRow, WarningRow,
};
pub use render::{Rendered, Rendering};
pub use write_csv::{read_csv, Relation, WriteRefusal, EVERY_RELATION};
pub use write_json::envelope;
