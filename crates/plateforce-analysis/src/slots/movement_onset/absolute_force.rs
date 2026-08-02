//! `onset.threshold.absolute_force`: a departure of a stated size in newtons.

use plateforce_core::onset::{sustained_excursion, ExcursionBand};
use plateforce_core::{Trial, TrialError, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};
use crate::slots::movement_onset::{direction, onset_search, OnsetDirection};

pub(crate) const APPLIES_BACKTRACK: bool = true;

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    resolved: &mut Resolution,
) -> Result<usize, RuleRefusal> {
    let force = trial.force();
    let search = onset_search(trial, epoch, resolved)?;
    let departure_newtons = resolved.number("threshold_n", 20.0);
    direction(resolved).and_then(|chosen| {
        let band = match chosen {
            OnsetDirection::TwoSided => {
                ExcursionBand::centred(epoch.system_weight_newtons, departure_newtons)
            }
            OnsetDirection::BelowOnly => {
                ExcursionBand::below(epoch.system_weight_newtons - departure_newtons)
            }
        };
        sustained_excursion(force, &band, &search).ok_or_else(|| {
            // The core's own error rather than a sentence, so a caller reads the code for what
            // happened instead of parsing the sentence back apart. A rule that ran and found
            // nothing is a no-crossing, and every surface can say so in the same word.
            RuleRefusal::Trial(TrialError::NoCrossing {
                method_id: "onset.threshold.absolute_force".to_string(),
                parameter: "threshold_n".to_string(),
                value: departure_newtons,
                search_bound_seconds: search.end_index.saturating_sub(search.start_index) as f64
                    / trial.sample_rate_hz(),
            })
        })
    })
}
