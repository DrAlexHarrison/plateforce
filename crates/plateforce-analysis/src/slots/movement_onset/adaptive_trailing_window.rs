//! `onset.threshold.adaptive_trailing_window`: the threshold recomputed from a window that
//! trails the sample under test, so it never reads the quiet epoch the other rules share.

use plateforce_core::onset::onset_adaptive_trailing_window;
use plateforce_core::Trial;

use crate::resolution::{Resolution, RuleRefusal};

/// The trailing window resolves its own backtrack.
pub(crate) const APPLIES_BACKTRACK: bool = false;

pub(crate) fn crossing(trial: &Trial, resolved: &mut Resolution) -> Result<usize, RuleRefusal> {
    let rate = trial.sample_rate_hz();
    let window_samples = resolved
        .seconds_as_samples("window_seconds", 1.0, rate)
        .max(2);
    let k = resolved.number("k", 5.0);
    let dispersion = resolved.dispersion()?;
    onset_adaptive_trailing_window(trial.force(), window_samples, k, dispersion, rate)
        .map_err(RuleRefusal::Trial)
}
