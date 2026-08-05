//! The force the trace has reached a stated time after onset.
//!
//! Newtons, where a rate is newtons per second, which is why this is a construct of its own
//! rather than a second quantity on the rule that reports the rate over the same interval.
//! `rate_of_force_development` holds nine rules and declares that they answer one question,
//! so a rate rule reporting this beside its rate would cost every caller who reached for one
//! of the other eight a quantity, with nothing on the result saying so. The disjoint gate
//! does not catch that shape, because it faults on a pair where each rule withholds something
//! from the other and an extra on one side alone withholds in one direction.
//!
//! One source states the force and the rate together. Under a net convention they are one
//! quantity in two units; under a gross one the force at onset does not cancel and they are
//! two. So they are two kinds of rule that one paper describes, which is two entries.

pub mod at_epoch_from_onset;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "force_at_epoch";

/// The key the rule reports under.
pub const KEY: &str = "force_at_epoch_newtons";
