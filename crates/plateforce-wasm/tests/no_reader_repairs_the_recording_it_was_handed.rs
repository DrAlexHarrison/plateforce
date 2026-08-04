//! Every reader hands the engine the trace the file wrote, and can tell a damaged recording
//! from a clean one.
//!
//! Two readers in this repository have rewritten a trace before anything read it, and each
//! was found by asking a surface a question the others were already being asked. The tab held
//! every sample it could not read at the last real reading, so it answered an interrupted
//! recording with the intact trial's numbers to the last digit and refused nothing. The batch
//! reader removed the samples instead, which closed the gap and shifted every timestamp after
//! it. Neither reader was wrong about the counts it published; both were wrong about the trace.
//!
//! So the property held here is about the trace and not about a count. The samples in question
//! are exactly the ones a value comparison cannot see, because a NaN never equals itself, and
//! the bit patterns are what a reader that repaired one would move.
//!
//! `plateforce_core::signal::trial_from_column` is the one home the policy has. These two
//! readers are the ones that used to spell it themselves.

use plateforce_batch::{SourceFormat, TrialIdentity, TrialSet};
use plateforce_core::signal::{reported_samples, trial_from_column, Sentinel};
use plateforce_wasm::{ForceFile, LoadedTrial};

const INTERRUPTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/damaged/subject01_trial1_interrupted.force.txt"
);
const INTACT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
);

const SAMPLE_RATE_HZ: f64 = 1200.0;
const ROWS_IN_THE_RECORDING: usize = 6000;
/// Three samples of the quiet stance carry no number. The zero convention matches 157 more,
/// every one of them an athlete in the air.
const CARRYING_NO_NUMBER: usize = 3;
const MATCHING_THE_ZERO_CONVENTION: usize = 157;

/// The request the parity gate asks of this recording, in the form the tab posts it.
const REQUEST: &str = r#"{
  "weighing": { "method_id": "bwepoch.fixed_window", "parameters": { "duration": 1.0 } },
  "onset": { "method_id": "onset.threshold.noise_relative", "parameters": { "k": 5.0 } },
  "takeoff": { "method_id": "takeoff.threshold.absolute_force", "parameters": { "threshold_n": 20.0 } }
}"#;

/// What the file holds, read without going through any surface, so the comparisons below have
/// something to be held against that no reader under test produced.
fn as_the_file_wrote_it(path: &str) -> Vec<f64> {
    std::fs::read_to_string(path)
        .expect("the committed recording is readable")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().parse::<f64>().expect("one value per line"))
        .collect()
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}

/// The population the parity gate compares, read from the record it holds the four surfaces
/// to rather than listed here. A list written here would narrow the moment the gate widened,
/// and a projection over a narrower population is how a comparison stops seeing the answer.
fn fields_the_gate_compares() -> Vec<String> {
    let committed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/result-parity-interrupted.json"
        ))
        .expect("the committed record for this recording is readable"),
    )
    .expect("the committed record parses");
    let fields: Vec<String> = committed["compared_fields"]
        .as_array()
        .expect("the record names the fields it compares")
        .iter()
        .map(|field| field.as_str().expect("a field name").to_string())
        .collect();
    assert!(!fields.is_empty(), "the record names no compared field");
    fields
}

fn tab_answer(path: &str, convention: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("the committed recording is readable");
    let file = ForceFile::parse_text(&text).expect("one value per line parses");
    let loaded = LoadedTrial::from_force_file(&file, 0, SAMPLE_RATE_HZ, convention)
        .expect("the column binds");
    let document = loaded
        .analyse(REQUEST, Some(path.to_string()), None)
        .expect("the tab answers");
    serde_json::from_str(&document).expect("the tab's answer parses")
}

/// The tab declines the landmark on a recording whose weighing window carries no number, and
/// publishes no number that rested on it.
///
/// This is the state the other three surfaces were already in when the tab was reporting the
/// intact trial's numbers for this file. The refusal is asserted by code and slot rather than
/// by message, so a reworded sentence does not read as a repaired reader.
#[test]
fn the_tab_declines_the_landmark_the_damaged_stance_puts_out_of_reach() {
    let answer = tab_answer(INTERRUPTED, "none");
    let result = &answer["ok"];
    assert!(
        !result.is_null(),
        "the tab answered with a refusal document rather than a result: {answer}"
    );

    let refusals = result["refusals"].as_array().expect("refusals is a list");
    assert_eq!(refusals.len(), 1, "refusals: {refusals:?}");
    assert_eq!(refusals[0]["code"], "no_crossing");
    assert_eq!(refusals[0]["slot"], "movement_onset");
    assert_eq!(
        result["warnings"].as_array().expect("warnings is a list").len(),
        1,
        "warnings: {:?}",
        result["warnings"]
    );

    // The three numbers the tab published for this recording while it was repairing it, each
    // read off the record by the key it carries there.
    for key in [
        "system_weight_newtons",
        "time_to_takeoff_seconds",
        "jump_height_from_takeoff_meters",
    ] {
        let metric = result["metrics"]
            .as_array()
            .expect("metrics is a list")
            .iter()
            .find(|metric| metric["key"] == key)
            .unwrap_or_else(|| panic!("{key} is reported"));
        assert!(
            metric["value"].is_null(),
            "{key} rested on a landmark this recording puts out of reach, and the tab \
             published {}",
            metric["value"]
        );
    }
}

/// The tab tells the damaged recording from the clean one, counted over the fields the parity
/// gate compares.
///
/// The measurement that found the defect, kept as the assertion. Projected over those fields
/// the tab's two answers differed in 0 places while the terminal's differed in 25, which is
/// the whole of what "the browser cannot tell them apart" means. A count rather than a list:
/// a legitimate change to either recording moves the count without changing what it means,
/// and 0 is the only value that says the reader is blind.
#[test]
fn the_tab_answers_the_damaged_recording_differently_from_the_clean_one() {
    let damaged = tab_answer(INTERRUPTED, "none");
    let clean = tab_answer(INTACT, "none");

    let compared = fields_the_gate_compares();
    // A field neither answer carries reads as two nulls and compares equal, which would let
    // the population narrow to nothing while this still reported a comparison. Asserted
    // before anything is counted.
    for field in &compared {
        for (which, answer) in [("damaged", &damaged), ("clean", &clean)] {
            assert!(
                answer["ok"].get(field).is_some(),
                "the tab's {which} answer carries no {field}, which the committed record \
                 compares on every surface"
            );
        }
    }
    let differing: Vec<&str> = compared
        .iter()
        .map(String::as_str)
        .filter(|field| damaged["ok"][field] != clean["ok"][field])
        .collect();

    // The clean answer is asserted to be an answer, so a run where both recordings failed to
    // load would not read as the two being told apart.
    assert!(
        clean["ok"]["refusals"]
            .as_array()
            .is_some_and(|refusals| refusals.is_empty()),
        "the clean recording answers without refusing: {}",
        clean["ok"]["refusals"]
    );
    assert!(
        !differing.is_empty(),
        "the tab answered a recording with three unreadable samples in its weighing window \
         exactly as it answered the intact one, over all {} compared fields",
        compared.len()
    );
    println!(
        "the tab's two answers differ in {} of {} compared fields: {differing:?}",
        differing.len(),
        compared.len()
    );
}

/// The tab hands the engine the trace the file wrote, under every convention it accepts.
///
/// Read through the envelope at one bucket per sample, which is the tab's own window onto the
/// trace it holds. The counts alone cannot see this: a reader that held every unreadable
/// sample at the last real reading published exactly the counts below while analysing a trace
/// with no gaps in it, and this assertion was written against the counts and passed while that
/// reader was in place.
#[test]
fn the_tab_hands_back_the_trace_the_file_wrote() {
    let wrote = as_the_file_wrote_it(INTERRUPTED);
    assert_eq!(wrote.len(), ROWS_IN_THE_RECORDING);
    let gaps: Vec<usize> = wrote
        .iter()
        .enumerate()
        .filter(|(_, value)| !value.is_finite())
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        gaps.len(),
        CARRYING_NO_NUMBER,
        "the recording this is about has to carry gaps, or nothing below is being asked"
    );

    let text = std::fs::read_to_string(INTERRUPTED).expect("the committed recording is readable");
    let file = ForceFile::parse_text(&text).expect("one value per line parses");
    for (convention, matched) in [
        ("none", 0),
        ("zero", MATCHING_THE_ZERO_CONVENTION),
        ("negative_one", 0),
    ] {
        let loaded = LoadedTrial::from_force_file(&file, 0, SAMPLE_RATE_HZ, convention)
            .expect("the column binds");
        let info: serde_json::Value =
            serde_json::from_str(&loaded.info_json().expect("the tab describes what it loaded"))
                .expect("the description parses");
        assert_eq!(
            info["sample_count"], ROWS_IN_THE_RECORDING,
            "under {convention} the tab loaded a different number of samples from the file's"
        );
        assert_eq!(info["samples_matching_the_convention"], matched);
        assert_eq!(info["samples_carrying_no_number"], CARRYING_NO_NUMBER);

        let envelope: serde_json::Value = serde_json::from_str(
            &loaded
                .envelope_json(ROWS_IN_THE_RECORDING)
                .expect("the tab draws what it holds"),
        )
        .expect("the envelope parses");
        let lower = envelope["lower"].as_array().expect("one bucket per sample");
        assert_eq!(lower.len(), ROWS_IN_THE_RECORDING);
        // A sample the file wrote no number at reaches the drawing as no number. A reader
        // that held it at the last real reading draws a force there, and one that removed it
        // shifts every sample after it into the wrong bucket.
        let reported_as_a_number: Vec<usize> = gaps
            .iter()
            .copied()
            .filter(|index| lower[*index].as_f64().is_some_and(f64::is_finite))
            .collect();
        assert!(
            reported_as_a_number.is_empty(),
            "under {convention} the tab holds a force at {reported_as_a_number:?}, where the \
             file wrote no number"
        );
        for index in [0, gaps[0] - 1, gaps[gaps.len() - 1] + 1, wrote.len() - 1] {
            assert_eq!(
                lower[index].as_f64(),
                Some(wrote[index]),
                "under {convention} sample {index} is not the one the file wrote"
            );
        }
    }
}

/// The batch reader hands the engine the trace the file wrote, compared bit for bit.
///
/// Bit patterns rather than values, because the three samples this recording is about carry no
/// number and a NaN never equals itself. A comparison by value would pass over exactly the
/// samples the reader used to remove.
#[test]
fn the_batch_reader_hands_back_the_trace_the_file_wrote() {
    let wrote = as_the_file_wrote_it(INTERRUPTED);
    let directory = std::env::temp_dir().join(format!(
        "plateforce-one-home-{}-{}",
        std::process::id(),
        line!()
    ));
    std::fs::create_dir_all(&directory).expect("a directory to walk");
    std::fs::copy(INTERRUPTED, directory.join("subject01_trial1.force.txt"))
        .expect("the recording copies");

    for declared in [None, Some(0.0)] {
        let format = SourceFormat {
            delimiter: '\t',
            force_column_index: 0,
            sample_rate_hz: SAMPLE_RATE_HZ,
            trial_file_suffixes: vec!["force.txt".to_string()],
            sentinel: declared,
        };
        let set = TrialSet::walk(&directory, &format, &TrialIdentity::FileStem)
            .expect("one trial is found");
        let (trial, _report, reported) = set
            .iter()
            .next()
            .expect("one trial is found")
            .1
            .source
            .read(&format)
            .expect("the recording reads");

        assert_eq!(
            bits(trial.force()),
            bits(&wrote),
            "declaring {declared:?}, the batch reader rewrote the trace"
        );
        assert_eq!(
            reported,
            reported_samples(&wrote, declared.map(Sentinel::Value)),
            "declaring {declared:?}, the batch reader's counts left the home's answer"
        );
        // Apart rather than as one total: under the zero convention the two are 157 and 3, and
        // the 157 are an athlete in the air.
        assert_eq!(reported.carried_no_number, CARRYING_NO_NUMBER);
        assert_eq!(
            reported.matched_the_convention,
            declared.map_or(0, |_| MATCHING_THE_ZERO_CONVENTION)
        );
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// Declaring the convention that matches the flight phase moves no number.
///
/// The invariant the parity gate's `sentinel` row states for the other four surfaces, asserted
/// here for the batch reader, which is not on that gate. It is the assertion the removing
/// reader failed: on this trial the zero convention matches the whole flight, so removing the
/// matches deleted the flight and moved jump height from flight time by 17.13 cm, from
/// 0.44022460156250015 m to 0.2689609062500001 m.
#[test]
fn declaring_the_convention_that_matches_the_flight_moves_no_number() {
    let wrote = as_the_file_wrote_it(INTACT);
    let (undeclared, _) =
        trial_from_column(wrote.clone(), SAMPLE_RATE_HZ, None).expect("the intact trial loads");
    let (declared, reported) =
        trial_from_column(wrote, SAMPLE_RATE_HZ, Some(Sentinel::Zero)).expect("and loads again");

    assert_eq!(
        reported.matched_the_convention, MATCHING_THE_ZERO_CONVENTION,
        "the convention has to match something here, or this asserts nothing"
    );
    assert_eq!(bits(declared.force()), bits(undeclared.force()));
    assert_eq!(declared.duration_seconds(), undeclared.duration_seconds());
}
