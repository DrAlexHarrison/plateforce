//! Which rules this build can run, and the construct each one fills.
//!
//! A binding is the link between a registry id and a core function. An id with no row
//! here has no rule behind it, and asking for one is refused rather than served by the
//! nearest neighbour.

use serde::Serialize;

pub const WEIGHING_CONSTRUCT: &str = "system_weight";
pub const ONSET_CONSTRUCT: &str = "movement_onset";
pub const TAKEOFF_CONSTRUCT: &str = "takeoff";

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
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        id: "bwepoch.fixed_window",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Fixed window at the start of the recording",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "bwepoch.adaptive_lowest_variance",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Quietest window in the recording",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "bwepoch.manual_placement",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Window placed by hand",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.noise_relative",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Noise-relative threshold, k SD of the quiet epoch",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.relative_to_system_weight",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Fixed fraction below system weight",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.absolute_force",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Absolute departure in newtons",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.last_within_band",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Last sample still inside the noise band",
        composed_from: Some("onset.threshold.noise_relative"),
        note: "Composition: onset.op.crossing_selection bound to last.",
    },
    Binding {
        id: "onset.threshold.adaptive_trailing_window",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Threshold recomputed from a trailing window",
        composed_from: None,
        note: "The registry files this concept as bwepoch.rolling_trailing_window, in group B with the reference-epoch rules.",
    },
    Binding {
        id: "takeoff.threshold.absolute_force",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "First sustained run below a residual threshold",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "takeoff.threshold.longest_run",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Longest run below the threshold",
        composed_from: Some("takeoff.threshold.absolute_force"),
        note: "Composition: onset.op.crossing_selection bound to longest_run at the falling edge.",
    },
    Binding {
        id: "takeoff.threshold.descending_crossing",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Sample before a confirmed descending crossing",
        composed_from: Some("takeoff.threshold.absolute_force"),
        note: "Composition: onset.op.direction bound at the falling edge.",
    },
    Binding {
        id: "takeoff.threshold.flight_noise_k_sd",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Threshold re-estimated from the flight phase itself",
        composed_from: None,
        note: "",
    },
];

pub fn bindings_for(slot: &str) -> impl Iterator<Item = &'static Binding> + '_ {
    BINDINGS.iter().filter(move |binding| binding.slot == slot)
}

pub(crate) fn unbound_method_message(method_id: &str, slot: &str) -> String {
    let available: Vec<&str> = bindings_for(slot).map(|binding| binding.id).collect();
    format!(
        "'{method_id}' was passed as the {slot} method, and the rules available for that step are {available:?}"
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
