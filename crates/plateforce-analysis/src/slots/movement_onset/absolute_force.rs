//! `onset.threshold.absolute_force`: a departure of a stated size in newtons.

use plateforce_core::onset::{sustained_excursion, ExcursionBand};
use plateforce_core::{Trial, WeighingEpoch};

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
            RuleRefusal::Stated(format!(
                "onset.threshold.absolute_force(threshold_n = {departure_newtons}) found no crossing"
            ))
        })
    })
}
