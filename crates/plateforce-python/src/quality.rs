//! What the software already knows about a number it is handing back.
//!
//! Nothing here computes or decides anything. `plateforce_analysis::quality` raises the
//! signals, for every surface, and this carries them to a notebook as the fields they are.
//!
//! This surface writes no sentence. It reports the status the signal declares and leaves the
//! value absent, so a caller branching on either reads the record rather than prose about it.

use plateforce_analysis::quality::{QualitySignal as CoreQualitySignal, QualityStatus};
use pyo3::prelude::*;

/// One thing the software noticed about a value it is reporting.
///
/// A signal is not a refusal. The number stands and the reader decides what to do about the
/// comparison, which is why every one of these carries an action rather than a verdict.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "QualitySignal"
)]
#[derive(Clone)]
pub struct QualitySignal {
    inner: CoreQualitySignal,
}

impl QualitySignal {
    pub(crate) fn of(inner: CoreQualitySignal) -> Self {
        Self { inner }
    }

    pub(crate) fn qualifies_key(&self, key: &str) -> bool {
        self.inner.qualifies.iter().any(|held| held == key)
    }
}

/// The word this surface reports for each status, asked of the status itself, so a caller
/// branching on this and one reading the JSON are reading one decision.
fn status_name(status: QualityStatus) -> &'static str {
    status.wire_name()
}

#[pymethods]
impl QualitySignal {
    /// What was compared, in the reader's words.
    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    /// The computed value the threshold was applied to, or None where the comparison
    /// produced no number.
    ///
    /// Why a comparison produced nothing is what `status` and `remedy` are for.
    #[getter]
    fn value(&self) -> Option<f64> {
        self.inner.value
    }

    #[getter]
    fn unit(&self) -> &str {
        self.inner.unit
    }

    /// The figure the value was held against. A choice, so it travels with the signal rather
    /// than sitting inside the comparison where a reader cannot see it.
    #[getter]
    fn threshold(&self) -> f64 {
        self.inner.threshold
    }

    /// `disagrees` where two routes to one quantity differ past what the published
    /// difference between them accounts for, `incomparable` where the check could not run.
    #[getter]
    fn status(&self) -> &'static str {
        status_name(self.inner.status)
    }

    /// An action, never a verdict.
    #[getter]
    fn remedy(&self) -> &str {
        &self.inner.remedy
    }

    /// The construct whose bound rule the reader would change.
    ///
    /// A construct rather than a rule id, because naming one rule would resolve a live
    /// methodological debate on the reader's behalf. Reach the published alternatives with
    /// it: `[entry for entry in registry.methods() if entry.construct ==
    /// signal.remedy_construct]`.
    #[getter]
    fn remedy_construct(&self) -> &str {
        self.inner.remedy_construct
    }

    /// The quantities this signal is about, by the engine's name for each, so a caller
    /// places it beside the value it qualifies without a second lookup table.
    #[getter]
    fn qualifies(&self) -> Vec<&str> {
        self.inner.qualifies.iter().map(String::as_str).collect()
    }

    fn __repr__(&self) -> String {
        // Every field as it stands, and no sentence about an absent value.
        format!(
            "QualitySignal('{}', status='{}', value={}, threshold={} {}, remedy_construct='{}')",
            self.inner.label,
            self.status(),
            match self.inner.value {
                Some(value) => format!("{value:.4}"),
                None => "None".to_string(),
            },
            self.inner.threshold,
            self.inner.unit,
            self.inner.remedy_construct
        )
    }
}
