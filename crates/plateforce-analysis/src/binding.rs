//! Which rules this build can run, and the construct each one fills.
//!
//! A binding is the link between a registry id and a core function. An id with no row
//! here has no rule behind it, and asking for one is refused rather than served by the
//! nearest neighbour.

use serde::Serialize;

use crate::derived::DerivedRule;
use crate::response::Quantity;

pub const WEIGHING_CONSTRUCT: &str = "system_weight";
pub const ONSET_CONSTRUCT: &str = "movement_onset";
pub const TAKEOFF_CONSTRUCT: &str = "takeoff";

/// The three the request names by its own fields, in the order `run` resolves them. Every
/// request names all three, so a rule that reads one of their answers and finds none is
/// looking at a rule that ran and declined, never at a choice nobody made.
pub const SPINE_CONSTRUCTS: &[&str] = &[WEIGHING_CONSTRUCT, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT];

/// How `run` reaches a rule.
///
/// One field rather than a set of optional function pointers, so a row cannot be in two
/// states at once and a spine row says what it is rather than being defined by what it is
/// not.
#[derive(Debug, Clone, Copy)]
pub enum Dispatch {
    /// One of the three landmark rules, which `run` calls in a fixed order because each
    /// reads what the last one settled. Reached through the request's own named field.
    Spine,
    /// Computed from what the landmark rules resolved, reached by construct id through
    /// `AnalysisRequest::derived`.
    Derived(DerivedRule),
}

/// One rule this build can run, and the slot it fills.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Binding {
    pub id: &'static str,
    pub slot: &'static str,
    pub construct: &'static str,
    pub title: &'static str,
    /// The registry row this id binds an operator on. Under the per-kind-of-rule grain a
    /// composition is a method plus bound parameters, so it carries the base row's
    /// citations and the fingerprint carries the binding.
    pub composed_from: Option<&'static str>,
    pub note: &'static str,
    /// The quantities this rule can report, declared here so one row carries a rule's
    /// metadata and everything it produces. A surface listing quantities reads the table
    /// rather than a transcription of it.
    pub quantities: &'static [Quantity],
    /// A function pointer is `Copy` and is not `Serialize`, so the row stays `Copy` and the
    /// field is skipped. Every surface that reads a `Binding` reads the fields beside it.
    #[serde(skip)]
    pub dispatch: Dispatch,
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        id: "bwepoch.fixed_window",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Fixed window at the start of the recording",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "bwepoch.adaptive_lowest_variance",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Quietest window in the recording",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "bwepoch.manual_placement",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Window placed by hand",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "onset.threshold.noise_relative",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Noise-relative threshold, k SD of the quiet epoch",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "onset.threshold.relative_to_system_weight",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Fixed fraction below system weight",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "onset.threshold.absolute_force",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Absolute departure in newtons",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "onset.threshold.last_within_band",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Last sample still inside the noise band",
        composed_from: Some("onset.threshold.noise_relative"),
        note: "Composition: onset.op.crossing_selection bound to last.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "onset.threshold.adaptive_trailing_window",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Threshold recomputed from a trailing window",
        composed_from: None,
        note: "The registry files this concept as bwepoch.rolling_trailing_window, in group B with the reference-epoch rules.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.absolute_force",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "First sustained run below a residual threshold",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.longest_run",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Longest run below the threshold",
        composed_from: Some("takeoff.threshold.absolute_force"),
        note: "Composition: onset.op.crossing_selection bound to longest_run at the falling edge.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.descending_crossing",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Sample before a confirmed descending crossing",
        composed_from: Some("takeoff.threshold.absolute_force"),
        note: "Composition: onset.op.direction bound at the falling edge.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.flight_noise_k_sd",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Threshold re-estimated from the flight phase itself",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.landing_shape",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "First low-force run the recording closes with a landing",
        composed_from: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    // Everything below the spine runs after it, in this order. A rule that reads what
    // another rule placed is declared after it, which is the whole of the ordering: the
    // window is placed here and every peak below reads it.
    Binding {
        id: crate::slots::analysis_window::takeoff_detected::ID,
        slot: crate::slots::analysis_window::CONSTRUCT,
        construct: crate::slots::analysis_window::CONSTRUCT,
        title: "The recording up to the sample takeoff was placed at",
        composed_from: None,
        note: "",
        quantities: crate::slots::analysis_window::takeoff_detected::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::analysis_window::takeoff_detected::RULE),
    },
    Binding {
        id: crate::slots::analysis_window::fixed_duration_isometric::ID,
        slot: crate::slots::analysis_window::CONSTRUCT,
        construct: crate::slots::analysis_window::CONSTRUCT,
        title: "A fixed test length measured from onset",
        composed_from: None,
        note: "Scoped to an isometric test by its entry. It runs on any recording and answers the question it was asked.",
        quantities: crate::slots::analysis_window::fixed_duration_isometric::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::analysis_window::fixed_duration_isometric::RULE),
    },
    Binding {
        id: crate::slots::peak_force::gross::ID,
        slot: crate::slots::peak_force::CONSTRUCT,
        construct: crate::slots::peak_force::CONSTRUCT,
        title: "The biggest force, system weight included",
        composed_from: None,
        note: "",
        quantities: crate::slots::peak_force::gross::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::peak_force::gross::RULE),
    },
    Binding {
        id: crate::slots::peak_force::net::ID,
        slot: crate::slots::peak_force::CONSTRUCT,
        construct: crate::slots::peak_force::CONSTRUCT,
        title: "The biggest force above standing weight",
        composed_from: None,
        note: "",
        quantities: crate::slots::peak_force::net::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::peak_force::net::RULE),
    },
    Binding {
        id: crate::slots::peak_force::estimator::ID,
        slot: crate::slots::peak_force::CONSTRUCT,
        construct: crate::slots::peak_force::CONSTRUCT,
        title: "The biggest force, read off a centred average of stated width",
        composed_from: None,
        note: "",
        quantities: crate::slots::peak_force::estimator::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::peak_force::estimator::RULE),
    },
    Binding {
        id: crate::slots::time_to_takeoff::onset_to_takeoff::ID,
        slot: crate::slots::time_to_takeoff::CONSTRUCT,
        construct: crate::slots::time_to_takeoff::CONSTRUCT,
        title: "From the placed onset to the placed takeoff",
        composed_from: None,
        note: "",
        quantities: crate::slots::time_to_takeoff::onset_to_takeoff::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::time_to_takeoff::onset_to_takeoff::RULE),
    },
    Binding {
        id: crate::slots::flight_time::takeoff_to_touchdown::ID,
        slot: crate::slots::flight_time::CONSTRUCT,
        construct: crate::slots::flight_time::CONSTRUCT,
        title: "From the placed takeoff to the return to the plate",
        composed_from: None,
        note: "",
        quantities: crate::slots::flight_time::takeoff_to_touchdown::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::flight_time::takeoff_to_touchdown::RULE),
    },
    Binding {
        id: crate::slots::jump_height_takeoff_frame::impulse_momentum::ID,
        slot: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the velocity the net impulse gave",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_takeoff_frame::impulse_momentum::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jump_height_takeoff_frame::impulse_momentum::RULE),
    },
    Binding {
        id: crate::slots::jump_height_takeoff_frame::flight_time::ID,
        slot: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the time spent off the plate",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_takeoff_frame::flight_time::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jump_height_takeoff_frame::flight_time::RULE),
    },
    Binding {
        id: crate::slots::jump_height_takeoff_frame::peak_velocity::ID,
        slot: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the highest velocity reached",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_takeoff_frame::peak_velocity::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jump_height_takeoff_frame::peak_velocity::RULE),
    },
    Binding {
        id: crate::slots::jump_height_takeoff_frame::work_energy::ID,
        slot: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jump_height_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the work the net force did",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_takeoff_frame::work_energy::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jump_height_takeoff_frame::work_energy::RULE),
    },
    Binding {
        id: crate::slots::jump_height_standing_frame::double_integration::ID,
        slot: crate::slots::jump_height_standing_frame::CONSTRUCT,
        construct: crate::slots::jump_height_standing_frame::CONSTRUCT,
        title: "Rise from standing, as the apex of one integrated curve",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_standing_frame::double_integration::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jump_height_standing_frame::double_integration::RULE,
        ),
    },
    Binding {
        id: crate::slots::jump_height_standing_frame::tov_plus_displacement::ID,
        slot: crate::slots::jump_height_standing_frame::CONSTRUCT,
        construct: crate::slots::jump_height_standing_frame::CONSTRUCT,
        title: "Rise from standing, as the rise to takeoff plus the flight",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_standing_frame::tov_plus_displacement::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jump_height_standing_frame::tov_plus_displacement::RULE,
        ),
    },
    Binding {
        id: crate::slots::jump_height_undeclared::frame::ID,
        slot: crate::slots::jump_height_undeclared::CONSTRUCT,
        construct: crate::slots::jump_height_undeclared::CONSTRUCT,
        title: "Which rise the height denotes, stated by the reader",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_undeclared::frame::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jump_height_undeclared::frame::RULE),
    },
    Binding {
        id: crate::slots::jump_height_undeclared::flight_phase_displacement::ID,
        slot: crate::slots::jump_height_undeclared::CONSTRUCT,
        construct: crate::slots::jump_height_undeclared::CONSTRUCT,
        title: "The apex of the curve, searched between takeoff and landing",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_undeclared::flight_phase_displacement::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jump_height_undeclared::flight_phase_displacement::RULE,
        ),
    },
    Binding {
        id: crate::slots::jump_height_undeclared::minimum_of_two_routes::ID,
        slot: crate::slots::jump_height_undeclared::CONSTRUCT,
        construct: crate::slots::jump_height_undeclared::CONSTRUCT,
        title: "The smaller of the flight-time and takeoff-velocity heights",
        composed_from: None,
        note: "",
        quantities: crate::slots::jump_height_undeclared::minimum_of_two_routes::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jump_height_undeclared::minimum_of_two_routes::RULE,
        ),
    },
];

/// Every rule reached by construct id through the request rather than by a named field.
pub fn derived_bindings() -> impl Iterator<Item = &'static Binding> {
    BINDINGS
        .iter()
        .filter(|binding| matches!(binding.dispatch, Dispatch::Derived(_)))
}

/// Every construct a rule computed from the landmarks fills, in declaration order without
/// repeats. What a request may name in its `derived` map, and what a refusal lists when it
/// names one this build runs no rule for.
pub fn derived_constructs() -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for binding in derived_bindings() {
        if !seen.contains(&binding.construct) {
            seen.push(binding.construct);
        }
    }
    seen
}

pub fn bindings_for(slot: &str) -> impl Iterator<Item = &'static Binding> + '_ {
    BINDINGS.iter().filter(move |binding| binding.slot == slot)
}

/// Every rule filed under a construct.
///
/// The three shipped bindings carry a short slot name beside their construct id, and those
/// names cover three constructs of the fifty-eight the registry declares. A rule for any
/// other construct has no slot name to be found by, so a lookup keyed on the construct is
/// the one that reaches all of them.
pub fn bindings_for_construct(construct: &str) -> impl Iterator<Item = &'static Binding> + '_ {
    BINDINGS
        .iter()
        .filter(move |binding| binding.construct == construct)
}

/// Every construct a rule in this build can fill, in declaration order without repeats.
pub fn executable_constructs() -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for binding in BINDINGS {
        if !seen.contains(&binding.construct) {
            seen.push(binding.construct);
        }
    }
    seen
}

pub(crate) fn unbound_method_message(method_id: &str, slot: &str) -> String {
    unbound_method_refusal(method_id, slot)
        .message()
        .to_string()
}

/// The construct a slot word names, read off the table rather than written down twice.
///
/// The registry declares constructs and declares no `weighing` or `onset`, so a refusal
/// carrying a slot word hands a caller a name it cannot look up. `None` for a step this
/// build runs no rule for, which has no construct to read off the table.
pub fn construct_for_slot(slot: &str) -> Option<&'static str> {
    bindings_for(slot).map(|binding| binding.construct).next()
}

/// An id with no rule behind it, as the record rather than as a sentence.
///
/// Public because every surface refuses this case and each one that formatted its own
/// sentence for it was a second description of one failure.
pub fn unbound_method_refusal(method_id: &str, slot: &str) -> plateforce_core::Refusal {
    let available: Vec<String> = bindings_for(slot)
        .map(|binding| binding.id.to_string())
        .collect();
    plateforce_core::Refusal::method_not_implemented(
        method_id,
        construct_for_slot(slot).unwrap_or(slot),
        available,
    )
}

/// An id with no rule behind it is refused rather than run under the nearest rule, which
/// would carry a published author's citation onto a number that author's method did not
/// produce.
pub(crate) fn expect_bound(method_id: &str, slot: &str) -> Result<(), String> {
    if bindings_for(slot).any(|binding| binding.id == method_id) {
        return Ok(());
    }
    Err(unbound_method_message(method_id, slot))
}
