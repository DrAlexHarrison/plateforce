//! `takeoff.threshold.longest_run`: the longest run below the threshold, wherever it sits.

use plateforce_core::takeoff::{takeoff_longest_run, ShortRunHandling};
use plateforce_core::Trial;

use crate::resolution::{Resolution, RuleRefusal};

pub(crate) fn crossing(
    trial: &Trial,
    threshold_newtons: f64,
    resolved: &mut Resolution,
    warnings: &mut Vec<String>,
) -> Result<usize, RuleRefusal> {
    let rate = trial.sample_rate_hz();
    // The operator this rule binds by being chosen. It prefers a later run than the first
    // flight phase on 155 of 244 trials, so which of the two ran is recorded rather than
    // left implicit in which function ran, and asking this rule for the first run is
    // refused rather than dropped.
    resolved.entailed(
        super::TAKEOFF_OP_CROSSING_SELECTION,
        "selection",
        "longest_run",
    )?;
    super::record_search_floor_at_trial_start(trial, resolved);
    let minimum_flight_samples = resolved
        .milliseconds_as_samples("persistence_ms", 0.0, rate)
        .max(1);
    let comparison = resolved.residual_comparison()?;
    let handling = resolved.enumerated(
        "short_run_handling",
        "rank_then_filter",
        &[
            ("rank_then_filter", ShortRunHandling::RankThenFilter),
            ("filter_then_rank", ShortRunHandling::FilterThenRank),
        ],
    )?;
    takeoff_longest_run(
        trial.force(),
        threshold_newtons,
        minimum_flight_samples,
        comparison,
        handling,
        rate,
    )
    .map(|selection| {
        // On an untrimmed recording the longest low-force run is often the
        // athlete standing off the plate. Two tools report nothing when it is.
        if !selection.selected_is_first_qualifying {
            warnings.push(format!(
                "the longest-run rule skipped past the first of {} qualifying flight phases, which on an untrimmed recording places takeoff after the athlete has already landed",
                selection.qualifying_run_count
            ));
        }
        selection.start_index
    })
    .map_err(RuleRefusal::Trial)
}
