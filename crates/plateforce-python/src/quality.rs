//! What the software already knows about a number it is handing back.
//!
//! Nothing here computes or decides anything. `plateforce_analysis::quality` raises the
//! signals, for every surface, and this carries them to a notebook as the fields they are.
//!
//! This surface writes no sentence at all. It reports the status the signal declares and
//! leaves the value absent, which is what the signal holds, so a caller branching on either
//! reads the record rather than prose about it.
//!
//! The three rendering surfaces write a sentence because a person is reading it, and each
//! now names the status in the record's own spelling for the same reason this one does. They
//! arrived there the hard way: all three once hardcoded a phrase written for the first signal
//! that shipped, so the terminal and R told a reader "not comparable" and the browser told
//! them "no second route on this trace" whatever the signal was about, and a second signal
//! with an absent value would have printed a sentence that was false about their own data.

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

/// The word this surface reports for each status, asked of the status itself.
///
/// It used to be spelled again here, correctly and exhaustively, and a third surface then
/// spelled it a third way. The word now has one home beside the enum, so a caller branching
/// on this and one reading the JSON are reading one decision because there is only one.
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
