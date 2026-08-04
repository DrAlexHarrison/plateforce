//! Whether a reader who runs a folder is told what a reader who runs one trial is told.
//!
//! A batch is the surface whose numbers go into a spreadsheet, and a landmark placed at the
//! boundary of its own search looks in that spreadsheet exactly like a landmark found in the
//! trace. The analysis has already worked out which is which by the time the loop places a
//! row, so the property here is that nothing it worked out is dropped on the way to the
//! table.
//!
//! The expected side is built by calling the analysis directly rather than by reading the
//! batch back, so the two sides are two paths over one recording and not one path compared
//! with itself.

mod common;

use std::collections::BTreeSet;

use plateforce_analysis::run;
use plateforce_batch::{analyse, Relation, SignalRow, TrialIdentity, TrialSet};
use plateforce_core::read_trial_from_path;

use common::{analysis_request, bound_request, committed_format, registry, tempdir, FIXTURES};

/// The recording and the rule that place an onset at the floor of the rule's own search: the
/// quiet stance already sits outside a 20 N band, so the first sample the rule may examine
/// already satisfies it.
const TRIAL_FILE: &str = "subject01_trial1.force.txt";
const TRIAL_ID: &str = "subject01_trial1";
const FLOOR_LANDING_ONSET_RULE: &str = "onset.threshold.absolute_force";
const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// A request whose onset rule returns its floor on this recording, so the run has something
/// to carry. A rule that found a departure would leave every relation below empty and the
/// guard would pass on a surface that drops signals silently.
fn request_that_raises_a_signal() -> plateforce_analysis::AnalysisRequest {
    let mut analysis = analysis_request(1.0);
    analysis.onset.method_id = FLOOR_LANDING_ONSET_RULE.to_string();
    analysis.onset.parameters = [("threshold_n".to_string(), 20.0)].into_iter().collect();
    analysis
}

/// What the analysis says about this trial, reached without going through the batch at all.
fn signals_the_analysis_raised() -> Vec<plateforce_analysis::quality::QualitySignal> {
    let (trial, _) = read_trial_from_path(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        '\t',
        0,
        CORPUS_SAMPLE_RATE_HZ,
    )
    .expect("the committed trace reads");
    run(&trial, &request_that_raises_a_signal())
        .expect("the trial computes")
        .signals
}

fn run_over_one_trial() -> plateforce_batch::BatchResult {
    let directory = tempdir("signals");
    std::fs::copy(
        format!("{FIXTURES}/{TRIAL_FILE}"),
        directory.join(TRIAL_FILE),
    )
    .expect("the fixture copies");

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem)
        .expect("the folder walks");
    let mut request = bound_request();
    request.analysis = request_that_raises_a_signal();
    analyse(&set, &request, &registry()).expect("the run produces a result")
}

#[test]
fn every_signal_the_analysis_raised_reaches_the_run() {
    let raised = signals_the_analysis_raised();
    assert!(
        !raised.is_empty(),
        "the fixture and rule chosen here raise nothing, so this guard could not fail"
    );

    let result = run_over_one_trial();
    println!(
        "analysis raised {}, the run carries {}",
        raised.len(),
        result.signals.len()
    );
    assert_eq!(result.signals.len(), raised.len());

    for (signal, row) in raised.iter().zip(&result.signals) {
        assert_eq!(row.trial_id, TRIAL_ID);
        assert_eq!(row.label, signal.label);
        assert_eq!(row.value, signal.value);
        assert_eq!(row.unit, signal.unit);
        assert_eq!(row.threshold, signal.threshold);
        assert_eq!(row.remedy, signal.remedy);
        assert_eq!(row.remedy_construct, signal.remedy_construct);
        assert_eq!(row.qualifies, signal.qualifies.join(","));
        // Named rather than blank. A status that reached a reader as an empty cell would read
        // as a signal with nothing wrong.
        assert!(!row.status.is_empty(), "{row:?}");
        assert_eq!(row.status, "at_search_floor");
    }
}

/// The columns the signal is about have to be the columns the table actually holds, or a
/// reader joining the two finds nothing and concludes the signal is about something else.
#[test]
fn the_columns_a_signal_names_are_columns_the_results_table_carries() {
    let result = run_over_one_trial();
    let quantities: BTreeSet<&str> = result.quantities.iter().map(String::as_str).collect();

    let mut checked = 0;
    for row in &result.signals {
        for key in row.qualifies.split(',') {
            assert!(
                quantities.contains(key),
                "the signal names {key}, which results does not carry: {quantities:?}"
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "no signal named a column, so nothing was checked"
    );
    println!(
        "{checked} column names checked against {} quantities",
        quantities.len()
    );
}

/// A relation nobody writes is a relation nobody reads. The written set is asserted rather
/// than the enum, because the enum being right and the writer skipping it is the failure.
#[test]
fn a_run_writes_its_signals_beside_its_numbers() {
    let result = run_over_one_trial();
    let out = tempdir("signals-out");
    let written = result.write_csv(&out).expect("the tables write");

    let names: BTreeSet<String> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    println!("{names:?}");
    assert!(names.contains("signals.csv"), "{names:?}");
    assert_eq!(Relation::Signals.file_name(), "signals.csv");

    let text = std::fs::read_to_string(out.join("signals.csv")).expect("the relation is on disk");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1 + result.signals.len(), "{text}");
    assert!(
        lines[0].starts_with("trial_id,ordinal,status,"),
        "{}",
        lines[0]
    );
    // The trial, the status and the columns it qualifies, in the row a reader opens.
    assert!(lines[1].contains(TRIAL_ID), "{}", lines[1]);
    assert!(lines[1].contains("at_search_floor"), "{}", lines[1]);
    assert!(lines[1].contains("time_to_takeoff_seconds"), "{}", lines[1]);
}

/// The envelope every non-terminal surface returns. A signal that travels in the CSV and not
/// in the JSON reaches a spreadsheet and not a program.
#[test]
fn the_envelope_carries_the_signals_through_a_round_trip() {
    let result = run_over_one_trial();
    let text = result.to_json();
    assert!(
        text.contains("at_search_floor"),
        "the envelope drops the status"
    );

    let read_back = plateforce_batch::BatchResult::from_json(&text).expect("the envelope reads");
    assert_eq!(read_back.signals, result.signals);
}

/// The columnar surface an R or pandas reader opens. A caveat that reaches the CSV and not
/// this one reaches the reader who least expects to need it.
#[cfg(feature = "parquet")]
#[test]
fn the_columnar_surface_carries_them_too() {
    use plateforce_batch::write_parquet::read_relation;

    let result = run_over_one_trial();
    let out = tempdir("signals-parquet");
    let written = result.write_parquet(&out).expect("the relations write");
    let path = written
        .iter()
        .find(|path| path.file_name().unwrap() == "signals.parquet")
        .expect("the run wrote a signals relation");

    let batches = read_relation(path).expect("the relation reads back");
    let rows: usize = batches.iter().map(|batch| batch.num_rows()).sum();
    println!("signals.parquet holds {rows} rows");
    assert_eq!(rows, result.signals.len());
    let schema = batches[0].schema();
    let columns: Vec<&str> = schema
        .fields()
        .iter()
        .map(|field| field.name().as_str())
        .collect();
    assert_eq!(columns, SignalRow::header());
}

/// A reader watching a run go past sees the table. If the caveat is only in a file beside it,
/// the table is read as a table of numbers.
#[test]
fn the_rendered_table_says_it_too() {
    let result = run_over_one_trial();
    let rendered = result.render(plateforce_batch::Rendering::WithProvenance);
    println!("{:?}", rendered.signals);
    assert_eq!(rendered.signals.len(), result.signals.len());
    assert!(
        rendered.signals[0].contains(TRIAL_ID),
        "{:?}",
        rendered.signals
    );
    assert!(
        rendered.signals[0].contains("time_to_takeoff_seconds"),
        "{:?}",
        rendered.signals
    );
    // The action the analysis composed, carried whole rather than summarised again here.
    assert!(
        rendered.signals[0].ends_with(&result.signals[0].remedy),
        "{:?}",
        rendered.signals
    );
}
