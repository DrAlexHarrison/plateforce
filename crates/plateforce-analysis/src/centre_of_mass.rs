//! The centre-of-mass series a height rule reads, and the four integration choices behind it.
//!
//! Those four are entries on `net_impulse` and `takeoff_velocity`, not parameters of any
//! jump-height entry, so no jump-height rule states them and every one of them inherits the
//! same four. Each is recorded on the rule that read the series: a displacement integrated
//! under a quadrature nobody stated is a silent default.

use plateforce_core::provenance::ParameterSource;
use plateforce_core::{
    centre_of_mass_displacement_meters, centre_of_mass_velocity_meters_per_second,
    DisplacementSeries, IntegrationAnchor, IntegrationDirection, IntegrationSpec, IntegrationStart,
    QuadratureRule, Trial, VelocitySeries, WeighingEpoch,
};

use crate::resolution::Resolution;

/// The dimension each recorded name stands for. The value against each is the registry id of
/// the entry chosen for it, so a reader looks the choice up rather than reading a word.
const QUADRATURE: &str = "integration_rule";
const DIRECTION: &str = "integration_direction";
const START: &str = "integration_start";
const ANCHOR: &str = "integration_anchor";

/// The spec `takeoff_velocity_meters_per_second` integrates under, built here so a rule that
/// adds a displacement term to a takeoff velocity adds it to the same series rather than to a
/// second one that agrees with it only approximately.
pub(crate) fn spec_anchored_at(onset_index: usize) -> IntegrationSpec {
    IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: IntegrationStart::DetectedOnset { index: onset_index },
        anchor: IntegrationAnchor::SinglePoint { index: onset_index },
    }
}

/// The four ids the spec names, written into the record of what the rule read.
fn record(resolved: &mut Resolution, spec: &IntegrationSpec) {
    let [quadrature, _, _, _] = spec.method_ids();
    resolved.record(QUADRATURE, quadrature.to_string(), ParameterSource::Assumed);
    record_operators(resolved, spec);
}

/// The direction, start, and anchor composed onto a quadrature rule.
pub(crate) fn record_operators(resolved: &mut Resolution, spec: &IntegrationSpec) {
    let [_, direction, start, anchor] = spec.method_ids();
    for (name, id) in [(DIRECTION, direction), (START, start), (ANCHOR, anchor)] {
        resolved.record(name, id.to_string(), ParameterSource::Assumed);
    }
}

/// The four ids behind a series a core function integrated for itself.
///
/// `takeoff_velocity_meters_per_second` integrates inside core, so a rule calling it sees no
/// spec at the call site and the choices would travel with the number unrecorded. That the
/// four recorded here are the four core used is held by
/// `the_recorded_integration_choices_are_the_ones_the_takeoff_velocity_ran_under`.
pub(crate) fn record_choices(resolved: &mut Resolution, onset_index: usize) {
    record(resolved, &spec_anchored_at(onset_index));
}

/// Centre-of-mass velocity from the placed onset, with the four choices recorded.
pub(crate) fn velocity(
    trial: &Trial,
    epoch: &WeighingEpoch,
    onset_index: usize,
    gravity_meters_per_second_squared: f64,
    resolved: &mut Resolution,
) -> VelocitySeries {
    let spec = spec_anchored_at(onset_index);
    record(resolved, &spec);
    centre_of_mass_velocity_meters_per_second(
        trial,
        epoch,
        &spec,
        gravity_meters_per_second_squared,
    )
}

/// Centre-of-mass displacement, zero at the placed onset, so its value at any later sample is
/// the rise from quiet standing to that sample.
pub(crate) fn displacement(
    trial: &Trial,
    epoch: &WeighingEpoch,
    onset_index: usize,
    gravity_meters_per_second_squared: f64,
    resolved: &mut Resolution,
) -> DisplacementSeries {
    let spec = spec_anchored_at(onset_index);
    let velocity = velocity(
        trial,
        epoch,
        onset_index,
        gravity_meters_per_second_squared,
        resolved,
    );
    centre_of_mass_displacement_meters(&velocity, &spec)
}

/// The last sample the athlete was still in contact for.
///
/// Takeoff is placed at the first sample of the run below the threshold, so the interval
/// across it is already flight and the value that belongs to the takeoff instant is the one
/// before. `takeoff_velocity_meters_per_second` reads its series at the same sample, and a
/// displacement read one sample later would be a rise the plate did not measure.
pub(crate) fn last_sample_in_contact(takeoff_index: usize) -> usize {
    takeoff_index.saturating_sub(1)
}
