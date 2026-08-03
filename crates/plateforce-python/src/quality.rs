//! What the software already knows about a number it is handing back.
//!
//! Nothing here computes or decides anything. `plateforce_analysis::quality` raises the
//! signals, for every surface, and this carries them to a notebook as the fields they are.
//!
//! Every other surface renders a signal into a sentence, and all three of those renderers
//! hardcode one for a signal carrying no value: the terminal and R write "not comparable"
//! and the browser writes "no second route on this trace". Both are true of the one signal
//! that ships and neither is true of a signal in general, so a second signal with an absent
//! value makes them print something false. This surface writes no sentence. It reports the
//! status the signal declares and leaves the value absent, which is what the signal holds.

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
        self.inner.qualifies.contains(&key)
    }
}

/// The word this surface reports for each status, matched exhaustively so a status added to
/// the vocabulary has to be ruled on here rather than reaching Python unnamed. The spellings
/// are the ones the wire carries, so a caller branching on this and one reading the JSON are
/// reading one decision.
fn status_name(status: QualityStatus) -> &'static str {
    match status {
        QualityStatus::Disagrees => "disagrees",
        QualityStatus::Incomparable => "incomparable",
        QualityStatus::AtSearchFloor => "at_search_floor",
    }
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
    /// None is reported as None rather than as a sentence explaining it. Why a comparison
    /// produced nothing is what `status` and `remedy` are for, and a reader who wants to
    /// branch on it should not have to parse prose to do so.
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
    /// The two are not the same fact and silence would read like neither.
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
    /// methodological debate on the reader's behalf at the moment they are most likely to
    /// accept whatever is suggested. The browser turns this into a control that focuses the
    /// selector for that construct; a notebook has no selector, so it reaches the published
    /// alternatives with it: `[entry for entry in registry.methods() if entry.construct ==
    /// signal.remedy_construct]`.
    #[getter]
    fn remedy_construct(&self) -> &str {
        self.inner.remedy_construct
    }

    /// The quantities this signal is about, by the engine's name for each, so a caller
    /// places it beside the value it qualifies without a second lookup table.
    #[getter]
    fn qualifies(&self) -> Vec<&str> {
        self.inner.qualifies.to_vec()
    }

    fn __repr__(&self) -> String {
        // Every field as it stands, and no sentence about an absent value. A repr that
        // explained why a value was missing would be this surface making the same assumption
        // the other three make, in the one place a reader trusts to be literal.
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
