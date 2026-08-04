//! `onset.threshold.noise_relative`: k standard deviations of the quiet epoch.

use plateforce_core::onset::{onset_noise_relative, BandSides, DegenerateBandPolicy};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::{Trial, WeighingEpoch};

use crate::resolution::{format_number, Resolution, RuleRefusal};
use crate::slots::movement_onset::{
    direction, onset_search, record_inherited_spread, OnsetDirection,
};

pub(crate) const APPLIES_BACKTRACK: bool = true;

const RULE_ID: &str = "onset.threshold.noise_relative";

pub(crate) fn crossing(
    trial: &Trial,
    epoch: &WeighingEpoch,
    inherited_spread: (&str, bool),
    resolved: &mut Resolution,
) -> Result<usize, RuleRefusal> {
    let force = trial.force();
    let rate = trial.sample_rate_hz();
    let search = onset_search(trial, epoch, resolved)?;
    let k = resolved.number("k", 5.0);
    record_inherited_spread(resolved, inherited_spread)?;
    let chosen_direction = direction(resolved);
    // Refuse rather than substitute. A collapsed band means the window the rule
    // assumed was quiet was not, and a silent fallback would hide that.
    let degenerate_band = match resolved.stated("degenerate_fraction") {
        Some(fraction) => {
            resolved.entailed(RULE_ID, "degenerate_band", "fraction_of_reference")?;
            resolved.record_measured(
                "degenerate_fraction",
                fraction,
                format_number(fraction),
                ParameterSource::Stated,
            );
            DegenerateBandPolicy::FractionOfReference(fraction)
        }
        None => {
            resolved.entailed(RULE_ID, "degenerate_band", "refuse")?;
            DegenerateBandPolicy::Refuse
        }
    };
    chosen_direction.and_then(|chosen| {
        let sides = match chosen {
            OnsetDirection::BelowOnly => BandSides::BelowOnly,
            OnsetDirection::TwoSided => BandSides::BothSides,
        };
        onset_noise_relative(
            force,
            epoch.system_weight_newtons,
            epoch.standard_deviation_newtons,
            k,
            sides,
            degenerate_band,
            &search,
            rate,
        )
        .map_err(RuleRefusal::Trial)
    })
}
