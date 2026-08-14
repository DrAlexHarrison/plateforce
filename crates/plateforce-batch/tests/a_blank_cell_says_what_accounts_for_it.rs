//! Every cell a folder run leaves blank is joined to the reason it is blank.
//!
//! The run is bound to `takeoff.threshold.flight_noise_k_sd`, which is what the shipped Sams
//! preset resolves takeoff to and what the rest of this suite does not exercise: every other
//! test here binds a fixed 20 N threshold, which no plate's noise floor reaches, so the
//! failing case sat outside the whole guard population.
//!
//! That rule re-estimates the threshold per trial from the flight-phase noise. On two of the
//! six shipped fixtures it settles below 1.398 N, which is the step this plate's converter
//! quantises at, so every sample the plate can report as nonzero clears it. The first step of
//! dither three samples after takeoff then reads as the athlete returning to the plate, and
//! the table carried a flight time of 0.0025 s and a height of 0.0000076 m beside four real
//! heights of about 0.42 m.
//!
//! The run walks every committed trace, which is the six subject recordings plus the two
//! synthetics built to carry a landing and a step off the plate after it. Five of the six
//! subject recordings stop while the athlete is still in the air, which is a measured property
//! of this corpus rather than of the software, so five rows here carry blank cells and three
//! carry none. The two synthetics are the positive control: a confirmation set too high would
//! refuse every recording, and that reads exactly like a confirmation that works until the
//! recordings that do land are named as well.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::MethodChoice;
use plateforce_batch::{analyse, BatchRequest, TrialIdentity, TrialSet};

mod common;

const FLIGHT_TIME: &str = "flight_time_seconds";
const FLIGHT_TIME_HEIGHT: &str = "jump_height_from_flight_time_meters";

/// The height every trial in this corpus answers, which is what makes the flight-time column
/// beside it readable as a jump rather than as a failure of the recording.
const IMPULSE_HEIGHT: &str = "jump_height_from_takeoff_meters";

/// A run bound the way the shipped preset binds takeoff.
fn a_run_under_the_flight_noise_threshold() -> BatchRequest {
    let mut request = common::analysis_request(1.0);
    request.takeoff = MethodChoice {
        method_id: "takeoff.threshold.flight_noise_k_sd".to_string(),
        parameters: BTreeMap::from([("k".to_string(), 5.0)]),
        options: BTreeMap::from([(
            "flight_window".to_string(),
            "middle_fraction_of_flight".to_string(),
        )]),
        ..Default::default()
    };
    request.reading(&common::registry());
    BatchRequest::new(request).resolving(&["system_weight", "movement_onset", "takeoff"])
}

fn run() -> plateforce_batch::BatchResult {
    let directory = common::tempdir("blank-cells");
    common::copy_committed_fixtures(&directory);
    let set = TrialSet::walk(
        &directory,
        &common::committed_format(),
        &TrialIdentity::FileStem,
    )
    .expect("the fixtures walk");
    let result = analyse(
        &set,
        &a_run_under_the_flight_noise_threshold(),
        &common::registry(),
    )
    .expect("the run is bound");
    std::fs::remove_dir_all(&directory).ok();
    result
}

/// A flight time of three samples and the height taken from it.
///
/// Held against the trials that answer rather than against a bare floor: the smallest real
/// jump in this corpus is about 0.41 m, so a height two orders of magnitude under the
/// smallest answered height on the same run is not a jump anybody performed. A fixed epsilon
/// would pass a plate whose dither happens to be larger.
#[test]
fn no_height_is_written_from_a_step_of_the_plates_own_dither() {
    let result = run();
    let answered: Vec<f64> = result
        .results
        .iter()
        .filter_map(|row| row.values.get(IMPULSE_HEIGHT).copied().flatten())
        .collect();
    let smallest_real_jump = answered
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    assert!(
        smallest_real_jump.is_finite(),
        "the run answered no height at all, so this proves nothing: {answered:?}"
    );

    for row in &result.results {
        let Some(height) = row.values.get(FLIGHT_TIME_HEIGHT).copied().flatten() else {
            continue;
        };
        println!("{} {FLIGHT_TIME_HEIGHT} {height}", row.trial_id);
        assert!(
            height > smallest_real_jump / 100.0,
            "{} carries {height} m against a smallest answered height of {smallest_real_jump} m, \
             which is the plate's noise floor read as a jump",
            row.trial_id
        );
    }
}

/// The corpus's own denominator, stated rather than implied.
///
/// Five of the six subject recordings stop before the athlete comes back down, so the flight
/// time is answered by `subject01_trial1` and by the two synthetic recordings built to carry a
/// landing and a step off the plate after it. Both directions matter: a rule that refused
/// every recording would satisfy half of this and is what a confirmation set too high looks
/// like, so the recordings that do land are named as well as the ones that do not.
#[test]
fn flight_time_is_answered_by_the_recordings_that_carry_a_landing_and_by_no_other() {
    let result = run();
    let answered: Vec<&str> = result
        .results
        .iter()
        .filter(|row| row.values.get(FLIGHT_TIME).copied().flatten().is_some())
        .map(|row| row.trial_id.as_str())
        .collect();
    println!("flight time on {} of {}", answered.len(), result.results.len());
    assert_eq!(
        answered,
        vec![
            "subject01_trial1",
            "synthetic_untrimmed_step_off",
            "synthetic_untrimmed_step_off_after_jump",
        ],
        "the recordings that stop in flight are the ones that carry no landing"
    );
}

/// The property this file is named for. Every blank cell is reachable from the row that
/// explains it, mechanically, without a reader knowing which construct fills which column.
#[test]
fn every_blank_cell_is_named_by_a_refusal_row_the_result_row_points_at() {
    let result = run();
    let mut blanks = 0usize;

    for row in &result.results {
        let absent: Vec<&str> = result
            .quantities
            .iter()
            .filter(|key| row.values.get(*key).copied().flatten().is_none())
            .map(String::as_str)
            .collect();
        blanks += absent.len();
        if absent.is_empty() {
            assert!(
                row.refusal_code.is_empty(),
                "{} answers every column and still carries {}",
                row.trial_id,
                row.refusal_code
            );
            continue;
        }

        assert!(
            !row.refusal_code.is_empty(),
            "{} leaves {absent:?} blank and carries no code",
            row.trial_id
        );

        // The codes on the row are the codes of its own refusal rows, and the columns those
        // rows name cover every blank. Both directions, because a row naming one blank of two
        // reads exactly like a row naming both.
        let mine: Vec<&plateforce_batch::RefusalRow> = result
            .refusals
            .iter()
            .filter(|refusal| refusal.trial_id == row.trial_id)
            .collect();
        let codes: BTreeSet<&str> = mine.iter().map(|refusal| refusal.code.as_str()).collect();
        assert_eq!(
            row.refusal_code,
            codes.into_iter().collect::<Vec<&str>>().join(","),
            "{} names codes its own refusal rows do not",
            row.trial_id
        );

        let named: BTreeSet<&str> = mine
            .iter()
            .flat_map(|refusal| refusal.quantity.split(','))
            .filter(|key| !key.is_empty())
            .collect();
        for key in &absent {
            assert!(
                named.contains(key),
                "{} leaves {key} blank and no refusal row names that column: {named:?}",
                row.trial_id
            );
        }
    }

    assert!(
        blanks > 0,
        "no cell came back blank, so this guard looked at nothing"
    );
    println!("{blanks} blank cells, every one of them named");
}

/// The sentence a reader is shown and the file they open, held against each other.
#[test]
fn the_coverage_line_counts_the_codes_the_file_carries() {
    let result = run();
    let in_the_file = result
        .results
        .iter()
        .filter(|row| !row.refusal_code.is_empty())
        .count();
    let line = result.coverage.line();
    println!("{line}");
    assert_eq!(result.coverage.carrying_a_refusal_code, in_the_file);
    assert!(
        line.contains(&format!(
            "a refusal code on {in_the_file} of {}",
            result.results.len()
        )),
        "the line does not carry the count the file does: {line}"
    );
}

/// A trial that answered nine of eleven quantities is a member of the population every figure
/// over this run is taken over. Reading membership off `refusal_code`, which now carries the
/// codes for the two it declined, would have dropped five of six trials from every mean and
/// every reliability figure in the product.
#[test]
fn a_trial_that_declined_a_quantity_is_still_in_the_population() {
    let result = run();
    let population = result.population();
    println!("population {} of {}", population.len(), result.results.len());
    assert_eq!(population.len(), result.results.len());
    assert!(
        result
            .results
            .iter()
            .any(|row| !row.refusal_code.is_empty()),
        "no trial declined anything, so this guard looked at nothing"
    );
    for row in &result.results {
        assert!(
            population.contains(&row.trial_id),
            "{} answered {} quantities and left the population",
            row.trial_id,
            row.values.values().filter(|value| value.is_some()).count()
        );
    }
}

/// The long-form table is one row per trial per quantity, with no exceptions.
///
/// A rule that declined reported no metric, so the account writer had nothing to describe and
/// the row was absent rather than present and empty: five trials carried nine rows against
/// eleven, and a reader filtering by quantity got three of six trials with nothing saying the
/// other three existed.
#[test]
fn the_long_form_table_carries_a_row_for_every_trial_and_every_quantity() {
    let result = run();
    let expected = result.results.len() * result.quantities.len();
    println!(
        "{} description rows against {} trials by {} quantities",
        result.descriptions.len(),
        result.results.len(),
        result.quantities.len()
    );
    assert_eq!(result.descriptions.len(), expected);

    let keyed: BTreeSet<(&str, &str)> = result
        .descriptions
        .iter()
        .map(|row| (row.trial_id.as_str(), row.quantity.as_str()))
        .collect();
    assert_eq!(keyed.len(), expected, "a trial and quantity pair repeats");
    for row in &result.results {
        for quantity in &result.quantities {
            assert!(
                keyed.contains(&(row.trial_id.as_str(), quantity.as_str())),
                "{} has no row for {quantity}",
                row.trial_id
            );
        }
    }
}

/// Every filled-in row says why its number is absent, in the sentence the refusal already
/// generated. A row present and silent is the same absence the missing row was.
#[test]
fn a_row_with_no_number_carries_the_account_of_its_absence() {
    let result = run();
    let mut absent = 0usize;
    for row in &result.descriptions {
        if row.value.is_some() {
            continue;
        }
        absent += 1;
        assert!(
            !row.account.is_empty(),
            "{} {} has no number and no account",
            row.trial_id,
            row.quantity
        );
        assert!(
            !row.method_id.is_empty(),
            "{} {} has no number and names no rule",
            row.trial_id,
            row.quantity
        );
    }
    assert!(absent > 0, "no row came back without a number");
    println!("{absent} rows carry an account and no number");
}

/// The rows the walk wrote keep their number and their own account.
///
/// Filling in the absent rows must not rewrite the present ones, and a guard on the total
/// alone would pass on an implementation that replaced every account with one sentence.
#[test]
fn a_quantity_that_answered_keeps_the_account_the_engine_wrote() {
    let result = run();
    let answered: Vec<&plateforce_batch::DescriptionRow> = result
        .descriptions
        .iter()
        .filter(|row| row.value.is_some())
        .collect();
    println!("{} rows carry a number", answered.len());
    assert_eq!(
        answered.len(),
        result
            .results
            .iter()
            .map(|row| row.values.values().filter(|value| value.is_some()).count())
            .sum::<usize>(),
        "the rows carrying a number are the numbers the table carries"
    );
    for row in &answered {
        let value = row.value.expect("filtered to rows with a number");
        assert!(
            row.account
                .starts_with(&plateforce_analysis::recorded_number_text(value)),
            "{} {} opens with something other than its own value: {}",
            row.trial_id,
            row.quantity,
            row.account.lines().next().unwrap_or_default()
        );
    }
}

/// The refusal a reader meets, which has to be about their recording. Reported as a required
/// parameter nobody stated, it sent them to look up a landing index by hand on a recording
/// that holds no landing to look up.
#[test]
fn the_refusal_describes_the_recording_rather_than_an_unstated_parameter() {
    let result = run();
    let sentences: Vec<&str> = result
        .refusals
        .iter()
        .filter(|refusal| refusal.quantity.split(',').any(|key| key == FLIGHT_TIME))
        .map(|refusal| refusal.message.as_str())
        .collect();
    assert!(!sentences.is_empty(), "no refusal accounts for the flight time");
    for sentence in &sentences {
        println!("{sentence}");
        assert!(
            sentence.contains("carries no landing"),
            "the refusal does not say what is missing from the recording: {sentence}"
        );
        assert!(
            !sentence.contains("has to be stated"),
            "the refusal asks for a parameter rather than naming the recording: {sentence}"
        );
    }
}
