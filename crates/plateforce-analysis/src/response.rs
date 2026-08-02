//! What one analysis hands back: the landmarks, the levels drawn on the trace, the numbers,
//! and the record of what produced each of them.

use serde::Serialize;

use crate::resolution::{BoundMethod, RuleRefusal};

/// The symbol an interface draws beside a number, for the units this build reports.
///
/// The registry spells every unit out and that spelling is what a fingerprint carries, so
/// the short form is derived from it here and never stored as a second vocabulary.
pub(crate) fn unit_symbol(unit: &'static str) -> &'static str {
    match unit {
        "newtons" => "N",
        "kilograms" => "kg",
        "seconds" => "s",
        "meters" => "m",
        "meters_per_second" => "m/s",
        "newton_seconds" => "N.s",
        other => other,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub key: &'static str,
    pub label: &'static str,
    pub value: Option<f64>,
    /// As the registry spells it. `docs/schema.md` carries the same spelling on every
    /// construct and every parameter.
    pub unit: &'static str,
    pub unit_symbol: &'static str,
    /// The landmark rules whose answers this number rests on.
    pub contributing_method_ids: Vec<String>,
    /// The registry entry for the arithmetic that turned those landmarks into this number,
    /// which is a different question from which landmarks fed it.
    ///
    /// Both jump-height figures and modified reactive strength are registry entries with
    /// citations, published parameters and, in one case, a `force_a_decision` surfacing
    /// verdict. Reporting only the landmark chain leaves the reader unable to tell which of
    /// two numerators produced a reactive-strength number, or which of the registry's 22
    /// jump-height methods was run. `None` means no entry describes this arithmetic, which
    /// is itself worth seeing.
    #[serde(default)]
    pub computed_by: Option<&'static str>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Levels {
    pub system_weight_newtons: f64,
    pub weighing_standard_deviation_newtons: f64,
    pub onset_band_lower_newtons: Option<f64>,
    pub onset_band_upper_newtons: Option<f64>,
    pub takeoff_threshold_newtons: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisResponse {
    pub weighing_start_index: usize,
    pub weighing_end_index: usize,
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub levels: Levels,
    pub bound_methods: Vec<BoundMethod>,
    pub metrics: Vec<Metric>,
    /// Windows the weighing rule could not choose between. One for a fixed window, and
    /// anything above one means the selection is an artefact of the arithmetic. Skipped
    /// over the wire, where the interface draws it inside the warning that reports it.
    #[serde(skip)]
    pub weighing_epoch_tied_window_count: usize,
    pub warnings: Vec<String>,
    /// The same failures `warnings` describes, kept as the errors they were, keyed by the
    /// slot whose rule produced nothing. Skipped over the wire, where the sentence is what
    /// the interface draws.
    #[serde(skip)]
    pub refusals: Vec<(&'static str, RuleRefusal)>,
}
