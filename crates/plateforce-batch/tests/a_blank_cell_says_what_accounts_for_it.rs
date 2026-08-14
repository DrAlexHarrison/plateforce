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
//!
//! The ninth trace is the damaged one, and it is here because everything above it is a
//! recording on which every landmark places. This file asserted that every valueless row
//! carries a rule and an account, and it was green while a recording that places no onset wrote
//! eight of its eleven rows blank in both columns. A guard whose population cannot reach the
//! interesting case is a guard that cannot fail, and adding that recording is what would have
//! caught it.
//!
//! Two of those eight blanks stay blank on purpose, and they are asserted as a count rather
//! than as an absence. `system_weight_newtons` and the mass beside it are computed over the
//! weighing window that holds the recording's unreadable samples, so the arithmetic ran and
//! produced something that is not a number. Nothing wrote a sentence about that, and a sentence
//! written here would be this crate's own rather than a producer's.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::MethodChoice;
use plateforce_batch::{analyse, BatchRequest, TrialIdentity, TrialSet};

mod common;

/// The two quantities on the damaged recording whose arithmetic ran and produced no number.
///
/// Named rather than counted, because the count alone would go on passing if one of them came
/// back and something else fell out.
const PRODUCED_NO_NUMBER: [&str; 2] = ["system_weight_newtons", "system_mass_kilograms"];

const DAMAGED_TRIAL: &str = "subject01_trial1_interrupted";

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
    let committed = common::copy_committed_fixtures(&directory);
    let damaged = common::copy_damaged_recording(&directory);
    assert_eq!(
        committed + damaged,
        9,
        "the population is the eight committed traces and the damaged one"
    );
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

/// The same walk over a folder holding one recording the reader cannot analyse.
///
/// A file that could not be read reached no rule, so its refusal names no column and its row
/// carries the code for all of them. Its own directory, so the counts every other guard in this
/// file takes are over the traces those guards are about.
fn a_run_over_a_recording_that_will_not_read() -> plateforce_batch::BatchResult {
    let directory = common::tempdir("blank-cells-unreadable");
    common::copy_committed_fixtures(&directory);
    std::fs::write(
        directory.join("subject01_trial9.force.txt"),
        "the plate wrote nothing anybody can read\n",
    )
    .expect("the unreadable trace is written");
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

/// A run bound to the trio that reads a step-off as the start of the jump.
///
/// The condition that suppresses quantities without any rule declining, which is the state the
/// refusal relation cannot reach: on this recording five columns come back empty with zero
/// refusals in the run.
fn a_run_whose_landmarks_come_back_out_of_order() -> plateforce_batch::BatchResult {
    let directory = common::tempdir("blank-cells-inverted");
    let source = std::path::Path::new(common::FIXTURES)
        .join("synthetic_untrimmed_step_off_after_jump.force.txt");
    std::fs::copy(&source, directory.join("subject01_trial1.force.txt"))
        .expect("the recording copies");

    let mut request = common::analysis_request(1.0);
    request.weighing = plateforce_analysis::WeighingChoice {
        method_id: "bwepoch.adaptive_lowest_variance".to_string(),
        parameters: BTreeMap::from([("window_seconds".to_string(), 1.0)]),
        ..Default::default()
    };
    request.onset = MethodChoice {
        method_id: "onset.threshold.adaptive_trailing_window".to_string(),
        parameters: BTreeMap::from([("k".to_string(), 5.0)]),
        ..Default::default()
    };
    request.takeoff = MethodChoice {
        method_id: "takeoff.threshold.flight_noise_k_sd".to_string(),
        ..Default::default()
    };
    request.reading(&common::registry());
    let request =
        BatchRequest::new(request).resolving(&["system_weight", "movement_onset", "takeoff"]);

    let set = TrialSet::walk(
        &directory,
        &common::committed_format(),
        &TrialIdentity::FileStem,
    )
    .expect("the recording walks");
    let result = analyse(&set, &request, &common::registry()).expect("the run is bound");
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
    let smallest_real_jump = answered.iter().copied().fold(f64::INFINITY, f64::min);
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
///
/// The damaged recording answers it too, and that is the recording rather than a fault. It is
/// `subject01_trial1` with unreadable samples in its weighing window, so the landing the
/// original carries is still in the trace and the rule measuring to the return to the plate
/// still reaches it. What that recording loses is the system weight, which is why every
/// quantity measured from a net force is absent on it while the two measured between two
/// instants are not.
#[test]
fn flight_time_is_answered_by_the_recordings_that_carry_a_landing_and_by_no_other() {
    let result = run();
    let answered: Vec<&str> = result
        .results
        .iter()
        .filter(|row| row.values.get(FLIGHT_TIME).copied().flatten().is_some())
        .map(|row| row.trial_id.as_str())
        .collect();
    println!(
        "flight time on {} of {}",
        answered.len(),
        result.results.len()
    );
    assert_eq!(
        answered,
        vec![
            "subject01_trial1",
            DAMAGED_TRIAL,
            "synthetic_untrimmed_step_off",
            "synthetic_untrimmed_step_off_after_jump",
        ],
        "the recordings that stop in flight are the ones that carry no landing"
    );
}

/// The property this file is named for. Every blank cell is reachable from the row that
/// explains it, mechanically, without a reader knowing which construct fills which column.
///
/// With one carve-out, stated as a population rather than as an absence. Two quantities on the
/// damaged recording are blank because their arithmetic ran and produced something that is not
/// a number, which no rule declined over and no refusal row can name. They are asserted by name
/// and by count: a build that stopped naming a third column would redden here, and a build that
/// named none of them would too. Asserting instead that no cell goes unnamed would have to be
/// satisfied by inventing a refusal for a state nobody refused.
#[test]
fn every_blank_cell_is_named_by_a_refusal_row_the_result_row_points_at() {
    let result = run();
    let mut blanks = 0usize;
    let mut unnamed_total = 0usize;

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
        let unnamed: BTreeSet<&str> = absent
            .iter()
            .copied()
            .filter(|key| !named.contains(key))
            .collect();
        let produced_no_number: BTreeSet<&str> = if row.trial_id == DAMAGED_TRIAL {
            PRODUCED_NO_NUMBER.into_iter().collect()
        } else {
            BTreeSet::new()
        };
        assert_eq!(
            unnamed, produced_no_number,
            "{} leaves a column blank that no refusal row names and whose arithmetic did produce \
             a number, or names one whose arithmetic did not",
            row.trial_id
        );
        unnamed_total += unnamed.len();
    }

    assert!(
        blanks > 0,
        "no cell came back blank, so this guard looked at nothing"
    );
    println!("{blanks} blank cells, {unnamed_total} of them in a state no rule declined over");
    assert_eq!(
        unnamed_total,
        PRODUCED_NO_NUMBER.len(),
        "the population whose arithmetic produced no number changed size"
    );
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
    println!(
        "population {} of {}",
        population.len(),
        result.results.len()
    );
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

/// Every row with no number says why, in the sentence the producer that owns that state already
/// wrote. A row present and silent is the same absence the missing row was.
///
/// Two populations, each with its own count. A quantity a rule on its chain declined over
/// carries that rule's refusal. Two quantities on the damaged recording carry nothing, because
/// their arithmetic ran and produced something that is not a number, and no producer owns that
/// state: a sentence there would be one this crate composed about a fact nobody recorded. Every
/// row names the rule at the root of its own chain either way, so a reader holding a blank cell
/// can always see which rule was asked.
#[test]
fn a_row_with_no_number_carries_the_account_of_its_absence() {
    let result = run();
    let mut accounted = 0usize;
    let mut silent: BTreeSet<(&str, &str)> = BTreeSet::new();
    for row in &result.descriptions {
        if row.value.is_some() {
            continue;
        }
        assert!(
            !row.method_id.is_empty(),
            "{} {} has no number and names no rule",
            row.trial_id,
            row.quantity
        );
        if row.account.is_empty() {
            silent.insert((&row.trial_id, &row.quantity));
        } else {
            accounted += 1;
        }
    }

    println!(
        "{accounted} rows carry an account and no number, {} carry neither",
        silent.len()
    );
    assert!(
        accounted > 0,
        "no row came back with an account and no number"
    );
    let expected: BTreeSet<(&str, &str)> = PRODUCED_NO_NUMBER
        .into_iter()
        .map(|key| (DAMAGED_TRIAL, key))
        .collect();
    assert_eq!(
        silent, expected,
        "a row carries neither a number nor an account and is not one of the two whose \
         arithmetic produced no number"
    );
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
    assert!(
        !sentences.is_empty(),
        "no refusal accounts for the flight time"
    );
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

/// A refusal names exactly the columns that came back empty because of it.
///
/// The columns are the quantities whose own chain names the declining rule and which have no
/// number, read off the trial that declined rather than off a table filed beside the binding
/// rows. That table answered for the three rules that place the landmarks with an empty list,
/// which is why eight rows on the damaged recording were blank in both columns while this file
/// was green.
///
/// Both directions, because a refusal naming one blank of six reads exactly like one naming all
/// six, and a refusal naming a column that carries a number tells a reader that a number in
/// front of them is absent.
#[test]
fn a_refusal_names_exactly_the_columns_that_came_back_empty_because_of_it() {
    let result = run();
    let row = result
        .results
        .iter()
        .find(|row| row.trial_id == DAMAGED_TRIAL)
        .expect("the damaged recording is in the population");

    let empty: BTreeSet<&str> = result
        .quantities
        .iter()
        .filter(|key| row.values.get(*key).copied().flatten().is_none())
        .map(String::as_str)
        .collect();
    let answered: BTreeSet<&str> = result
        .quantities
        .iter()
        .filter(|key| row.values.get(*key).copied().flatten().is_some())
        .map(String::as_str)
        .collect();
    assert_eq!(empty.len(), 8, "{empty:?}");
    assert_eq!(answered.len(), 3, "{answered:?}");

    let named: BTreeSet<&str> = result
        .refusals
        .iter()
        .filter(|refusal| refusal.trial_id == DAMAGED_TRIAL)
        .flat_map(|refusal| refusal.quantity.split(','))
        .filter(|key| !key.is_empty())
        .collect();
    println!("{} columns named of {} empty", named.len(), empty.len());
    assert!(
        named.intersection(&answered).count() == 0,
        "a refusal claims a column that carries a number: {:?}",
        named.intersection(&answered).collect::<Vec<&&str>>()
    );
    assert!(
        named.is_subset(&empty),
        "a refusal claims a column outside the ones that came back empty: {named:?}"
    );
    let unclaimed: BTreeSet<&str> = empty.difference(&named).copied().collect();
    assert_eq!(
        unclaimed,
        PRODUCED_NO_NUMBER.into_iter().collect::<BTreeSet<&str>>(),
        "the columns no refusal claims are not the two whose arithmetic produced no number"
    );
}

/// The control on the guard above, and it is the one that can pass by naming everything.
///
/// A trial where nothing declined has no columns to claim. A build writing every quantity into
/// every refusal row would satisfy the assertions above completely and fail here.
#[test]
fn a_trial_where_nothing_declined_has_no_refusal_row_claiming_a_column() {
    let result = run();
    let answered_everything: Vec<&str> = result
        .results
        .iter()
        .filter(|row| {
            result
                .quantities
                .iter()
                .all(|key| row.values.get(key).copied().flatten().is_some())
        })
        .map(|row| row.trial_id.as_str())
        .collect();
    println!("{answered_everything:?} answer every column");
    assert!(
        answered_everything.contains(&"subject01_trial1"),
        "the recording that answers everything is no longer in the population, so this control \
         compares nothing"
    );

    for trial_id in &answered_everything {
        let claimed: Vec<&str> = result
            .refusals
            .iter()
            .filter(|refusal| refusal.trial_id == **trial_id)
            .flat_map(|refusal| refusal.quantity.split(','))
            .filter(|key| !key.is_empty())
            .collect();
        assert!(
            claimed.is_empty(),
            "{trial_id} answers every column and a refusal row claims {claimed:?}"
        );
    }
}

/// A refusal that reached no rule names no column.
///
/// The population is a file the reader put in the folder that the plate never wrote: it is
/// refused before any rule runs, so there is no chain to attribute anything to and every column
/// on the row is absent under the one code the row carries. A build deriving columns from the
/// id would have to invent them here.
#[test]
fn a_refusal_that_reached_no_rule_names_no_column() {
    let result = a_run_over_a_recording_that_will_not_read();
    let unreadable: Vec<&plateforce_batch::RefusalRow> = result
        .refusals
        .iter()
        .filter(|refusal| refusal.trial_id == "subject01_trial9")
        .collect();
    assert_eq!(unreadable.len(), 1, "{unreadable:#?}");
    let refusal = unreadable[0];
    println!("{} {}", refusal.code, refusal.message);
    assert!(refusal.method_id.is_empty(), "{}", refusal.method_id);
    assert_eq!(refusal.quantity, "");

    // And the row it belongs to carries the code for every column, so the reader is not left
    // with a blank cell pointing at nothing.
    let row = result
        .results
        .iter()
        .find(|row| row.trial_id == "subject01_trial9")
        .expect("the trial has a row");
    assert_eq!(row.refusal_code, refusal.code);
    assert!(
        row.values.values().all(Option::is_none),
        "the file did not read and a column carries a number"
    );

    // The control on the assertion above: rules that did run in the same walk do claim their
    // columns, so an empty cell here is this refusal's own state rather than a run that claims
    // nothing anywhere.
    let claimed_elsewhere = result
        .refusals
        .iter()
        .filter(|other| !other.quantity.is_empty())
        .count();
    assert!(
        claimed_elsewhere > 0,
        "no refusal in this run claims a column, so the empty cell above says nothing"
    );
}

/// The account under a blank cell and the refusal row that names that cell are one sentence.
///
/// Two relations written by two paths through the engine: the account comes from the one site
/// that writes accounts, the refusal row from the one writer for refusals. Compared whole
/// rather than by `contains`, because a sentence with anything added to it is a sentence
/// somewhere composed a second time, and one fact with two producers is what this whole product
/// exists against.
#[test]
fn the_account_under_a_blank_cell_is_the_refusal_rows_own_sentence() {
    let result = run();
    let mut compared = 0usize;
    for description in &result.descriptions {
        if description.value.is_some() || description.account.is_empty() {
            continue;
        }
        let naming: Vec<&plateforce_batch::RefusalRow> = result
            .refusals
            .iter()
            .filter(|refusal| {
                refusal.trial_id == description.trial_id
                    && refusal
                        .quantity
                        .split(',')
                        .any(|key| key == description.quantity)
            })
            .collect();
        assert_eq!(
            naming.len(),
            1,
            "{} {} is named by {} refusal rows",
            description.trial_id,
            description.quantity,
            naming.len()
        );
        assert_eq!(
            description.account, naming[0].message,
            "{} {} is accounted for twice, in two different sentences",
            description.trial_id, description.quantity
        );
        compared += 1;
    }
    println!("{compared} blank cells hold the sentence their refusal row holds");
    assert!(compared > 0, "no blank cell carried an account");
}

/// The account under a suppressed cell and the signal row that names it are one sentence.
///
/// The state no refusal reaches. On this recording one published onset rule reads the step off
/// the plate as the start of the jump, so five columns come back empty with **zero** refusals
/// in the run, and a build that reached only refusals leaves all five blank in both columns.
#[test]
fn the_account_under_a_suppressed_cell_is_the_signal_rows_own_remedy() {
    let result = a_run_whose_landmarks_come_back_out_of_order();
    assert!(
        result.refusals.is_empty(),
        "a rule declined, so this run is no longer the state that has no refusal in it: {:#?}",
        result.refusals
    );
    assert_eq!(result.signals.len(), 1, "{:#?}", result.signals);
    let signal = &result.signals[0];
    let qualifies: BTreeSet<&str> = signal
        .qualifies
        .split(',')
        .filter(|key| !key.is_empty())
        .collect();
    assert_eq!(qualifies.len(), 5, "{qualifies:?}");

    let empty: BTreeSet<&str> = result
        .descriptions
        .iter()
        .filter(|row| row.value.is_none())
        .map(|row| row.quantity.as_str())
        .collect();
    assert_eq!(
        empty, qualifies,
        "a column came back empty that the signal does not account for, or the signal names a \
         column carrying a number"
    );

    for row in result.descriptions.iter().filter(|row| row.value.is_none()) {
        assert_eq!(
            row.account, signal.remedy,
            "{} is accounted for in a sentence the signal did not write",
            row.quantity
        );
    }
    println!(
        "{} suppressed cells hold the remedy their signal row holds",
        empty.len()
    );
}

/// A trial that computed writes one row per quantity the run asked for, with no exceptions.
///
/// The reconciliation that fills in a trial which produced nothing rests entirely on this: a
/// computed trial's key set is settled by the request rather than by the recording, so the
/// engine's own answer is already complete and there is nothing left for a second writer to
/// fill. Were that to stop holding, the missing rows would come back and something would have
/// to fill them, which is the two producers this whole change exists to end.
///
/// The denominator is the run's own column set, and the column set is held to what the rules
/// this request bound declare, read off the binding rows. Break it by making a rule report
/// fewer keys than its row declares and the row count falls.
#[test]
fn every_trial_that_computed_writes_one_row_per_quantity_the_run_asked_for() {
    let result = run();
    let computed: Vec<&plateforce_batch::ResultRow> = result
        .results
        .iter()
        .filter(|row| !row.provenance_id.is_empty())
        .collect();
    println!(
        "{} of {} trials computed, {} quantities asked for",
        computed.len(),
        result.results.len(),
        result.quantities.len()
    );
    assert_eq!(
        computed.len(),
        result.results.len(),
        "a trial produced nothing, so this guard is looking at a population it was not written \
         for"
    );

    for row in &computed {
        let mine: BTreeSet<&str> = result
            .descriptions
            .iter()
            .filter(|description| description.trial_id == row.trial_id)
            .map(|description| description.quantity.as_str())
            .collect();
        let asked: BTreeSet<&str> = result.quantities.iter().map(String::as_str).collect();
        assert_eq!(
            mine, asked,
            "{} writes a different set of rows from the set of quantities the run asked for",
            row.trial_id
        );
    }

    // And the column set is what the bound rules declare, rather than what this run happened to
    // produce. Read off the binding table so a rule added to the build widens the check on the
    // day it is added.
    let declared: BTreeSet<&str> = plateforce_analysis::BINDINGS
        .iter()
        .filter(|binding| {
            [
                "bwepoch.fixed_window",
                "onset.threshold.noise_relative",
                "takeoff.threshold.flight_noise_k_sd",
            ]
            .contains(&binding.id)
        })
        .flat_map(|binding| binding.quantities.iter().map(|quantity| quantity.key))
        .collect();
    let asked: BTreeSet<&str> = result.quantities.iter().map(String::as_str).collect();
    println!(
        "{} keys declared by the three rules this run named",
        declared.len()
    );
    assert!(
        declared.is_subset(&asked),
        "a rule this run bound declares a column the run does not carry: {:?}",
        declared.difference(&asked).collect::<Vec<&&str>>()
    );
}

/// A rule that declined names the columns its refusal accounts for.
///
/// The property `derive::quantities_of_rule`'s first test held, asserted against a real
/// response rather than against a static table read with no trial in hand. That is why the
/// original could not reach the damaged recording: it never ran one.
///
/// Two exact ids, each returning its own column on the same run, which is the control that has
/// to live inside this test. A build whose lookup came back empty for everything would satisfy
/// any assertion phrased as one id naming one column, because an empty list contains no wrong
/// column either.
#[test]
fn a_declining_rule_names_the_columns_its_refusal_accounts_for() {
    let result = run();
    let columns_of = |method_id: &str| -> BTreeSet<String> {
        result
            .refusals
            .iter()
            .filter(|refusal| refusal.method_id == method_id)
            .flat_map(|refusal| refusal.quantity.split(','))
            .filter(|key| !key.is_empty())
            .map(str::to_string)
            .collect()
    };

    for (method_id, column) in [
        ("flight_time.takeoff_to_touchdown", "flight_time_seconds"),
        (
            "jumpheight.takeoff.flight_time",
            "jump_height_from_flight_time_meters",
        ),
    ] {
        let named = columns_of(method_id);
        println!("{method_id} accounts for {named:?}");
        assert_eq!(
            named,
            BTreeSet::from([column.to_string()]),
            "{method_id} names a set of columns other than the one it reports"
        );
    }

    // And a name no rule in this build carries claims nothing, on the same response, so the
    // two above are a lookup that discriminates rather than one that answers every id alike.
    for absent in ["jumpheight.takeoff", "flight_time", "not.a.rule", ""] {
        assert!(
            columns_of(absent).is_empty(),
            "{absent} claims {:?}",
            columns_of(absent)
        );
    }
}

/// A trial that produced nothing still writes a row for every quantity the run asked for.
///
/// The branch of the fill-in that nothing can replace. The analysis returned an error, so there
/// is no response to inherit an account from, and no rule was reached: every column is absent
/// under one reason and this is the only place they can be named. Removing it costs a reader
/// eleven rows per refused trial, each of which carried the sentence the file reader itself
/// wrote about their recording.
///
/// The population is a file the plate never wrote, added because the committed fixtures are all
/// files that read. A guard whose population holds no unreadable file cannot reach a trial that
/// produced nothing, which is the same shape as a guard whose population holds no recording
/// where a landmark rule declines.
///
/// Both halves. The row count, so a branch that stopped filling in reddens, and the content, so
/// a branch that filled in with silence reddens too.
#[test]
fn a_trial_that_produced_nothing_still_writes_a_row_for_every_quantity() {
    let result = a_run_over_a_recording_that_will_not_read();
    let refused = "subject01_trial9";

    let produced_nothing: Vec<&plateforce_batch::ResultRow> = result
        .results
        .iter()
        .filter(|row| row.provenance_id.is_empty())
        .collect();
    assert_eq!(
        produced_nothing.len(),
        1,
        "the population no longer holds exactly one trial that produced nothing: {:?}",
        produced_nothing
            .iter()
            .map(|row| row.trial_id.as_str())
            .collect::<Vec<&str>>()
    );
    assert_eq!(produced_nothing[0].trial_id, refused);

    let mine: Vec<&plateforce_batch::DescriptionRow> = result
        .descriptions
        .iter()
        .filter(|row| row.trial_id == refused)
        .collect();
    println!(
        "{} rows for the trial that produced nothing, against {} quantities the run asked for, \
         {} rows in the table",
        mine.len(),
        result.quantities.len(),
        result.descriptions.len()
    );
    assert_eq!(
        mine.len(),
        result.quantities.len(),
        "a reader filtering this table by quantity is answered by the absence of the trial that \
         produced nothing"
    );
    assert_eq!(
        result.descriptions.len(),
        result.results.len() * result.quantities.len(),
        "the table is no longer one row per trial per quantity"
    );

    // The reader's own sentence about their recording, taken from the refusal row rather than
    // rebuilt, and no rule named because no rule was reached.
    let sentence = result
        .refusals
        .iter()
        .find(|refusal| refusal.trial_id == refused)
        .map(|refusal| refusal.message.clone())
        .expect("the file that would not read is refused");
    println!("{sentence}");
    for row in &mine {
        assert!(row.value.is_none(), "{} carries a number", row.quantity);
        assert_eq!(
            row.account, sentence,
            "{} is accounted for in a sentence the reader was not given",
            row.quantity
        );
        assert_eq!(
            row.method_id, "",
            "{} names a rule, and no rule was reached on this trial",
            row.quantity
        );
    }

    // The control: the trials that computed in the same run carry their own accounts rather
    // than this one, so the fill-in reaches the trial that produced nothing and no other.
    let borrowed: Vec<&str> = result
        .descriptions
        .iter()
        .filter(|row| row.trial_id != refused && row.account == sentence)
        .map(|row| row.trial_id.as_str())
        .collect();
    assert!(
        borrowed.is_empty(),
        "a trial that computed carries the unreadable file's sentence: {borrowed:?}"
    );
}
