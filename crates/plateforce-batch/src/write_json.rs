//! The envelope every surface returns.
//!
//! One string, built once, so the browser, the library and Python can be compared byte for
//! byte rather than approximately. Key ordering is deterministic because `serde_json` orders
//! object keys, which is what makes that comparison achievable at all.

use serde_json::{json, Value};

use crate::engine::{BatchResult, Coverage, RunRefusal};
use crate::relations::RunRow;

impl BatchResult {
    /// `{"ok": {...}}`, and never both keys.
    pub fn to_json(&self) -> String {
        Value::Object(
            [("ok".to_string(), self.to_value())]
                .into_iter()
                .collect::<serde_json::Map<String, Value>>(),
        )
        .to_string()
    }

    pub fn to_value(&self) -> Value {
        json!({
            "run": self.run,
            "quantities": self.quantities,
            "units": self.units,
            "results": self.results,
            "provenance": self.provenance,
            "descriptions": self.descriptions,
            "refusals": self.refusals,
            "warnings": self.warnings,
            "signals": self.signals,
            "aggregates": self.aggregates,
            "exclusions": self.exclusions,
        })
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let body = value
            .get("ok")
            .ok_or_else(|| "the envelope carries a refusal rather than a result".to_string())?;
        let read = |key: &str| body.get(key).cloned().unwrap_or(Value::Array(Vec::new()));
        let run: RunRow = serde_json::from_value(read("run")).map_err(|error| error.to_string())?;
        let results: Vec<crate::relations::ResultRow> =
            serde_json::from_value(read("results")).map_err(|error| error.to_string())?;
        Ok(Self {
            coverage: coverage_of(&run, &results),
            run,
            results,
            quantities: serde_json::from_value(read("quantities"))
                .map_err(|error| error.to_string())?,
            units: serde_json::from_value(
                value
                    .get("ok")
                    .and_then(|b| b.get("units"))
                    .cloned()
                    .unwrap_or(Value::Object(Default::default())),
            )
            .map_err(|error| error.to_string())?,
            provenance: serde_json::from_value(read("provenance"))
                .map_err(|error| error.to_string())?,
            descriptions: serde_json::from_value(read("descriptions"))
                .map_err(|error| error.to_string())?,
            refusals: serde_json::from_value(read("refusals"))
                .map_err(|error| error.to_string())?,
            warnings: serde_json::from_value(read("warnings"))
                .map_err(|error| error.to_string())?,
            signals: serde_json::from_value(read("signals")).map_err(|error| error.to_string())?,
            aggregates: serde_json::from_value(read("aggregates"))
                .map_err(|error| error.to_string())?,
            exclusions: serde_json::from_value(read("exclusions"))
                .map_err(|error| error.to_string())?,
        })
    }
}

/// The counts a read-back result reports, taken from the record it arrived with.
///
/// Every one of them is on the `run` row, and the two counts over the rows are taken over the
/// rows themselves. A result rebuilt with zeroes here reports a run that read nothing, which
/// is a measurement nobody made.
fn coverage_of(run: &RunRow, results: &[crate::relations::ResultRow]) -> Coverage {
    Coverage {
        files_found: run.files_found,
        files_without_declared_suffix: run.files_without_declared_suffix,
        files_unidentified: run.files_unidentified,
        trial_count: run.trial_count,
        results_written: results.len(),
        computed: run.computed_count,
        refused: run.refusal_count,
        excluded: run.trials_excluded,
        carrying_a_refusal_code: crate::engine::rows_carrying_a_refusal_code(results),
    }
}

impl RunRefusal {
    /// `{"refusal": {...}}`. A run that declined before reading a trial names every choice
    /// that is still open, so a caller can print candidates without a second query.
    pub fn to_json(&self) -> String {
        json!({ "refusal": self }).to_string()
    }
}

/// The envelope a surface returns either way, so a caller reads one shape.
pub fn envelope(outcome: &Result<BatchResult, RunRefusal>) -> String {
    match outcome {
        Ok(result) => result.to_json(),
        Err(refusal) => refusal.to_json(),
    }
}
