//! `onset.threshold.relative_to_system_weight`: a fixed fraction below system weight.

use plateforce_core::onset::onset_relative_to_reference;
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{Resolution, RuleRefusal};
use crate::slots::movement_onset::onset_search;

pub(crate) const APPLIES_BACKTRACK: bool = true;

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    resolved: &mut Resolution,
) -> Result<usize, RuleRefusal> {
    let search = onset_search(trial, epoch, resolved)?;
    let percent_of_system_weight = resolved.number("pct", 2.5);
    onset_relative_to_reference(
        trial.force(),
        epoch.system_weight_newtons,
        percent_of_system_weight / 100.0,
        &search,
        trial.sample_rate_hz(),
    )
    .map_err(RuleRefusal::Trial)
}
