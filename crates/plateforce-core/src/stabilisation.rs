//! When force settles back inside a band around system weight, and stays there.
//!
//! The published rule reports the instant the dwell completed rather than the instant the
//! band was entered, so the number it returns is one dwell longer than the settling it
//! describes. Both instants come back here, so a reader is not asked to take the difference
//! on trust.
//!
//! Nothing here decides a method. A caller passes the band and the dwell a bound rule
//! resolved, and turns an outcome that is not a settling into a refusal under the id it
//! bound.

/// Where force first stayed inside the band for a whole dwell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StabilisationFinding {
    pub entered_band_index: usize,
    /// The last sample of the dwell. What the published rule reports the time of.
    pub dwell_completed_index: usize,
    pub band_lower_newtons: f64,
    pub band_upper_newtons: f64,
    pub dwell_samples: usize,
}

/// What the search found, kept apart because a trace that ends early and one that never
/// settles are different facts about the recording and want different remedies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StabilisationOutcome {
    Stabilised(StabilisationFinding),
    /// Fewer samples remain after the search start than a dwell needs, so no window of this
    /// length could have been examined at all.
    TraceShorterThanDwell {
        available_samples: usize,
        dwell_samples: usize,
    },
    /// Long enough to examine, and force never held inside the band for the whole dwell.
    /// The longest stretch it did hold is what says how close it came.
    NeverSustained {
        longest_run_samples: usize,
        dwell_samples: usize,
    },
    /// The band or the dwell is not a quantity a search can be run against.
    Unsearchable,
}

/// The first stretch of `dwell_samples` at or after `search_from_index` in which every
/// sample lies within `band_pct_of_system_weight` of system weight.
pub fn first_sustained_band_entry(
    force_newtons: &[f64],
    search_from_index: usize,
    system_weight_newtons: f64,
    band_pct_of_system_weight: f64,
    dwell_samples: usize,
) -> StabilisationOutcome {
    if dwell_samples == 0
        || !system_weight_newtons.is_finite()
        || system_weight_newtons <= 0.0
        || !band_pct_of_system_weight.is_finite()
        || band_pct_of_system_weight < 0.0
        || search_from_index >= force_newtons.len()
    {
        return StabilisationOutcome::Unsearchable;
    }

    let half_width = system_weight_newtons * band_pct_of_system_weight / 100.0;
    let band_lower_newtons = system_weight_newtons - half_width;
    let band_upper_newtons = system_weight_newtons + half_width;

    let available_samples = force_newtons.len() - search_from_index;
    if available_samples < dwell_samples {
        return StabilisationOutcome::TraceShorterThanDwell {
            available_samples,
            dwell_samples,
        };
    }

    // One pass, carrying the run length so the failing case can say how close it came
    // rather than only that it failed.
    let mut run_start = search_from_index;
    let mut run_length = 0usize;
    let mut longest_run_samples = 0usize;
    for (offset, force) in force_newtons[search_from_index..].iter().enumerate() {
        if (band_lower_newtons..=band_upper_newtons).contains(force) {
            if run_length == 0 {
                run_start = search_from_index + offset;
            }
            run_length += 1;
            longest_run_samples = longest_run_samples.max(run_length);
            if run_length == dwell_samples {
                return StabilisationOutcome::Stabilised(StabilisationFinding {
                    entered_band_index: run_start,
                    dwell_completed_index: search_from_index + offset,
                    band_lower_newtons,
                    band_upper_newtons,
                    dwell_samples,
                });
            }
        } else {
            run_length = 0;
        }
    }

    StabilisationOutcome::NeverSustained {
        longest_run_samples,
        dwell_samples,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEIGHT_NEWTONS: f64 = 600.0;
    const BAND_PCT: f64 = 5.0;

    /// Flight, a landing spike, then settled standing.
    fn landing_then_settled(settled_samples: usize) -> Vec<f64> {
        let mut force = vec![0.0; 200];
        force.extend([1800.0, 1500.0, 900.0, 400.0]);
        force.extend(std::iter::repeat_n(WEIGHT_NEWTONS, settled_samples));
        force
    }

    #[test]
    fn the_reported_instant_is_one_whole_dwell_after_the_band_was_entered() {
        let force = landing_then_settled(2000);
        let StabilisationOutcome::Stabilised(found) =
            first_sustained_band_entry(&force, 200, WEIGHT_NEWTONS, BAND_PCT, 1200)
        else {
            panic!("a settled trace did not stabilise");
        };

        assert_eq!(found.entered_band_index, 204);
        assert_eq!(
            found.dwell_completed_index - found.entered_band_index + 1,
            found.dwell_samples,
            "the reported instant is not one dwell after the band was entered"
        );
    }

    /// A trace that ends inside the dwell gets its own answer, never a number clipped to
    /// the last sample.
    #[test]
    fn a_trace_that_ends_inside_the_dwell_says_so_rather_than_stopping_short() {
        let force = landing_then_settled(400);
        match first_sustained_band_entry(&force, 200, WEIGHT_NEWTONS, BAND_PCT, 1200) {
            StabilisationOutcome::TraceShorterThanDwell {
                available_samples,
                dwell_samples,
            } => {
                assert_eq!(available_samples, 404);
                assert_eq!(dwell_samples, 1200);
            }
            other => panic!("a trace shorter than the dwell returned {other:?}"),
        }
    }

    /// Long enough to look at, never still enough. The longest run is what says how close.
    #[test]
    fn a_trace_that_never_settles_reports_the_longest_run_it_managed() {
        let mut force = vec![0.0; 200];
        for block in 0..20 {
            force.extend(std::iter::repeat_n(WEIGHT_NEWTONS, 100));
            force.extend(std::iter::repeat_n(WEIGHT_NEWTONS * 1.4, 10 + block));
        }
        match first_sustained_band_entry(&force, 200, WEIGHT_NEWTONS, BAND_PCT, 1200) {
            StabilisationOutcome::NeverSustained {
                longest_run_samples,
                dwell_samples,
            } => {
                assert_eq!(longest_run_samples, 100);
                assert_eq!(dwell_samples, 1200);
            }
            other => panic!("a trace that never settles returned {other:?}"),
        }
    }

    /// The band is what the answer turns on, so widening it has to move the instant.
    #[test]
    fn a_wider_band_settles_the_trace_sooner() {
        let mut force = vec![0.0; 200];
        force.extend(std::iter::repeat_n(WEIGHT_NEWTONS * 1.08, 300));
        force.extend(std::iter::repeat_n(WEIGHT_NEWTONS, 2000));

        let narrow = first_sustained_band_entry(&force, 200, WEIGHT_NEWTONS, 5.0, 1200);
        let wide = first_sustained_band_entry(&force, 200, WEIGHT_NEWTONS, 10.0, 1200);

        let (StabilisationOutcome::Stabilised(narrow), StabilisationOutcome::Stabilised(wide)) =
            (narrow, wide)
        else {
            panic!("one of the two bands did not stabilise");
        };
        assert!(
            wide.entered_band_index < narrow.entered_band_index,
            "the band width moved nothing: {} against {}",
            wide.entered_band_index,
            narrow.entered_band_index
        );
    }

    #[test]
    fn a_band_or_dwell_no_search_can_be_run_against_returns_no_answer() {
        let force = landing_then_settled(2000);
        for (weight, band, dwell) in [
            (WEIGHT_NEWTONS, BAND_PCT, 0usize),
            (0.0, BAND_PCT, 1200),
            (WEIGHT_NEWTONS, -1.0, 1200),
            (f64::NAN, BAND_PCT, 1200),
        ] {
            assert_eq!(
                first_sustained_band_entry(&force, 200, weight, band, dwell),
                StabilisationOutcome::Unsearchable,
                "weight {weight} band {band} dwell {dwell}"
            );
        }
    }
}
