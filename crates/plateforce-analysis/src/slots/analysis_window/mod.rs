//! The stretch of the recording a number is taken over.
//!
//! Every extremum in the registry, and every impulse, is taken over a window. Which window
//! is a published choice with entries that disagree, and the rules here are the first thing
//! any of them needs: a peak taken over the whole recording includes the landing, and a peak
//! taken from onset to takeoff does not, on the same trace.
//!
//! The span is published under two names because a span has two ends and a later rule reads
//! whichever it needs. Both names are constants here rather than spellings a consumer
//! retypes, so a rule that reads this window names the rule that placed it.

pub mod fixed_duration_isometric;
pub mod force_dropoff_from_running_max;
pub mod named_phase;
pub mod positive_impulse;
pub mod stated_by_caller;
pub mod takeoff_detected;

use crate::binding::Binding;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "analysis_window";

/// The first sample inside the window.
pub const START: &str = "analysis_window.start";
/// The first sample past the window, matching the half-open convention the core integrates
/// and takes maxima over.
pub const END: &str = "analysis_window.end";

/// The window a rule needs, or nothing when no rule placed one. Both ends or neither: a
/// span with one end is not a span.
pub fn span(context: &crate::derived::DerivedContext) -> Option<(usize, usize)> {
    Some((context.sample(START)?, context.sample(END)?))
}

/// Every rule filed under this construct, for a refusal that names what the caller could
/// have asked for.
pub fn available() -> Vec<String> {
    crate::binding::bindings_for_construct(CONSTRUCT)
        .map(|binding: &Binding| binding.id.to_string())
        .collect()
}
