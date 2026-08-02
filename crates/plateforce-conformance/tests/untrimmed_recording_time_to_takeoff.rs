//! `MISSION.md` P5 names one defect as the pillar's own test: an untrimmed recording that
//! returns a time to takeoff of a few tens of milliseconds, with no warning.
//!
//! The test reproduces the defect before it closes it. A test that only asserted the fixed
//! behaviour could not tell a fix from a fixture that never had the defect, and this project
//! has shipped that mistake before.
//!
//! The fixture is synthetic and says so in its name. Every corpus recording is trimmed:
//! surveyed across the 242 trials the shipped rules place a takeoff on, 0 return a time to
//! takeoff under 150 ms, so the corpus cannot carry this case, and of the corpus only
//! subject 01 could be published if it did.

use plateforce_core::onset::{
    backtrack, onset_noise_relative, BandSides, CrossingSearch, CrossingSelection,
    DegenerateBandPolicy,
};
use plateforce_core::takeoff::landing_shape::{takeoff_by_landing_shape, LandingShapeSpec};
use plateforce_core::takeoff::{
    takeoff_first_sustained_run, takeoff_longest_run, ResidualComparison, ShortRunHandling,
};
use plateforce_core::trial::CentralTendency;
use plateforce_core::{read_trial_from_path, DispersionEstimator, Trial, WeighingEpoch};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const THRESHOLD_NEWTONS: f64 = 20.0;
const PERSISTENCE_SAMPLES: usize = 36;
const FIXTURE: &str = "synthetic_untrimmed_step_off.force.txt";

/// An athlete standing still, stepping off the plate and back on, and then jumping. The
/// step-off is the low-force run the shipped rules mistake for flight.
fn untrimmed_trial() -> (Trial, WeighingEpoch, usize) {
    let path = format!("{}/fixtures/{FIXTURE}", env!("CARGO_MANIFEST_DIR"));
    let (trial, _report) =
        read_trial_from_path(&path, ',', 0, SAMPLE_RATE_HZ).expect("the committed fixture");
    let epoch = WeighingEpoch::fixed_window(
        &trial,
        1.0,
        CentralTendency::Mean,
        DispersionEstimator::Sample,
    )
    .expect("a second of quiet standing opens the recording");
    let search = CrossingSearch {
        start_index: epoch.end_index,
        end_index: trial.len(),
        persistence_samples: PERSISTENCE_SAMPLES,
        selection: CrossingSelection::First,
    };
    let crossing = onset_noise_relative(
        trial.force(),
        epoch.system_weight_newtons,
        epoch.standard_deviation_newtons,
        5.0,
        BandSides::BelowOnly,
        DegenerateBandPolicy::Refuse,
        &search,
        SAMPLE_RATE_HZ,
    )
    .expect("onset fires on the step off the plate");
    let onset = backtrack(crossing, PERSISTENCE_SAMPLES).index;
    (trial, epoch, onset)
}

fn milliseconds_between(onset: usize, takeoff: usize) -> f64 {
    (takeoff as f64 - onset as f64) / SAMPLE_RATE_HZ * 1000.0
}

#[test]
fn the_untrimmed_recording_stops_returning_a_step_off_time_to_takeoff() {
    let (trial, epoch, onset) = untrimmed_trial();
    let force = trial.force();

    // The defect, under both rules the software ships. Each places takeoff on the step off
    // the plate, which is the first and, at 0.608 s, also the longest low-force run before
    // the jump's own 0.451 s of flight.
    let first_run = takeoff_first_sustained_run(
        force,
        THRESHOLD_NEWTONS,
        1,
        ResidualComparison::SignedValue,
        epoch.end_index,
        SAMPLE_RATE_HZ,
    )
    .expect("the first-run rule places a takeoff");
    let longest = takeoff_longest_run(
        force,
        THRESHOLD_NEWTONS,
        1,
        ResidualComparison::SignedValue,
        ShortRunHandling::RankThenFilter,
        SAMPLE_RATE_HZ,
    )
    .expect("the longest-run rule places a takeoff");

    let shipped_milliseconds = milliseconds_between(onset, first_run);
    println!("{FIXTURE}");
    println!("  takeoff.threshold.absolute_force  time to takeoff {shipped_milliseconds:.1} ms");
    println!(
        "  takeoff.threshold.longest_run     time to takeoff {:.1} ms",
        milliseconds_between(onset, longest.start_index)
    );

    assert_eq!(
        first_run, longest.start_index,
        "both shipped rules land on the step off the plate"
    );
    assert!(
        (shipped_milliseconds - 58.3).abs() < 0.1,
        "the defect this pillar names, measured on this fixture: {shipped_milliseconds:.1} ms"
    );
    // The one warning the software carries for a misplaced flight phase does not fire,
    // because the step-off is the first qualifying run as well as the longest. That silence
    // is the half of P5's sentence reading "with no warning".
    assert!(
        longest.selected_is_first_qualifying,
        "nothing tells the user the takeoff sits on a step-off"
    );

    // The closure. The rule reads what force did on the way back up: the step-on rises at
    // 6.5 bodyweights per second to 1.00 bodyweights, and the landing at 160.9 to 4.00.
    let (placed, landings) = takeoff_by_landing_shape(
        force,
        epoch.system_weight_newtons,
        THRESHOLD_NEWTONS,
        SAMPLE_RATE_HZ,
        &LandingShapeSpec::default(),
    );
    let placed = placed.expect("the recording closes one run with a landing");
    let closed_milliseconds = milliseconds_between(onset, placed);
    println!("  takeoff.threshold.landing_shape   time to takeoff {closed_milliseconds:.1} ms");
    println!("  landings found: {landings}");

    assert_eq!(landings, 1, "one jump in the recording, one landing");
    assert!(
        (closed_milliseconds - 2269.2).abs() < 0.1,
        "the jump's own time to takeoff: {closed_milliseconds:.1} ms"
    );
    assert!(
        closed_milliseconds > shipped_milliseconds * 10.0,
        "the two answers differ by more than an order of magnitude, which is the defect"
    );
}
