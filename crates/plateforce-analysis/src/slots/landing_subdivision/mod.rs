//! Where the landing is split in two, on the only landing partition the jump literature
//! carries.
//!
//! Impact runs from touchdown to peak force and stabilising from peak force to peak negative
//! displacement, so the split is one instant inside the landing and both sub-phases are read
//! off it. The instant is reported under one key, as the propulsion subdivision's two rules
//! are, so a second partition arriving later is an answer to the same question rather than a
//! new quantity.
//!
//! Every number here reads past takeoff. One of the six committed fixtures returns to the
//! plate, so the denominator for anything this construct reports is that one trial.

pub mod impact_stabilising;

/// The construct id, as `registry/constructs.toml` declares it.
pub const CONSTRUCT: &str = "landing_subdivision";

/// The key every rule here reports under.
pub const KEY: &str = "landing_subdivision_seconds";

/// The sample a rule here placed, under the name later rules read it by.
pub const PLACED: &str = "landing_subdivision";
