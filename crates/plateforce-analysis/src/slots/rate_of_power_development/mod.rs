//! How fast power rises, under two anchorings that disagree about where the line is drawn.
//!
//! Materially more reliable than rate of force development on the same trials, ICC 0.95 with
//! CV 7.15 percent against ICC 0.57 with CV 76.45 percent, and the source that measured that
//! recommends it as the replacement. The mechanism is not stated anywhere, and the plausible
//! one is that power multiplies a filtered-by-integration velocity with force, so it is
//! smoother than a raw first derivative.
//!
//! Both rules report the rate under one key and let `computed_by` say which produced it.
//!
//! Neither rule chooses what power is. `power.instantaneous.force_x_velocity` owns that, with
//! two required values and no published default for either: the two force terms differ by
//! system weight times velocity, which at 2.5 m/s and 800 N is 2000 W, and sign convention on
//! phase-restricted power is unmanaged across the field including inside single papers. So
//! both are read from the caller, refused when unstated, and the entry that owns them is
//! named among the entries the number rests on.

pub mod peak_to_peak_anchored;
pub mod phase_anchored;

use plateforce_core::power::{ForceTerm, PowerSeries, PowerSignConvention};

use crate::centre_of_mass;
use crate::derived::DerivedContext;
use crate::resolution::{Resolution, RuleRefusal};

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "rate_of_power_development";

/// The key both rules report under, so they are two answers to one question.
pub const KEY: &str = "rate_of_power_development_watts_per_second";

/// The entry that owns what power is, and its two required names as it spells them.
pub const POWER_ENTRY: &str = "power.instantaneous.force_x_velocity";
pub const FORCE_TERM_PARAMETER: &str = "force_term";
pub const SIGN_CONVENTION_PARAMETER: &str = "sign_convention";

/// The power series both rules read, and the choices behind it, or the refusal that says
/// which of them the caller left open.
///
/// Built here rather than in each rule, so the two rules cannot come to differ about what
/// power is while reporting under one key.
pub(crate) fn power_series(
    context: &DerivedContext,
    resolved: &mut Resolution,
    method_id: &str,
    onset_index: usize,
) -> Result<PowerSeries, RuleRefusal> {
    let force_term = resolved.required_enumerated(
        method_id,
        FORCE_TERM_PARAMETER,
        &[
            ("total", ForceTerm::GroundReaction),
            ("net", ForceTerm::NetOfSystemWeight),
        ],
    );
    let sign_convention = resolved.required_enumerated(
        method_id,
        SIGN_CONVENTION_PARAMETER,
        &[
            ("upward_positive", PowerSignConvention::UpwardPositive),
            ("downward_positive", PowerSignConvention::DownwardPositive),
        ],
    );
    // Both are consulted before either is judged, so a request stating one of them does not
    // report it as a name this rule never read.
    let (force_term, sign_convention) = match (force_term, sign_convention) {
        (Ok(term), Ok(convention)) => (term, convention),
        (Err(refusal), _) | (_, Err(refusal)) => return Err(refusal),
    };

    let velocity = centre_of_mass::velocity(
        context.trial,
        context.epoch(),
        onset_index,
        context.gravity_meters_per_second_squared,
        resolved,
    );
    plateforce_core::power::instantaneous_power_watts(
        context.trial.force(),
        &velocity,
        context.epoch().system_weight_newtons,
        force_term,
        sign_convention,
    )
    .map_err(|_| {
        RuleRefusal::Refused(Box::new(plateforce_core::Refusal::span_selects_no_samples(
            method_id,
            0,
            context.trial.len(),
        )))
    })
}

/// The entries a rate of power development rests on beyond the rules that placed its
/// landmarks: what power is, and the four integration choices the velocity was read under.
pub(crate) fn record_entries_behind(context: &DerivedContext, onset_index: usize) {
    let spec = centre_of_mass::spec_anchored_at(onset_index);
    let mut entries = vec![POWER_ENTRY];
    entries.extend(spec.method_ids());
    context.rests_on(KEY, &entries);
}
