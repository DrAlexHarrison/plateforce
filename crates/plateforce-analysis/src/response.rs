//! What one analysis hands back: the landmarks, the levels drawn on the trace, the numbers,
//! and the record of what produced each of them.

use serde::Serialize;

use crate::resolution::{BoundMethod, RuleRefusal};

/// The symbol an interface draws beside a number, for the units this build reports.
///
/// The registry spells every unit out and that spelling is what a fingerprint carries, so
/// the short form is derived from it here and never stored as a second vocabulary.
pub fn unit_symbol(unit: &'static str) -> &'static str {
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

/// One quantity this build can report, declared once.
///
/// The eleven keys used to be string literals at eleven construction sites, so a manifest
/// listing them transcribed them and went stale the first time a twelfth arrived. A rule
/// that produces a new quantity adds a row here and the manifest, the Python getters and
/// the browser all see it without an edit of their own.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Quantity {
    pub key: &'static str,
    pub label: &'static str,
    /// As the registry spells it. `docs/schema.md` carries the same spelling on every
    /// construct and every parameter.
    pub unit: &'static str,
    /// The registry entry for the arithmetic that turns landmarks into this number. `None`
    /// means no entry describes it, which is itself worth seeing.
    pub computed_by: Option<&'static str>,
}

pub const QUANTITIES: &[Quantity] = &[
    Quantity {
        key: "system_weight_newtons",
        label: "System weight",
        unit: "newtons",
        computed_by: None,
    },
    Quantity {
        key: "system_mass_kilograms",
        label: "System mass",
        unit: "kilograms",
        computed_by: None,
    },
    Quantity {
        key: "onset_time_seconds",
        label: "Movement onset",
        unit: "seconds",
        computed_by: None,
    },
    Quantity {
        key: "takeoff_time_seconds",
        label: "Takeoff",
        unit: "seconds",
        computed_by: None,
    },
    Quantity {
        key: "time_to_takeoff_seconds",
        label: "Time to takeoff",
        unit: "seconds",
        computed_by: Some("time_to_takeoff.onset_to_takeoff"),
    },
    Quantity {
        key: "flight_time_seconds",
        label: "Flight time",
        unit: "seconds",
        computed_by: Some("flight_time.takeoff_to_touchdown"),
    },
    Quantity {
        key: "takeoff_velocity_meters_per_second",
        label: "Takeoff velocity",
        unit: "meters_per_second",
        computed_by: Some("impulse.net_vertical.as_performance_determinant"),
    },
    Quantity {
        key: "net_impulse_newton_seconds",
        label: "Net impulse",
        unit: "newton_seconds",
        computed_by: Some("impulse.net_vertical.as_performance_determinant"),
    },
    Quantity {
        key: "jump_height_from_takeoff_meters",
        label: "Jump height, takeoff frame",
        unit: "meters",
        computed_by: Some("jumpheight.takeoff.impulse_momentum"),
    },
    Quantity {
        key: "jump_height_from_flight_time_meters",
        label: "Jump height, flight time",
        unit: "meters",
        computed_by: Some("jumpheight.takeoff.flight_time"),
    },
    Quantity {
        key: "reactive_strength_index_modified",
        label: "RSI modified",
        unit: "meters_per_second",
        computed_by: Some("rsimod.jh_tov_over_ttt"),
    },
];

/// The declaration for a key, or nothing when no quantity carries it.
pub fn quantity(key: &str) -> Option<&'static Quantity> {
    QUANTITIES.iter().find(|quantity| quantity.key == key)
}

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub key: String,
    pub label: String,
    pub value: Option<f64>,
    /// As the registry spells it. `docs/schema.md` carries the same spelling on every
    /// construct and every parameter.
    pub unit: String,
    pub unit_symbol: String,
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
    pub computed_by: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl Metric {
    /// A reported number, taking its key, label, unit and computed-by from the one
    /// declaration rather than from a literal at the call site.
    pub fn declared(
        key: &str,
        value: Option<f64>,
        contributing_method_ids: Vec<String>,
        note: Option<String>,
    ) -> Self {
        let declared = quantity(key).unwrap_or_else(|| panic!("{key} is not a declared quantity"));
        Self {
            key: declared.key.to_string(),
            label: declared.label.to_string(),
            value,
            unit: declared.unit.to_string(),
            unit_symbol: unit_symbol(declared.unit).to_string(),
            contributing_method_ids,
            computed_by: declared.computed_by.map(str::to_string),
            note,
        }
    }
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
    /// Quality signals computed over this result: a comparison, a threshold, and an action.
    ///
    /// On the response rather than behind an entry point of their own, so a signal cannot be
    /// fetched for one request and drawn beside the numbers of another. Every surface reads
    /// this response, so the browser, the terminal, Python and R receive them together.
    #[serde(default)]
    pub signals: Vec<crate::quality::QualitySignal>,
    /// The same failures `warnings` describes, kept as the errors they were, keyed by the
    /// slot whose rule produced nothing. Skipped over the wire, where the sentence is what
    /// the interface draws.
    #[serde(skip)]
    pub refusals: Vec<(&'static str, RuleRefusal)>,
}
