//! Whether a trial is admitted to analysis, and the four published tests that decide it.
//!
//! These are the only rules in the build whose failure mode is silence. A rejected trial
//! produces no number, no message and no row in an export, and a folder analysis inherits the
//! exclusion without knowing. So nothing here removes anything: each rule reports what it
//! observed, what it compared that against, and its verdict, and whether a firing gate
//! excludes a trial is a decision made above this crate with the report in hand.
//!
//! The four answer four different questions. Pre-loading before the effort, a countermovement
//! inside a trial meant not to hold one, transient peaks in braking, and a flight phase whose
//! duration falls outside a plausible window. A request carries one rule per construct, so
//! each reports quantities the others do not and choosing one settles which of the four tests
//! this analysis ran. That is what `rules_answer = "their_own_questions"` on the construct
//! says about them, and it is measured by the reading at the bottom of `pipeline.rs` rather
//! than declared here.
//!
//! A rule that cannot reach what it needs declines by name. It does not report a third value
//! meaning unknown: a gate whose verdict reads admitted when it never ran is the silent
//! default this registry exists to prevent, and a boolean has no room to say which of the two
//! it is.

pub mod countermovement_contamination;
pub mod flight_time_window;
pub mod pretension_ceiling;
pub mod transient_peak_count;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "trial_validity";

/// A verdict as the number a metric carries. One is admitted and zero is excluded, so a mean
/// over a set of trials is the admitted fraction rather than a code a reader has to look up.
pub(crate) fn admitted(fired: bool) -> Option<f64> {
    Some(f64::from(u8::from(!fired)))
}
