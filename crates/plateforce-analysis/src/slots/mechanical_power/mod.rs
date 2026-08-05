//! What power is at every instant, and the interval anything read off it is read over.
//!
//! Five entries sit under this construct and they are four measurement modalities plus the one
//! this build can run. A plate integrates a measured force once to reach velocity; a
//! displacement transducer differentiates a measured displacement twice to reach force. The
//! differentiated term dominates the noise in the product, which is the physical reason the
//! modalities disagree and why the other four are walled behind an instrument rather than
//! behind unwritten code.
//!
//! Nothing here reports a number. The rule settles what power meant for this analysis, records
//! the two choices that settled it, and every rule that reads a power series names this entry
//! among the entries its number rests on. That is the same shape `conditioned_force_signal`
//! has: a construct whose rules produce a series rather than a value.
//!
//! The interval is the other half. Five entries across this family name a declared phase, and
//! a peak, a mean or an integral over an interval nobody named is the defect this project was
//! founded on. `phase_interval` is the one home for turning the registry's four published
//! names into a pair of samples other constructs' rules placed.

pub mod force_x_velocity;

use plateforce_core::power::{DeclaredPhase, ForceTerm, PowerSeries, PowerSignConvention};

use crate::centre_of_mass;
use crate::derived::DerivedContext;
use crate::resolution::{Resolution, RuleRefusal};

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "mechanical_power";

/// The entry that owns what power is, and its two required names as it spells them.
pub const POWER_ENTRY: &str = "power.instantaneous.force_x_velocity";
pub const FORCE_TERM_PARAMETER: &str = "force_term";
pub const SIGN_CONVENTION_PARAMETER: &str = "sign_convention";

/// The two values the entry publishes for each, in the entry's own spelling.
pub const FORCE_TERMS: &[(&str, ForceTerm)] = &[
    ("total", ForceTerm::GroundReaction),
    ("net", ForceTerm::NetOfSystemWeight),
];
pub const SIGN_CONVENTIONS: &[(&str, PowerSignConvention)] = &[
    ("upward_positive", PowerSignConvention::UpwardPositive),
    ("downward_positive", PowerSignConvention::DownwardPositive),
];

/// The name the five phase-reading entries publish, and the four values they publish for it.
pub const PHASE_PARAMETER: &str = "phase";
pub const PHASE_BRAKING: &str = "braking";
pub const PHASE_PROPULSION: &str = "propulsion";
pub const PHASE_MOVEMENT: &str = "movement";
pub const PHASE_ANALYSIS_WINDOW: &str = "analysis_window";

/// The three names every rule in this family cannot run without, for the sweep that would
/// otherwise read each of them as a control moving nothing because the rule declines at every
/// probe of the other two.
pub const REQUIRED_OPTIONS: &[(&str, &str)] = &[
    (FORCE_TERM_PARAMETER, "total"),
    (SIGN_CONVENTION_PARAMETER, "upward_positive"),
    (PHASE_PARAMETER, PHASE_PROPULSION),
];

/// The same, for the two rules that read a power series and take no phase.
pub const REQUIRED_OPTIONS_WITHOUT_PHASE: &[(&str, &str)] = &[
    (FORCE_TERM_PARAMETER, "total"),
    (SIGN_CONVENTION_PARAMETER, "upward_positive"),
];

/// The power series a rule reads, and the choices behind it, or the refusal that says which of
/// them the caller left open.
///
/// Built here rather than in each rule, so nine rules across four constructs cannot come to
/// differ about what power is while reporting numbers a reader will draw against each other.
pub(crate) fn power_series(
    context: &DerivedContext,
    resolved: &mut Resolution,
    method_id: &str,
    onset_index: usize,
    // The number this series feeds, or `None` from the rule that forms the series and
    // reports nothing: its gravity moves no number of its own.
    quantity_key: Option<&'static str>,
) -> Result<PowerSeries, RuleRefusal> {
    let force_term = resolved.required_enumerated(method_id, FORCE_TERM_PARAMETER, FORCE_TERMS);
    let sign_convention =
        resolved.required_enumerated(method_id, SIGN_CONVENTION_PARAMETER, SIGN_CONVENTIONS);
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
        context.gravity_behind(quantity_key),
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

/// The entries a number read off a power series rests on beyond the rules that placed its
/// landmarks: what power is, and the four integration choices the velocity was read under.
pub(crate) fn record_entries_behind(
    context: &DerivedContext,
    quantity_key: &'static str,
    onset_index: usize,
) {
    let spec = centre_of_mass::spec_anchored_at(onset_index);
    let mut entries = vec![POWER_ENTRY];
    entries.extend(spec.method_ids());
    context.rests_on(quantity_key, &entries);
}

/// The pair of samples a stated phase name resolves to, and the constructs whose rules placed
/// neither.
///
/// Both ends are asked for whichever is missing, so the chain records that this rule read them
/// and a refusal names only what is actually absent. The interval is closed at both ends,
/// which is what `DeclaredPhase` takes and what the peak, the mean and the two integrals are
/// all read over.
///
/// The analysis window is published half-open, so its far end is the first sample past the
/// window and the last sample inside it is the one before. Read without that step a work value
/// over the window would integrate one interval the window does not contain.
pub(crate) fn phase_interval(
    context: &DerivedContext,
    resolved: &mut Resolution,
    method_id: &str,
) -> Result<DeclaredPhase, RuleRefusal> {
    let phase = resolved.required_enumerated(
        method_id,
        PHASE_PARAMETER,
        &[
            (PHASE_BRAKING, PHASE_BRAKING),
            (PHASE_PROPULSION, PHASE_PROPULSION),
            (PHASE_MOVEMENT, PHASE_MOVEMENT),
            (PHASE_ANALYSIS_WINDOW, PHASE_ANALYSIS_WINDOW),
        ],
    )?;

    let (first, last, missing) = match phase {
        PHASE_BRAKING => bounded(
            crate::slots::braking_phase_start::placed(context),
            crate::slots::propulsion_phase_start::placed(context),
            crate::slots::braking_phase_start::CONSTRUCT,
            crate::slots::propulsion_phase_start::CONSTRUCT,
        ),
        PHASE_PROPULSION => bounded(
            crate::slots::propulsion_phase_start::placed(context),
            crate::slots::propulsion_phase_end::placed(context),
            crate::slots::propulsion_phase_start::CONSTRUCT,
            crate::slots::propulsion_phase_end::CONSTRUCT,
        ),
        PHASE_MOVEMENT => bounded(
            context.onset_index(),
            context.takeoff_index(),
            crate::binding::ONSET_CONSTRUCT,
            crate::binding::TAKEOFF_CONSTRUCT,
        ),
        _ => {
            let span = crate::slots::analysis_window::span(context);
            bounded(
                span.map(|(start, _)| start),
                // The window's far end is the first sample past it.
                span.map(|(_, end)| end.saturating_sub(1)),
                crate::slots::analysis_window::CONSTRUCT,
                crate::slots::analysis_window::CONSTRUCT,
            )
        }
    };

    match (first, last) {
        (Some(first_index), Some(last_index)) if last_index > first_index => Ok(DeclaredPhase {
            first_index,
            last_index,
            method_id: method_id.to_string(),
        }),
        _ => Err(context.unavailable(method_id, &missing)),
    }
}

/// Which of an interval's two ends no rule placed, or the far end where both were placed and
/// the interval runs backwards.
///
/// An interval of no duration is the boundaries rather than the rule that read them, and the
/// far end is the one a caller moves to fix it.
fn bounded(
    first: Option<usize>,
    last: Option<usize>,
    first_construct: &'static str,
    last_construct: &'static str,
) -> (Option<usize>, Option<usize>, Vec<&'static str>) {
    let mut missing = Vec::new();
    if first.is_none() {
        missing.push(first_construct);
    }
    if last.is_none() {
        missing.push(last_construct);
    }
    if missing.is_empty() {
        missing.push(last_construct);
    }
    (first, last, missing)
}
