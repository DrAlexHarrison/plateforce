//! Every registry id this crate names, in one place.
//!
//! An id spelled into a `format!` in one function and into a binding table in another is two
//! vocabularies that agree until they do not. One rule here used to succeed under an id that
//! resolves and decline under one that does not, so which name a caller saw depended on
//! whether the rule worked.

pub const BWEPOCH_FIXED_WINDOW: &str = "bwepoch.fixed_window";
pub const BWEPOCH_ADAPTIVE_LOWEST_VARIANCE: &str = "bwepoch.adaptive_lowest_variance";
pub const BWEPOCH_MANUAL_PLACEMENT: &str = "bwepoch.manual_placement";

pub const ONSET_THRESHOLD_NOISE_RELATIVE: &str = "onset.threshold.noise_relative";
pub const ONSET_THRESHOLD_RELATIVE_TO_SYSTEM_WEIGHT: &str =
    "onset.threshold.relative_to_system_weight";
pub const ONSET_THRESHOLD_ABSOLUTE_FORCE: &str = "onset.threshold.absolute_force";
pub const ONSET_THRESHOLD_LAST_WITHIN_BAND: &str = "onset.threshold.last_within_band";
pub const ONSET_THRESHOLD_ADAPTIVE_TRAILING_WINDOW: &str =
    "onset.threshold.adaptive_trailing_window";

pub const TAKEOFF_THRESHOLD_ABSOLUTE_FORCE: &str = "takeoff.threshold.absolute_force";
pub const TAKEOFF_THRESHOLD_LONGEST_RUN: &str = "takeoff.threshold.longest_run";
pub const TAKEOFF_THRESHOLD_DESCENDING_CROSSING: &str = "takeoff.threshold.descending_crossing";
pub const TAKEOFF_THRESHOLD_FLIGHT_NOISE_K_SD: &str = "takeoff.threshold.flight_noise_k_sd";

/// Implemented, measured against its sibling, and reachable from no surface: it has no
/// binding and no registry entry, so it is absent from `ALL` rather than counted as
/// selectable.
pub const ONSET_THRESHOLD_NOISE_RELATIVE_FINAL_DEPARTURE: &str =
    "onset.threshold.noise_relative_final_departure";

/// Every id a surface can select. What `CAPABILITY.json` reports as this build's vocabulary,
/// and the set the binding table has to match exactly.
pub const ALL: &[&str] = &[
    BWEPOCH_FIXED_WINDOW,
    BWEPOCH_ADAPTIVE_LOWEST_VARIANCE,
    BWEPOCH_MANUAL_PLACEMENT,
    ONSET_THRESHOLD_NOISE_RELATIVE,
    ONSET_THRESHOLD_RELATIVE_TO_SYSTEM_WEIGHT,
    ONSET_THRESHOLD_ABSOLUTE_FORCE,
    ONSET_THRESHOLD_LAST_WITHIN_BAND,
    ONSET_THRESHOLD_ADAPTIVE_TRAILING_WINDOW,
    TAKEOFF_THRESHOLD_ABSOLUTE_FORCE,
    TAKEOFF_THRESHOLD_LONGEST_RUN,
    TAKEOFF_THRESHOLD_DESCENDING_CROSSING,
    TAKEOFF_THRESHOLD_FLIGHT_NOISE_K_SD,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn no_id_is_listed_twice() {
        let unique: BTreeSet<&str> = ALL.iter().copied().collect();
        assert_eq!(unique.len(), ALL.len(), "{ALL:?}");
    }

    #[test]
    fn every_id_is_dotted_and_lowercase() {
        for id in ALL {
            assert!(id.contains('.'), "{id}");
            assert_eq!(*id, id.to_lowercase(), "{id}");
        }
    }

    #[test]
    fn the_rule_no_surface_can_select_is_not_counted_as_selectable() {
        assert!(!ALL.contains(&ONSET_THRESHOLD_NOISE_RELATIVE_FINAL_DEPARTURE));
    }
}
