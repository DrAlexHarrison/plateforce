//! The envelope every surface returns.
//!
//! One string, built once, so the browser, the library and Python can be compared byte for
//! byte rather than approximately. Key ordering is deterministic because `serde_json` orders
//! object keys, which is what makes that comparison achievable at all.

use serde_json::{json, Value};

use crate::engine::{BatchResult, RunRefusal};

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
            "results": self.results,
            "provenance": self.provenance,
            "refusals": self.refusals,
            "warnings": self.warnings,
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
        Ok(Self {
            run: serde_json::from_value(read("run")).map_err(|error| error.to_string())?,
            quantities: serde_json::from_value(read("quantities"))
                .map_err(|error| error.to_string())?,
            results: serde_json::from_value(read("results")).map_err(|error| error.to_string())?,
            provenance: serde_json::from_value(read("provenance"))
                .map_err(|error| error.to_string())?,
            refusals: serde_json::from_value(read("refusals"))
                .map_err(|error| error.to_string())?,
            warnings: serde_json::from_value(read("warnings"))
                .map_err(|error| error.to_string())?,
            aggregates: serde_json::from_value(read("aggregates"))
                .map_err(|error| error.to_string())?,
            exclusions: serde_json::from_value(read("exclusions"))
                .map_err(|error| error.to_string())?,
            coverage: Default::default(),
        })
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
