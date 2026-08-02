//! Time to stabilisation over the committed subject-01 trials.
//!
//! Whether a trial can run this rule at all is a fact about the recording rather than about
//! the rule, so it is measured over every committed trial rather than assumed for one. A
//! recording that stops during flight holds no landing to settle after, and a recording that
//! stops during the dwell holds no dwell to complete. Both are answers.

use plateforce_analysis::run;
use plateforce_core::stabilisation::{first_sustained_band_entry, StabilisationOutcome};

use crate::common::{committed_trial, default_request, COMMITTED_TRIALS, CORPUS_SAMPLE_RATE_HZ};

/// Hawkin's published band and dwell, which the registry entry carries as its defaults.
const BAND_PCT: f64 = 5.0;
const DWELL_SECONDS: f64 = 1.0;

/// Every trial gets one definite answer, and none gets a time clipped to the end of its
/// recording. A trace with no landing, one that stops inside the dwell and one that never
/// settles are three different facts about the recording and are reported as three.
#[test]
fn every_committed_trial_settles_or_says_what_the_recording_lacks() {
    let dwell_samples = (DWELL_SECONDS * CORPUS_SAMPLE_RATE_HZ).round() as usize;

    let mut stabilised = 0usize;
    let mut no_landing_recorded = 0usize;
    let mut ended_inside_the_dwell = 0usize;
    let mut never_settled = 0usize;
    let mut could_not_check = Vec::new();

    for name in COMMITTED_TRIALS {
        let trial = committed_trial(name);
        let response = run(&trial, &default_request())
            .unwrap_or_else(|error| panic!("{name} did not run: {error}"));

        let Some(touchdown) = response.touchdown_index else {
            no_landing_recorded += 1;
            println!("{name}: the recording ends during flight");
            continue;
        };

        let outcome = first_sustained_band_entry(
            trial.force(),
            touchdown,
            response.levels.system_weight_newtons,
            BAND_PCT,
            dwell_samples,
        );
        match outcome {
            StabilisationOutcome::Stabilised(found) => {
                stabilised += 1;
                assert!(
                    found.dwell_completed_index < trial.len(),
                    "{name} reports a dwell completing past the end of its recording"
                );
                assert_eq!(
                    found.dwell_completed_index - found.entered_band_index + 1,
                    dwell_samples,
                    "{name} reports a dwell that is not the dwell that was asked for"
                );
            }
            StabilisationOutcome::TraceShorterThanDwell { .. } => ended_inside_the_dwell += 1,
            StabilisationOutcome::NeverSustained { .. } => never_settled += 1,
            StabilisationOutcome::Unsearchable => {
                could_not_check.push(format!("{name} could not be searched"))
            }
        }
        println!("{name}: {outcome:?}");
    }

    assert!(
        could_not_check.is_empty(),
        "{} of {} trials could not be examined, which is not the same as not settling:\n  {}",
        could_not_check.len(),
        COMMITTED_TRIALS.len(),
        could_not_check.join("\n  ")
    );
    assert_eq!(
        stabilised + no_landing_recorded + ended_inside_the_dwell + never_settled,
        COMMITTED_TRIALS.len(),
        "the tally does not cover every committed trial"
    );
    println!(
        "of {} committed trials: stabilised {stabilised}, no landing recorded \
         {no_landing_recorded}, recording ended inside the dwell {ended_inside_the_dwell}, never \
         settled {never_settled}",
        COMMITTED_TRIALS.len()
    );
}
