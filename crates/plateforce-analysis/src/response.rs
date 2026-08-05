//! What one analysis hands back: the landmarks, the levels drawn on the trace, the numbers,
//! and the record of what produced each of them.

use std::sync::LazyLock;

use plateforce_core::provenance::ParameterSource;
use serde::Serialize;

use crate::resolution::{BoundMethod, DeclinedRule};

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
        "meters_per_second_squared" => "m/s2",
        "newton_seconds" => "N.s",
        "newtons_per_kilogram" => "N/kg",
        "kilograms_to_the_exponent" => "kg^e",
        "newtons_per_kilogram_to_the_exponent" => "N/kg^e",
        "watts" => "W",
        "percent" => "%",
        // A count, a yes-or-no and a ratio are read from the label and the number. There is no
        // symbol to draw beside them and the registry's own word for the unit is not one.
        "count" | "boolean" | "dimensionless" => "",
        other => other,
    }
}

/// The word a number reads as, where its unit has words rather than a magnitude.
///
/// One home, because a yes-or-no is stored as one and zero on every surface that carries data
/// and read as a word only where a person is reading it. Two renderers deciding this
/// separately would let one of them draw `1.0000 boolean`.
pub fn reads_as_words(unit: &str, value: f64) -> Option<&'static str> {
    match unit {
        "boolean" if value >= 0.5 => Some("yes"),
        "boolean" => Some("no"),
        _ => None,
    }
}

/// A value the request binds for the whole analysis rather than for any one rule, and the
/// claim about where it came from.
///
/// Filed under the word every surface already spells this namespace with. The sweep offers
/// `global.gravity_meters_per_second_squared` as an axis and the browser's own axis is
/// `global:gravity`, so a reader meeting this row has met its name before.
///
/// Twelve rules read the analysis gravity and none of them records it, because none of their
/// registry entries declares such a parameter and a rule may not record a parameter its entry
/// does not carry. The value moves five of eleven numbers: measured on subject 01's first
/// trial at 9.80665 against 9.75, both jump heights, modified reactive strength, system mass
/// and takeoff velocity move, and the six that do not are the two instants, the two spans,
/// system weight and the net impulse.
#[derive(Debug, Clone, Serialize)]
pub struct BoundGlobal {
    pub name: &'static str,
    pub value: f64,
    /// As the registry spells units, with the symbol beside it for the same reason `Metric`
    /// carries both: a surface drawing the short form must not derive it a second way.
    pub unit: &'static str,
    pub unit_symbol: &'static str,
    pub source: ParameterSource,
}

impl BoundGlobal {
    pub fn of(name: &'static str, value: f64, unit: &'static str, source: ParameterSource) -> Self {
        Self {
            name,
            value,
            unit,
            unit_symbol: unit_symbol(unit),
            source,
        }
    }
}

/// One quantity this build can report, declared once.
///
/// A rule that produces a new quantity adds a row, and the manifest, the Python getters and
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

/// What the spine reports, which is what the pipeline computes from the landmarks directly
/// rather than through a rule of its own.
pub const SPINE_QUANTITIES: &[Quantity] = &[
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

/// Every quantity this build can report: the spine's, then one per bound rule, deduplicated
/// by key.
///
/// Assembled rather than transcribed. A rule that reports a quantity declares it on its own
/// binding row, so adding a rule cannot leave a key out of the population a manifest
/// publishes, and cannot add one the naming guards never read.
///
/// Deduplicated by key on purpose, and the first declaration wins. Three rules report peak
/// force and they are three answers to one question, not three quantities, so the key is
/// held still and `computed_by` varies per result. What may not vary is the label or the
/// unit, and `every_rule_reporting_one_key_agrees_on_what_it_is` holds that.
pub static QUANTITIES: LazyLock<Vec<Quantity>> = LazyLock::new(|| {
    let mut declared: Vec<Quantity> = Vec::new();
    for quantity in SPINE_QUANTITIES
        .iter()
        .chain(crate::binding::BINDINGS.iter().flat_map(|b| b.quantities))
    {
        if !declared.iter().any(|seen| seen.key == quantity.key) {
            declared.push(*quantity);
        }
    }
    declared
});

/// The declaration for a key, or nothing when no quantity carries it.
pub fn quantity(key: &str) -> Option<&'static Quantity> {
    QUANTITIES.iter().find(|quantity| quantity.key == key)
}

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub key: String,
    pub label: String,
    /// The number, and `None` where there is none. Never a value that is not finite: those
    /// arrive as `None` with `carried_no_number` set beside them.
    pub value: Option<f64>,
    /// True when the arithmetic ran and produced a value that is not finite.
    ///
    /// `serde_json` writes a non-finite float as `null`, exactly as it writes a quantity no
    /// rule produced. This tells the two apart: a gap in the recording reaching the number,
    /// against a refusal `refusals` names the rule for.
    ///
    /// Measured on `subject01_trial1_interrupted`: 8 of 11 metrics read `null`, and 2 of
    /// those 8 are this state. They are exactly the two computed over the weighing window
    /// that holds the recording's three unreadable samples.
    ///
    /// Always written, never skipped when false, for the reason `registry_version` is always
    /// written: a key a document sometimes omits cannot be told apart from a surface that
    /// never carried it.
    pub carried_no_number: bool,
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
        Self::from_declaration(declared, value, contributing_method_ids, note)
    }

    /// A number reported by one rule, taking its name and unit from the shared declaration
    /// and its arithmetic from the rule's own row.
    ///
    /// Three rules report peak force. Reading `computed_by` off the shared declaration would
    /// name whichever of them was declared first on every result, which is a citation the
    /// rule that ran did not earn.
    pub fn from_declaration(
        declared: &Quantity,
        value: Option<f64>,
        contributing_method_ids: Vec<String>,
        note: Option<String>,
    ) -> Self {
        let shared =
            quantity(declared.key).unwrap_or_else(|| panic!("{} is not declared", declared.key));
        // The one place a non-finite result becomes a state a reader can name. Every metric
        // this build reports is constructed here, so a rule cannot hand a NaN past this line
        // whatever it computed.
        let carried_no_number = value.is_some_and(|number| !number.is_finite());
        let value = value.filter(|number| number.is_finite());
        Self {
            key: shared.key.to_string(),
            label: shared.label.to_string(),
            value,
            carried_no_number,
            unit: shared.unit.to_string(),
            unit_symbol: unit_symbol(shared.unit).to_string(),
            contributing_method_ids,
            computed_by: declared.computed_by.map(str::to_string),
            note,
        }
    }
}

impl AnalysisResponse {
    /// The metric carrying a key, or nothing where no rule reported one.
    ///
    /// One lookup for every surface, so a repeated key resolves the same way on all of them.
    pub fn metric(&self, key: &str) -> Option<&Metric> {
        self.metrics.iter().find(|metric| metric.key == key)
    }
}

/// What an interface draws on the trace.
///
/// Every member is optional and none of them is ever a value that is not finite.
///
/// Which of the two a `null` here is, a quantity that was not computed or a quantity whose
/// arithmetic produced no number, is answered by the metric of the same key, where
/// `carried_no_number` says so. `weighing_standard_deviation_newtons` is not a reported
/// quantity, and `the_weighing_statistics_and_their_metrics_agree_about_having_no_number`
/// holds it to the metric that shares its window.
#[derive(Debug, Clone, Serialize)]
pub struct Levels {
    pub system_weight_newtons: Option<f64>,
    pub weighing_standard_deviation_newtons: Option<f64>,
    pub onset_band_lower_newtons: Option<f64>,
    pub onset_band_upper_newtons: Option<f64>,
    pub takeoff_threshold_newtons: Option<f64>,
}

/// A level an interface can draw, and `None` for anything that is not a finite number.
///
/// One function rather than a `filter` at each of the five construction sites, so a level
/// that stopped being checked would be a missing call rather than a missing clause.
pub fn drawable(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisResponse {
    /// Samples of the recording that carried no number, counted over the trace as it was
    /// handed to `run` rather than over whatever a conditioning rule made of it, because the
    /// question is about the recording and the answer must not move when a caller asks for a
    /// filter.
    ///
    /// On the response rather than on each surface's own reader report, so a notebook and an
    /// R session carry it as well as a terminal and a browser tab, and so the count has one
    /// home.
    ///
    /// The reader's own count of what its declared convention matched is a different fact and
    /// stays with the reader. A zero convention on a jump trace matches the whole flight
    /// phase, so the two added together are a number nobody can take apart.
    pub samples_carrying_no_number: usize,
    pub weighing_start_index: usize,
    pub weighing_end_index: usize,
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub levels: Levels,
    pub bound_methods: Vec<BoundMethod>,
    /// What the request bound for the whole analysis, which no rule's row can carry because
    /// no rule's entry declares it.
    pub bound_globals: Vec<BoundGlobal>,
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
    /// The same failures `warnings` describes, kept as the records they were, each naming the
    /// construct whose rule produced nothing and the id that rule was reached by.
    ///
    /// It crosses the wire as the typed record rather than as the sentence beside it, so a
    /// surface branches on the record instead of parsing the sentence back apart.
    pub refusals: Vec<DeclinedRule>,
}

#[cfg(test)]
mod unit_reading_tests {
    use super::*;

    /// A unit the registry spells out is drawn as a symbol or as nothing, never as the
    /// spelling. A reader met `1.0000 boolean` on the terminal the day the first yes-or-no
    /// quantity landed, and every unit this build reports is checked here rather than the
    /// ones that already had symbols.
    #[test]
    fn every_unit_this_build_reports_is_drawn_as_a_symbol_or_as_nothing() {
        let mut spelled_out = Vec::new();
        for quantity in QUANTITIES.iter() {
            let symbol = unit_symbol(quantity.unit);
            if symbol == quantity.unit && quantity.unit.contains('_') {
                spelled_out.push(format!("{} is drawn as {}", quantity.key, quantity.unit));
            }
        }
        assert!(
            spelled_out.is_empty(),
            "{} of {} reported quantities draw the registry's spelling where a symbol belongs:\
             \n  {}",
            spelled_out.len(),
            QUANTITIES.len(),
            spelled_out.join("\n  ")
        );
    }

    /// A yes-or-no reads as a word above the halfway mark and as the other word below it, and
    /// nothing else reads as a word at all.
    #[test]
    fn a_yes_or_no_reads_as_a_word_and_a_magnitude_does_not() {
        assert_eq!(reads_as_words("boolean", 1.0), Some("yes"));
        assert_eq!(reads_as_words("boolean", 0.0), Some("no"));
        assert_eq!(reads_as_words("newtons", 1.0), None);
        assert_eq!(reads_as_words("count", 1.0), None);
    }
}
