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
    /// A rule that conditions the signal the landmark rules then read. Runs before them,
    /// because the thresholds they resolve are scaled by what it produces.
    Conditioning(crate::conditioning::ConditioningRule),
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
    /// The entry a result reached by this id is recorded against, set only where the id is
    /// not itself an entry. A reader looks up what the record names, so an id that names no
    /// entry may be selected but may never be recorded.
    ///
    /// Composing does not put a row here. `takeoff.threshold.descending_crossing` composes
    /// and has an entry of its own, so it records under itself and this is `None`. What puts
    /// a row here is the registry already enumerating the choice as a value of an operator's
    /// parameter, which leaves the compound name a second spelling of a pair the registry
    /// spells already.
    pub records_under: Option<&'static str>,
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
    // First, because it produces the signal every rule below reads. A conditioning rule
    // that ran after the landmarks would leave the record saying they were placed on a
    // signal they were not.
    Binding {
        id: crate::slots::conditioned_force_signal::none::ID,
        slot: crate::slots::conditioned_force_signal::CONSTRUCT,
        construct: crate::slots::conditioned_force_signal::CONSTRUCT,
        title: "No conditioning before event detection or integration",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: &[],
        dispatch: Dispatch::Conditioning(crate::slots::conditioned_force_signal::none::apply),
    },
    Binding {
        id: "bwepoch.fixed_window",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Fixed window at the start of the recording",
        composed_from: None,
        records_under: None,
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
        records_under: None,
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
        records_under: None,
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
        records_under: None,
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
        records_under: None,
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
        records_under: None,
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
        records_under: Some("onset.threshold.noise_relative"),
        note: "Composition: onset.op.crossing_selection bound to last, above a search bound at the force minimum.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "onset.threshold.adaptive_trailing_window",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Threshold recomputed from a trailing window",
        composed_from: None,
        records_under: None,
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
        records_under: None,
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
        records_under: Some("takeoff.threshold.absolute_force"),
        note: "Composition: takeoff.op.crossing_selection bound to longest_run.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.descending_crossing",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Sample before a confirmed descending crossing",
        composed_from: Some("takeoff.threshold.absolute_force"),
        records_under: None,
        note: "Composition: onset.op.direction bound at the falling edge. An entry of its own, so it records under itself.",
        quantities: &[],
        dispatch: Dispatch::Spine,
    },
    Binding {
        id: "takeoff.threshold.flight_noise_k_sd",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Threshold re-estimated from the flight phase itself",
        composed_from: None,
        records_under: None,
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
        records_under: None,
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
        records_under: None,
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
        records_under: None,
        note: "Scoped to an isometric test by its entry. It runs on any recording and answers the question it was asked.",
        quantities: crate::slots::analysis_window::fixed_duration_isometric::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::analysis_window::fixed_duration_isometric::RULE),
    },
    // Beside their two siblings rather than at the end of the array, for the reason the
    // comment above gives: every peak below reads the window, and a window rule declared
    // after its readers places nothing they can see. A caller choosing either of these two
    // would get a peak over no window at all, which is the silent default this build exists
    // to refuse rather than a rule that declined.
    Binding {
        id: crate::slots::analysis_window::force_dropoff_from_running_max::ID,
        slot: crate::slots::analysis_window::CONSTRUCT,
        construct: crate::slots::analysis_window::CONSTRUCT,
        title: "The window ends where a smoothed force falls below its running maximum",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::analysis_window::force_dropoff_from_running_max::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::analysis_window::force_dropoff_from_running_max::RULE,
        ),
    },
    Binding {
        id: crate::slots::analysis_window::positive_impulse::ID,
        slot: crate::slots::analysis_window::CONSTRUCT,
        construct: crate::slots::analysis_window::CONSTRUCT,
        title: "The interval over which vertical force exceeds system weight",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::analysis_window::positive_impulse::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::analysis_window::positive_impulse::RULE),
    },
    Binding {
        id: crate::slots::peak_force::gross::ID,
        slot: crate::slots::peak_force::CONSTRUCT,
        construct: crate::slots::peak_force::CONSTRUCT,
        title: "The biggest force, system weight included",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::peak_force::gross::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::peak_force::gross::RULE),
    },
    Binding {
        id: crate::slots::peak_force::estimator::ID,
        slot: crate::slots::peak_force::CONSTRUCT,
        construct: crate::slots::peak_force::CONSTRUCT,
        title: "The biggest force, read off a centred average of stated width",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::peak_force::estimator::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::peak_force::estimator::RULE),
    },
    // The peak with system weight taken out, which fills a construct of its own. The two rules
    // above report `peak_force_newtons` and this one reports `net_peak_force_newtons`, so one
    // construct would carry a caller who names this one away from the number the others give.
    Binding {
        id: crate::slots::net_peak_force::net::ID,
        slot: crate::slots::net_peak_force::CONSTRUCT,
        construct: crate::slots::net_peak_force::CONSTRUCT,
        title: "The biggest force above standing weight",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::net_peak_force::net::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::net_peak_force::net::RULE),
    },
    Binding {
        id: crate::slots::time_to_takeoff::onset_to_takeoff::ID,
        slot: crate::slots::time_to_takeoff::CONSTRUCT,
        construct: crate::slots::time_to_takeoff::CONSTRUCT,
        title: "From the placed onset to the placed takeoff",
        composed_from: None,
        records_under: None,
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
        records_under: None,
        note: "",
        quantities: crate::slots::flight_time::takeoff_to_touchdown::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::flight_time::takeoff_to_touchdown::RULE),
    },
    // The two landing rules, declared before the heights because a drop-jump height integrates
    // from the sample one of them places. They are two claims about the same instant and the
    // registry calls the disagreement genuine: tying the rising edge to the threshold that
    // placed takeoff makes a threshold error compound across the flight phase, and flight-time
    // height goes as the square of flight time.
    Binding {
        id: crate::slots::landing::tied_to_takeoff::ID,
        slot: crate::slots::landing::CONSTRUCT,
        construct: crate::slots::landing::CONSTRUCT,
        title: "First return above the threshold that placed takeoff",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::landing::tied_to_takeoff::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::landing::tied_to_takeoff::RULE),
    },
    Binding {
        id: crate::slots::landing::absolute_force::ID,
        slot: crate::slots::landing::CONSTRUCT,
        construct: crate::slots::landing::CONSTRUCT,
        title: "First sustained run above a threshold stated for the rising edge",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::landing::absolute_force::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::landing::absolute_force::RULE),
    },
    Binding {
        id: crate::slots::net_impulse::as_performance_determinant::ID,
        slot: crate::slots::net_impulse::CONSTRUCT,
        construct: crate::slots::net_impulse::CONSTRUCT,
        title: "Force above standing weight over the interval, and the velocity it gave",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::net_impulse::as_performance_determinant::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::net_impulse::as_performance_determinant::RULE),
    },
    Binding {
        id: crate::slots::jh_takeoff_frame::impulse_momentum::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the velocity the net impulse gave",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_takeoff_frame::impulse_momentum::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::impulse_momentum::RULE),
    },
    Binding {
        id: crate::slots::jh_takeoff_frame::flight_time::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the time spent off the plate",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_takeoff_frame::flight_time::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::flight_time::RULE),
    },
    Binding {
        id: crate::slots::jh_takeoff_frame::peak_velocity::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the highest velocity reached",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_takeoff_frame::peak_velocity::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::peak_velocity::RULE),
    },
    Binding {
        id: crate::slots::jh_takeoff_frame::work_energy::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the work the net force did",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_takeoff_frame::work_energy::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::work_energy::RULE),
    },
    Binding {
        id: crate::slots::jh_takeoff_frame::mcmahon_correction_factor::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff on a drop jump, from the arrival the standing period recovers",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_takeoff_frame::mcmahon_correction_factor::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::mcmahon_correction_factor::RULE),
    },
    Binding {
        id: crate::slots::jh_standing_frame::double_integral::ID,
        slot: crate::slots::jh_standing_frame::CONSTRUCT,
        construct: crate::slots::jh_standing_frame::CONSTRUCT,
        title: "Rise from standing, as the apex of one integrated curve",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_standing_frame::double_integral::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jh_standing_frame::double_integral::RULE,
        ),
    },
    Binding {
        id: crate::slots::jh_standing_frame::tov_plus_rise::ID,
        slot: crate::slots::jh_standing_frame::CONSTRUCT,
        construct: crate::slots::jh_standing_frame::CONSTRUCT,
        title: "Rise from standing, as the rise to takeoff plus the flight",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_standing_frame::tov_plus_rise::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jh_standing_frame::tov_plus_rise::RULE,
        ),
    },
    Binding {
        id: crate::slots::jh_undeclared::frame::ID,
        slot: crate::slots::jh_undeclared::CONSTRUCT,
        construct: crate::slots::jh_undeclared::CONSTRUCT,
        title: "Which rise the height denotes, stated by the reader",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_undeclared::frame::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_undeclared::frame::RULE),
    },
    Binding {
        id: crate::slots::jh_undeclared::flight_apex::ID,
        slot: crate::slots::jh_undeclared::CONSTRUCT,
        construct: crate::slots::jh_undeclared::CONSTRUCT,
        title: "The apex of the curve, searched between takeoff and landing",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_undeclared::flight_apex::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jh_undeclared::flight_apex::RULE,
        ),
    },
    Binding {
        id: crate::slots::jh_undeclared::minimum_of_two::ID,
        slot: crate::slots::jh_undeclared::CONSTRUCT,
        construct: crate::slots::jh_undeclared::CONSTRUCT,
        title: "The smaller of the flight-time and takeoff-velocity heights",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_undeclared::minimum_of_two::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::jh_undeclared::minimum_of_two::RULE,
        ),
    },
    // The two corrections and the drop-jump initial condition, declared after the four routes
    // they correct or extend. Each reads a length, an angle or a height off the request that no
    // rule can read off the trace, and declines by name where the caller stated none, which is
    // the entry asking for the evidence it lacks rather than filling one in.
    Binding {
        id: crate::slots::jh_takeoff_frame::ankle_angle_corrected::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, from the flight with the landing posture taken out",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_takeoff_frame::ankle_angle_corrected::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::ankle_angle_corrected::RULE),
    },
    Binding {
        id: crate::slots::jh_takeoff_frame::drop_from_box_height::ID,
        slot: crate::slots::jh_takeoff_frame::CONSTRUCT,
        construct: crate::slots::jh_takeoff_frame::CONSTRUCT,
        title: "Rise from takeoff, with the arrival taken from the height of the box",
        composed_from: None,
        records_under: None,
        note: "Scoped to a drop jump by its entry. It runs on any recording and answers the question it was asked.",
        quantities: crate::slots::jh_takeoff_frame::drop_from_box_height::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_takeoff_frame::drop_from_box_height::RULE),
    },
    Binding {
        id: crate::slots::jh_standing_frame::heel_rise_constant::ID,
        slot: crate::slots::jh_standing_frame::CONSTRUCT,
        construct: crate::slots::jh_standing_frame::CONSTRUCT,
        title: "Rise from standing, as the flight plus a heel rise scaled to the athlete",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::jh_standing_frame::heel_rise_constant::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::jh_standing_frame::heel_rise_constant::RULE),
    },
    Binding {
        id: crate::slots::reactive_strength_index::jh_tov_over_ttt::ID,
        slot: crate::slots::reactive_strength_index::CONSTRUCT,
        construct: crate::slots::reactive_strength_index::CONSTRUCT,
        title: "Takeoff-velocity jump height over the time taken to produce it",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::reactive_strength_index::jh_tov_over_ttt::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::reactive_strength_index::jh_tov_over_ttt::RULE),
    },
    // The phase boundaries, in trace order, which is also dependency order: propulsion end
    // reads what braking start placed under its force option, and the phase models read the
    // propulsion boundaries.
    Binding {
        id: crate::slots::braking_phase_start::zero_net_force::ID,
        slot: crate::slots::braking_phase_start::CONSTRUCT,
        construct: crate::slots::braking_phase_start::CONSTRUCT,
        title: "Net force crosses zero upward after the minimum",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::braking_phase_start::zero_net_force::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::braking_phase_start::zero_net_force::RULE),
    },
    Binding {
        id: crate::slots::braking_phase_start::min_force::ID,
        slot: crate::slots::braking_phase_start::CONSTRUCT,
        construct: crate::slots::braking_phase_start::CONSTRUCT,
        title: "The instant of minimum vertical force following onset",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::braking_phase_start::min_force::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::braking_phase_start::min_force::RULE),
    },
    Binding {
        id: crate::slots::propulsion_phase_start::zero_velocity::ID,
        slot: crate::slots::propulsion_phase_start::CONSTRUCT,
        construct: crate::slots::propulsion_phase_start::CONSTRUCT,
        title: "Centre of mass velocity crosses zero from below",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_phase_start::zero_velocity::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_phase_start::zero_velocity::RULE),
    },
    Binding {
        id: crate::slots::propulsion_phase_start::velocity_threshold::ID,
        slot: crate::slots::propulsion_phase_start::CONSTRUCT,
        construct: crate::slots::propulsion_phase_start::CONSTRUCT,
        title: "Centre of mass velocity first exceeds a small positive threshold",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_phase_start::velocity_threshold::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_phase_start::velocity_threshold::RULE),
    },
    Binding {
        id: crate::slots::propulsion_phase_start::peak_grf::ID,
        slot: crate::slots::propulsion_phase_start::CONSTRUCT,
        construct: crate::slots::propulsion_phase_start::CONSTRUCT,
        title: "The instant of peak vertical force",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_phase_start::peak_grf::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_phase_start::peak_grf::RULE),
    },
    // Declared after braking start, which its force option reads and names.
    Binding {
        id: crate::slots::propulsion_phase_end::peak_com_velocity::ID,
        slot: crate::slots::propulsion_phase_end::CONSTRUCT,
        construct: crate::slots::propulsion_phase_end::CONSTRUCT,
        title: "Propulsion ends at maximum centre of mass velocity rather than at takeoff",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_phase_end::peak_com_velocity::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_phase_end::peak_com_velocity::RULE),
    },
    // Beside the rule it disagrees with rather than at the end of the array, because this
    // position is what the two subdivisions read. Declared after them, a caller composing this
    // boundary with a subdivision would meet the subdivision first and be told the propulsion
    // end placed nothing, which is the one composition this entry exists to make work.
    Binding {
        id: crate::slots::propulsion_phase_end::takeoff::ID,
        slot: crate::slots::propulsion_phase_end::CONSTRUCT,
        construct: crate::slots::propulsion_phase_end::CONSTRUCT,
        title: "Propulsion ends at takeoff",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_phase_end::takeoff::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_phase_end::takeoff::RULE),
    },
    // Declared last of the phase rules: the two propulsion subdivisions read the boundaries
    // the propulsion rules placed, so the interval they split is already settled here.
    Binding {
        id: crate::slots::phase_model::unweighting_single::ID,
        slot: crate::slots::phase_model::CONSTRUCT,
        construct: crate::slots::phase_model::CONSTRUCT,
        title: "One unweighting phase from onset to peak negative velocity",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::phase_model::unweighting_single::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::phase_model::unweighting_single::RULE),
    },
    Binding {
        id: crate::slots::phase_model::unloading_yielding_split::ID,
        slot: crate::slots::phase_model::CONSTRUCT,
        construct: crate::slots::phase_model::CONSTRUCT,
        title: "Unloading and eccentric yielding split at the force minimum",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::phase_model::unloading_yielding_split::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::phase_model::unloading_yielding_split::RULE),
    },
    Binding {
        id: crate::slots::phase_model::time_epochs::ID,
        slot: crate::slots::phase_model::CONSTRUCT,
        construct: crate::slots::phase_model::CONSTRUCT,
        title: "Fixed time epochs measured from contraction onset",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::phase_model::time_epochs::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::phase_model::time_epochs::RULE),
    },
    // The two propulsion subdivisions, which fill a construct of their own. A phase model and
    // a subdivision are two answers one request carries, and one construct would carry one.
    Binding {
        id: crate::slots::propulsion_subdivision::by_time::ID,
        slot: crate::slots::propulsion_subdivision::CONSTRUCT,
        construct: crate::slots::propulsion_subdivision::CONSTRUCT,
        title: "Split the propulsion phase at half its duration",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_subdivision::by_time::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_subdivision::by_time::RULE),
    },
    Binding {
        id: crate::slots::propulsion_subdivision::by_force_crossing::ID,
        slot: crate::slots::propulsion_subdivision::CONSTRUCT,
        construct: crate::slots::propulsion_subdivision::CONSTRUCT,
        title: "Split the propulsion phase where force descends through system weight",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::propulsion_subdivision::by_force_crossing::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_subdivision::by_force_crossing::RULE),
    },
    // How fast force rises, in the published variants that disagree about what that means.
    // Declared after the phase rules, because two of them read the propulsion boundaries
    // those rules placed and every rule here reads the analysis window placed above.
    Binding {
        id: crate::slots::rate_of_force_development::epoch_overlapping::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The force reached a stated time after onset, and the rate over that interval",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::epoch_overlapping::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::rate_of_force_development::epoch_overlapping::RULE),
    },
    Binding {
        id: crate::slots::rate_of_force_development::peak_sliding_window::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The steepest stretch of a stated width, anywhere in the window",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::peak_sliding_window::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_force_development::peak_sliding_window::RULE,
        ),
    },
    Binding {
        id: crate::slots::rate_of_force_development::at_fraction_of_peak::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The slope where force first reaches a stated share of its peak",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::at_fraction_of_peak::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_force_development::at_fraction_of_peak::RULE,
        ),
    },
    Binding {
        id: crate::slots::rate_of_force_development::between_force_levels::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The time from one stated force to another, and the slope between them",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::between_force_levels::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_force_development::between_force_levels::RULE,
        ),
    },
    Binding {
        id: crate::slots::rate_of_force_development::average_to_peak::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The biggest force divided by the time from onset to it",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::average_to_peak::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::rate_of_force_development::average_to_peak::RULE),
    },
    Binding {
        id: crate::slots::rate_of_force_development::phase_endpoint_secant::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The line between the force at each end of the push",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::phase_endpoint_secant::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_force_development::phase_endpoint_secant::RULE,
        ),
    },
    Binding {
        id: crate::slots::rate_of_force_development::exponential_model::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The steepest rate of a curve fitted to the rise",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::exponential_model::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_force_development::exponential_model::RULE,
        ),
    },
    Binding {
        id: crate::slots::rate_of_force_development::mean_force_over_duration::ID,
        slot: crate::slots::rate_of_force_development::CONSTRUCT,
        construct: crate::slots::rate_of_force_development::CONSTRUCT,
        title: "The average force over the push divided by how long it lasted",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_force_development::mean_force_over_duration::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_force_development::mean_force_over_duration::RULE,
        ),
    },
    // Force added up over a stretch of time. Both rules read the analysis window and one of
    // them reads the weighing epoch, under the convention the caller states.
    Binding {
        id: crate::slots::epoch_impulse::epoch_from_onset::ID,
        slot: crate::slots::epoch_impulse::CONSTRUCT,
        construct: crate::slots::epoch_impulse::CONSTRUCT,
        title: "Force added up over a stated stretch from the start of the jump",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::epoch_impulse::epoch_from_onset::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::epoch_impulse::epoch_from_onset::RULE),
    },
    Binding {
        id: crate::slots::epoch_impulse::to_fraction_of_peak::ID,
        slot: crate::slots::epoch_impulse::CONSTRUCT,
        construct: crate::slots::epoch_impulse::CONSTRUCT,
        title: "Force added up until it reaches a stated share of its peak",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::epoch_impulse::to_fraction_of_peak::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::epoch_impulse::to_fraction_of_peak::RULE),
    },
    // How fast power rises. Declared last: one of the two reads the propulsion boundaries and
    // both read the analysis window.
    Binding {
        id: crate::slots::rate_of_power_development::phase_anchored::ID,
        slot: crate::slots::rate_of_power_development::CONSTRUCT,
        construct: crate::slots::rate_of_power_development::CONSTRUCT,
        title: "From the start of the push to the instant of most power",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_power_development::phase_anchored::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::rate_of_power_development::phase_anchored::RULE),
    },
    Binding {
        id: crate::slots::rate_of_power_development::peak_to_peak_anchored::ID,
        slot: crate::slots::rate_of_power_development::CONSTRUCT,
        construct: crate::slots::rate_of_power_development::CONSTRUCT,
        title: "From the most negative power to the most positive power after it",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::rate_of_power_development::peak_to_peak_anchored::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::rate_of_power_development::peak_to_peak_anchored::RULE,
        ),
    },
    // What power is at each instant. Declared before everything that reads a power series, so
    // a caller who names it has settled the force term and the sign before any number is read
    // off the series they describe. It reports nothing itself: a series is not a value.
    Binding {
        id: crate::slots::mechanical_power::force_x_velocity::ID,
        slot: crate::slots::mechanical_power::CONSTRUCT,
        construct: crate::slots::mechanical_power::CONSTRUCT,
        title: "Force at each instant multiplied by velocity at that instant",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::mechanical_power::force_x_velocity::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::mechanical_power::force_x_velocity::RULE),
    },
    // The largest power reached. One rule reads the series and two never form one, estimating
    // the peak from a jump height and a mass.
    Binding {
        id: crate::slots::power_peak::instantaneous::ID,
        slot: crate::slots::power_peak::CONSTRUCT,
        construct: crate::slots::power_peak::CONSTRUCT,
        title: "The most power reached during a stated phase",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::power_peak::instantaneous::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::power_peak::instantaneous::RULE),
    },
    Binding {
        id: crate::slots::power_peak::from_height_lewis::ID,
        slot: crate::slots::power_peak::CONSTRUCT,
        construct: crate::slots::power_peak::CONSTRUCT,
        title: "Estimated from how high they jumped and how much they weigh",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::power_peak::from_height_lewis::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::power_peak::from_height_lewis::RULE),
    },
    Binding {
        id: crate::slots::power_peak::from_height_regression::ID,
        slot: crate::slots::power_peak::CONSTRUCT,
        construct: crate::slots::power_peak::CONSTRUCT,
        title: "Estimated from height and weight, fitted on a stated population",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::power_peak::from_height_regression::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::power_peak::from_height_regression::RULE),
    },
    // Power averaged across a stated phase, which is a different quantity from the peak and
    // sits under a construct of its own for that reason.
    Binding {
        id: crate::slots::power_mean::phase_mean::ID,
        slot: crate::slots::power_mean::CONSTRUCT,
        construct: crate::slots::power_mean::CONSTRUCT,
        title: "Power averaged across a stated phase",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::power_mean::phase_mean::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::power_mean::phase_mean::RULE),
    },
    // Work over a stated phase, by the two quadrature routes the registry files as one
    // quantity and the vendor product it files as a bias.
    Binding {
        id: crate::slots::mechanical_work::integral_power_dt::ID,
        slot: crate::slots::mechanical_work::CONSTRUCT,
        construct: crate::slots::mechanical_work::CONSTRUCT,
        title: "The area under the power curve over a stated phase",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::mechanical_work::integral_power_dt::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::mechanical_work::integral_power_dt::RULE),
    },
    Binding {
        id: crate::slots::mechanical_work::integral_force_ds::ID,
        slot: crate::slots::mechanical_work::CONSTRUCT,
        construct: crate::slots::mechanical_work::CONSTRUCT,
        title: "Force added up through the distance it moved them",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::mechanical_work::integral_force_ds::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::mechanical_work::integral_force_ds::RULE),
    },
    Binding {
        id: crate::slots::mechanical_work::single_product::ID,
        slot: crate::slots::mechanical_work::CONSTRUCT,
        construct: crate::slots::mechanical_work::CONSTRUCT,
        title: "One force value multiplied by one distance value",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::mechanical_work::single_product::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::mechanical_work::single_product::RULE),
    },
    // Which object the numbers describe, and what they are divided by. Both resolve a named
    // object to a mass, through one function, so the two cannot disagree about which mass a
    // name stands for.
    Binding {
        id: crate::slots::mechanical_object::computed_on_object::ID,
        slot: crate::slots::mechanical_object::CONSTRUCT,
        construct: crate::slots::mechanical_object::CONSTRUCT,
        title: "Whether the numbers describe the bar, the athlete, or both together",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::mechanical_object::computed_on_object::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::mechanical_object::computed_on_object::RULE),
    },
    Binding {
        id: crate::slots::normalisation_basis::denominator::ID,
        slot: crate::slots::normalisation_basis::CONSTRUCT,
        construct: crate::slots::normalisation_basis::CONSTRUCT,
        title: "Which mass the numbers are divided by",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::normalisation_basis::denominator::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::normalisation_basis::denominator::RULE),
    },
    // The velocity-based-training lineage's propulsion start, and the two phase models that
    // had no rule. All three read the propulsion boundaries settled above rather than being
    // read by them, so they sit here rather than beside their siblings.
    Binding {
        id: crate::slots::propulsion_phase_start::accel_above_neg_g::ID,
        slot: crate::slots::propulsion_phase_start::CONSTRUCT,
        construct: crate::slots::propulsion_phase_start::CONSTRUCT,
        title: "The portion of the concentric action with acceleration at or above minus g",
        composed_from: None,
        records_under: None,
        note: "The velocity-based-training lineage's partition, filed by the registry as a homonym of the velocity-sign rule rather than as a competing method.",
        quantities: crate::slots::propulsion_phase_start::accel_above_neg_g::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::propulsion_phase_start::accel_above_neg_g::RULE),
    },
    Binding {
        id: crate::slots::phase_model::squat_jump_distinct::ID,
        slot: crate::slots::phase_model::CONSTRUCT,
        construct: crate::slots::phase_model::CONSTRUCT,
        title: "The squat jump has no countermovement landmarks at all",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::phase_model::squat_jump_distinct::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::phase_model::squat_jump_distinct::RULE),
    },
    Binding {
        id: crate::slots::phase_model::downward_upward::ID,
        slot: crate::slots::phase_model::CONSTRUCT,
        construct: crate::slots::phase_model::CONSTRUCT,
        title: "Name the phases downward and upward rather than eccentric and concentric",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::phase_model::downward_upward::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::phase_model::downward_upward::RULE),
    },
    // The loaded-lift boundaries. The three end rules read where a lift-start rule placed the
    // beginning, so the two start rules are declared first, and the sticking region reads the
    // propulsion boundaries settled further up.
    Binding {
        id: crate::slots::lifting_phase_start::velocity_zero_crossing::ID,
        slot: crate::slots::lifting_phase_start::CONSTRUCT,
        construct: crate::slots::lifting_phase_start::CONSTRUCT,
        title: "The declared object's velocity changes from negative to positive",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::lifting_phase_start::velocity_zero_crossing::QUANTITIES,
        dispatch: Dispatch::Derived(
            crate::slots::lifting_phase_start::velocity_zero_crossing::RULE,
        ),
    },
    Binding {
        id: crate::slots::lifting_phase_start::visual_inspection::ID,
        slot: crate::slots::lifting_phase_start::CONSTRUCT,
        construct: crate::slots::lifting_phase_start::CONSTRUCT,
        title: "Onset of a dead-start lift placed by eye on the force-time curve",
        composed_from: None,
        records_under: None,
        note: "Its source states no algorithmic rule, so it reads the instant the request carries and refuses an unstated one by name.",
        quantities: crate::slots::lifting_phase_start::visual_inspection::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::lifting_phase_start::visual_inspection::RULE),
    },
    Binding {
        id: crate::slots::lifting_phase_end::net_force_zero::ID,
        slot: crate::slots::lifting_phase_end::CONSTRUCT,
        construct: crate::slots::lifting_phase_end::CONSTRUCT,
        title: "Net force on the declared object crosses zero downward",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::lifting_phase_end::net_force_zero::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::lifting_phase_end::net_force_zero::RULE),
    },
    Binding {
        id: crate::slots::lifting_phase_end::peak_displacement::ID,
        slot: crate::slots::lifting_phase_end::CONSTRUCT,
        construct: crate::slots::lifting_phase_end::CONSTRUCT,
        title: "The lifting phase ends at maximum vertical displacement of the tracked object",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::lifting_phase_end::peak_displacement::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::lifting_phase_end::peak_displacement::RULE),
    },
    Binding {
        id: crate::slots::lifting_phase_end::absolute_force_zero::ID,
        slot: crate::slots::lifting_phase_end::CONSTRUCT,
        construct: crate::slots::lifting_phase_end::CONSTRUCT,
        title: "The lifting phase ends where absolute force decreases to zero",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::lifting_phase_end::absolute_force_zero::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::lifting_phase_end::absolute_force_zero::RULE),
    },
    Binding {
        id: crate::slots::sticking_region::velocity_minimum::ID,
        slot: crate::slots::sticking_region::CONSTRUCT,
        construct: crate::slots::sticking_region::CONSTRUCT,
        title: "From the first local maximum of ascent bar velocity to the subsequent local minimum",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::sticking_region::velocity_minimum::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::sticking_region::velocity_minimum::RULE),
    },
    // The landing rules, last because every one of them reads past takeoff and the two below
    // read what the landing rules placed. The subdivision reads the landing end, so it is
    // declared after it.
    Binding {
        id: crate::slots::landing_phase_end::zero_com_velocity::ID,
        slot: crate::slots::landing_phase_end::CONSTRUCT,
        construct: crate::slots::landing_phase_end::CONSTRUCT,
        title: "Landing ends when reconstructed centre of mass velocity reaches zero",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::landing_phase_end::zero_com_velocity::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::landing_phase_end::zero_com_velocity::RULE),
    },
    Binding {
        id: crate::slots::landing_subdivision::impact_stabilising::ID,
        slot: crate::slots::landing_subdivision::CONSTRUCT,
        construct: crate::slots::landing_subdivision::CONSTRUCT,
        title: "Split landing into impact and stabilising",
        composed_from: None,
        records_under: None,
        note: "",
        quantities: crate::slots::landing_subdivision::impact_stabilising::QUANTITIES,
        dispatch: Dispatch::Derived(crate::slots::landing_subdivision::impact_stabilising::RULE),
    },
];

/// One name a caller may state on a landmark rule, and the registry entry that carries it.
///
/// A threshold rule carries its own threshold and the convention its spread was taken under.
/// Every other value belongs to an operator the registry files as an entry in its own right,
/// so recording one against the threshold rule puts a parameter on a row that does not have
/// it, and a reader who looks the id up does not find the value that moved the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OperatorName {
    /// What a caller writes.
    pub name: &'static str,
    /// The registry entry that publishes it, which is never the entry the request named.
    pub entry: &'static str,
}

/// Which registry entry carries each name an onset rule reads.
///
/// Data rather than a match, so a surface can enumerate what a caller may state to reach an
/// operator. Two names may reach one entry: the window searched for an excursion the other
/// side of the band is the trigger the retreat fires on, and where the retreat stops.
pub const ONSET_OPERATOR_NAMES: &[OperatorName] = &[
    OperatorName {
        name: crate::slots::movement_onset::OFFSET_MILLISECONDS,
        entry: crate::slots::movement_onset::BACKWARD_OFFSET_FIXED,
    },
    OperatorName {
        name: "span_ms",
        entry: "onset.op.persistence",
    },
    OperatorName {
        name: crate::slots::movement_onset::FLOOR_SECONDS,
        entry: crate::slots::movement_onset::SEARCH_FLOOR,
    },
    OperatorName {
        name: crate::slots::movement_onset::WEIGHING_EPOCH_END_SECONDS,
        entry: crate::slots::movement_onset::SEARCH_FLOOR_AT_WEIGHING_EPOCH_END,
    },
    OperatorName {
        name: "direction",
        entry: "onset.op.direction",
    },
    OperatorName {
        name: "selection",
        entry: crate::slots::movement_onset::CROSSING_SELECTION,
    },
    OperatorName {
        name: crate::slots::movement_onset::INVERSE_LOOKBACK_SECONDS,
        entry: crate::slots::movement_onset::BACKTRACK_TO_TOLERANCE,
    },
    OperatorName {
        name: crate::slots::movement_onset::TOLERANCE,
        entry: crate::slots::movement_onset::BACKTRACK_TO_TOLERANCE,
    },
    OperatorName {
        name: crate::slots::movement_onset::RETREAT_CAP_SAMPLES,
        entry: crate::slots::movement_onset::BACKTRACK_TO_TOLERANCE,
    },
    OperatorName {
        name: "bound",
        entry: crate::slots::movement_onset::SEARCH_UPPER_BOUND,
    },
    OperatorName {
        name: "search_bound_seconds",
        entry: crate::slots::movement_onset::SEARCH_UPPER_BOUND,
    },
];

/// The same, for a takeoff rule. A separate table because the two operator families are
/// separate registry entries, and a takeoff parameter recorded against an onset operator is a
/// value filed under a construct it never touched.
///
/// `comparison` and `short_run_handling` decide whether an unloaded plate reading negative
/// counts as flight, and whether a run too short to be a flight can win the comparison and
/// disqualify the trial. A threshold row lists neither.
pub const TAKEOFF_OPERATOR_NAMES: &[OperatorName] = &[
    OperatorName {
        name: "comparison",
        entry: crate::slots::takeoff::TAKEOFF_OP_RESIDUAL_COMPARISON,
    },
    OperatorName {
        name: "short_run_handling",
        entry: crate::slots::takeoff::TAKEOFF_OP_SHORT_RUN_HANDLING,
    },
    OperatorName {
        name: "selection",
        entry: crate::slots::takeoff::TAKEOFF_OP_CROSSING_SELECTION,
    },
    OperatorName {
        name: crate::slots::takeoff::TAKEOFF_WEIGHING_EPOCH_END_SECONDS,
        entry: crate::slots::takeoff::TAKEOFF_SEARCH_FLOOR_AT_WEIGHING_EPOCH_END,
    },
    OperatorName {
        name: crate::slots::takeoff::TAKEOFF_SEARCH_FLOOR_SECONDS,
        entry: crate::slots::takeoff::TAKEOFF_SEARCH_FLOOR_AT_TRIAL_START,
    },
];

/// The names a caller may state on a rule of this construct to reach an operator entry, and
/// nothing for a construct whose rules compose none.
pub fn operator_names_for_construct(construct: &str) -> &'static [OperatorName] {
    match construct {
        ONSET_CONSTRUCT => ONSET_OPERATOR_NAMES,
        TAKEOFF_CONSTRUCT => TAKEOFF_OPERATOR_NAMES,
        _ => &[],
    }
}

/// The entry a name reaches on a rule of this construct, and nothing for a name the rule
/// carries itself.
pub fn operator_for(construct: &str, name: &str) -> Option<&'static str> {
    operator_names_for_construct(construct)
        .iter()
        .find(|routed| routed.name == name)
        .map(|routed| routed.entry)
}

/// What a caller has to state before a rule will run, for the entries whose registry rows
/// publish no default, each with one value that answers it.
///
/// A rule whose entry states a parameter required and publishes no default cannot be reached
/// by a request that states nothing, and that is the entry working rather than the rule
/// failing. Held beside the rules rather than inside each caller, so a surface asking the
/// question and a check answering it read one list.
pub fn required_options(method_id: &str) -> &'static [(&'static str, &'static str)] {
    match method_id {
        crate::slots::propulsion_phase_end::peak_com_velocity::ID => {
            crate::slots::propulsion_phase_end::peak_com_velocity::REQUIRED_OPTIONS
        }
        crate::slots::epoch_impulse::epoch_from_onset::ID
        | crate::slots::epoch_impulse::to_fraction_of_peak::ID => {
            crate::slots::epoch_impulse::REQUIRED_OPTIONS
        }
        crate::slots::rate_of_force_development::phase_endpoint_secant::ID => {
            crate::slots::rate_of_force_development::phase_endpoint_secant::REQUIRED_OPTIONS
        }
        crate::slots::rate_of_force_development::mean_force_over_duration::ID => {
            crate::slots::rate_of_force_development::mean_force_over_duration::REQUIRED_OPTIONS
        }
        crate::slots::rate_of_force_development::between_force_levels::ID => {
            crate::slots::rate_of_force_development::between_force_levels::REQUIRED_OPTIONS
        }
        // What power is, which the two rate rules state without a phase, and the four rules
        // that read a number off a power series over an interval, which state all three.
        crate::slots::mechanical_power::force_x_velocity::ID
        | crate::slots::rate_of_power_development::phase_anchored::ID
        | crate::slots::rate_of_power_development::peak_to_peak_anchored::ID => {
            crate::slots::rate_of_power_development::REQUIRED_OPTIONS
        }
        crate::slots::power_peak::instantaneous::ID
        | crate::slots::power_mean::phase_mean::ID
        | crate::slots::mechanical_work::integral_power_dt::ID
        | crate::slots::mechanical_work::integral_force_ds::ID => {
            crate::slots::mechanical_power::REQUIRED_OPTIONS
        }
        crate::slots::mechanical_work::single_product::ID => {
            crate::slots::mechanical_work::single_product::REQUIRED_OPTIONS
        }
        crate::slots::power_peak::from_height_regression::ID => {
            crate::slots::power_peak::from_height_regression::REQUIRED_OPTIONS
        }
        crate::slots::mechanical_object::computed_on_object::ID => {
            crate::slots::mechanical_object::computed_on_object::REQUIRED_OPTIONS
        }
        crate::slots::normalisation_basis::denominator::ID => {
            crate::slots::normalisation_basis::denominator::REQUIRED_OPTIONS
        }
        _ => &[],
    }
}

/// The same, for the numbers a rule declines without, where the registry publishes no default
/// and the value is a property of the athlete rather than of the method.
///
/// The anthropometric jump-height rules need two or three of these at once, so a caller
/// stating one still meets a refusal naming the next. A check probing one parameter of such a
/// rule reads every one of its controls as moving nothing, which is the check failing to reach
/// the question rather than the control failing to matter.
pub fn required_numbers(method_id: &str) -> &'static [(&'static str, f64)] {
    match method_id {
        crate::slots::jh_takeoff_frame::ankle_angle_corrected::ID => {
            crate::slots::jh_takeoff_frame::ankle_angle_corrected::REQUIRED_NUMBERS
        }
        crate::slots::jh_standing_frame::heel_rise_constant::ID => {
            crate::slots::jh_standing_frame::heel_rise_constant::REQUIRED_NUMBERS
        }
        crate::slots::rate_of_force_development::between_force_levels::ID => {
            crate::slots::rate_of_force_development::between_force_levels::REQUIRED_NUMBERS
        }
        _ => &[],
    }
}

/// Every rule that conditions the signal before the landmark rules read it.
pub fn conditioning_bindings() -> impl Iterator<Item = &'static Binding> {
    BINDINGS
        .iter()
        .filter(|binding| matches!(binding.dispatch, Dispatch::Conditioning(_)))
}

/// Every construct conditioning fills, in declaration order without repeats. What a request
/// may name in its `conditioning` map, and what a refusal lists when it names one this build
/// runs no rule for.
pub fn conditioning_constructs() -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for binding in conditioning_bindings() {
        if !seen.contains(&binding.construct) {
            seen.push(binding.construct);
        }
    }
    seen
}

/// Whether this build conditions `construct` with `method_id`, as the record rather than as a
/// bool.
///
/// The one home for the question, so a surface reading a caller's line and the engine reading
/// the request that line built cannot answer it differently. `derive::accepts` is the same
/// predicate for the rules computed from the landmarks, and both halves are checked here for
/// the same reason: a construct with no conditioning rule behind it and an id filed under
/// another construct are different faults listing different alternatives, and either one
/// alone matches no binding, which the phase would skip in silence.
///
/// An empty id is a construct the caller named no rule for. That is what a request stating
/// values against the rule this phase runs anyway carries, and it is a state rather than a
/// name to look up: the phase runs its declared rule and records it either way.
pub fn accepts_conditioning(
    construct: &str,
    method_id: &str,
) -> Result<(), Box<plateforce_core::Refusal>> {
    let constructs = conditioning_constructs();
    if !constructs.contains(&construct) {
        return Err(Box::new(
            plateforce_core::Refusal::construct_not_on_the_path(
                construct,
                constructs.into_iter().map(str::to_string).collect(),
            ),
        ));
    }
    if method_id.is_empty() || conditioning_bindings().any(|binding| binding.id == method_id) {
        return Ok(());
    }
    Err(Box::new(plateforce_core::Refusal::method_not_implemented(
        method_id,
        construct,
        conditioning_bindings()
            .filter(|binding| binding.construct == construct)
            .map(|binding| binding.id.to_string())
            .collect(),
    )))
}

/// The registry entry a result reached by this id is recorded against.
///
/// Selecting and recording are different acts and this is the one place they diverge. A
/// caller may select a compound name the interface offers; what travels with the number is
/// the entry a stranger can look up, with the operator the compound name bound recorded
/// beside it by the rule that bound it. An id that is itself an entry answers itself, so
/// every caller can route through this without asking whether it needs to.
pub fn records_under(method_id: &str) -> &str {
    BINDINGS
        .iter()
        .find(|binding| binding.id == method_id)
        .and_then(|binding| binding.records_under)
        .unwrap_or(method_id)
}

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
/// The three spine bindings carry a short slot name beside their construct id, and those names
/// cover three constructs. Every other construct the registry declares has none, so a rule
/// filed under one has no slot name to be found by, and a lookup keyed on the construct is the
/// one that reaches all of them. `registry census` says how many constructs that is.
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
pub(crate) fn expect_bound(
    method_id: &str,
    slot: &str,
) -> Result<(), Box<plateforce_core::Refusal>> {
    if bindings_for(slot).any(|binding| binding.id == method_id) {
        return Ok(());
    }
    Err(Box::new(unbound_method_refusal(method_id, slot)))
}

#[cfg(test)]
mod operator_routing_tests {
    use super::*;

    /// The routing table and the operator id lists are one fact and were two homes. This is
    /// what the second home said before it went away, so the collapse is held to a comparison
    /// rather than to a reading.
    #[test]
    fn the_names_reach_exactly_the_operator_entries_this_build_declares() {
        for (construct, declared) in [
            (ONSET_CONSTRUCT, crate::ONSET_OPERATOR_IDS),
            (TAKEOFF_CONSTRUCT, crate::TAKEOFF_OPERATOR_IDS),
        ] {
            let mut reached: Vec<&str> = operator_names_for_construct(construct)
                .iter()
                .map(|routed| routed.entry)
                .collect();
            reached.sort();
            reached.dedup();
            let mut listed: Vec<&str> = declared.to_vec();
            listed.sort();
            assert_eq!(
                reached,
                listed,
                "{construct}: {} names reach {} entries and the build declares {}",
                operator_names_for_construct(construct).len(),
                reached.len(),
                listed.len()
            );
        }
    }

    /// The two families are separate registry entries, so a name shared between them routes to
    /// two different ids. `selection` is the one that does, and a table keyed on the name alone
    /// would file a takeoff crossing under the onset construct.
    #[test]
    fn one_name_on_two_constructs_reaches_two_entries() {
        assert_eq!(
            operator_for(ONSET_CONSTRUCT, "selection"),
            Some("onset.op.crossing_selection")
        );
        assert_eq!(
            operator_for(TAKEOFF_CONSTRUCT, "selection"),
            Some("takeoff.op.crossing_selection")
        );
        assert_eq!(operator_for(WEIGHING_CONSTRUCT, "selection"), None);
        // The control on the three above: a name no rule of either construct routes reaches
        // nothing, so the lookup is answering the name rather than answering the construct.
        assert_eq!(operator_for(ONSET_CONSTRUCT, "k"), None);
        assert_eq!(operator_for(TAKEOFF_CONSTRUCT, "threshold_n"), None);
    }
}
