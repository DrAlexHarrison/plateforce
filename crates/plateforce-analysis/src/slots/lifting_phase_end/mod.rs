//! Where the positive lifting phase ends on a loaded lift, which is the boundary the
//! literature actually argues about.
//!
//! Three rules, and the gap between the first two is the braking sub-phase, whose duration is
//! load dependent. The registry records mean force 24 percent lower and mean power 29 percent
//! lower under the peak-displacement rule than under the net-force rule on the same trials,
//! p < 0.0001, with phase duration correspondingly inflated and mean velocity unaffected.
//! 24 percent on mean force is larger than any training effect the number is used to detect.
//!
//! It is the loaded-barbell analogue of the onset problem and worse, because the
//! disagreement sits at the end of the phase where the signal is large rather than at the
//! start where it is small.
//!
//! All three report one key and let `computed_by` say which produced it.

pub mod absolute_force_zero;
pub mod net_force_zero;
pub mod peak_displacement;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "lifting_phase_end";

/// The key every rule here reports under.
pub const KEY: &str = "lifting_phase_end_seconds";

/// The sample a rule here placed, under the name later rules read it by.
pub const PLACED: &str = "lifting_phase_end";

/// The sample the search for a lift end starts from: where a lift-start rule placed the
/// beginning, or the onset when no lift-start rule ran.
///
/// One home for the fallback, because three rules need it and three spellings of it could
/// come to disagree about which landmark bounds a phase.
pub(crate) fn search_start(context: &crate::derived::DerivedContext) -> Option<usize> {
    super::lifting_phase_start::placed(context).or_else(|| context.onset_index())
}
