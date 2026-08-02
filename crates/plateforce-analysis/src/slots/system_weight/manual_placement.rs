//! `bwepoch.manual_placement`: the window a user dragged onto the trace.
//!
//! The span it covers is a span no paper published, so the registry gives this rule its own
//! name for the window's length and the placement travels in the fingerprint as this rule
//! rather than as the rule the user started from. Where the window is read from the trace
//! is `fixed_window`'s arithmetic, which this rule states a start for rather than searches.

pub(crate) const WINDOW_LENGTH_PARAMETER: &str = "span_seconds";
