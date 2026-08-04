//! What a reader opening a run's folder learns about a trial a gate examined.
//!
//! A trial a gate removed keeps its row and every one of its numbers in `results`, and no
//! figure over the run is taken over it. Those two facts are both true at once and the results
//! table shows only the first, so a reader pooling that table by hand rebuilds a denominator
//! the run did not use. The exclusions relation is where the second fact lives.
//!
//! The test that carries the weight is the last one: for each column of the relation, what a
//! reader could and could not have got from `results.csv`.

mod common;

use std::collections::BTreeSet;

use common::{bound_request, declared_pattern, registry, synthetic_format, tempdir};
use plateforce_analysis::AnalysisResponse;
use plateforce_batch::{
    analyse, BatchResult, GateFinding, PopulationExclusion, Relation, TrialSet, ValidityGate,
};

const GATE_ID: &str = "trial.gate.between_trial_agreement.kraska2009";
const TRIALS: usize = 6;
const NAMED_BY_THE_GATE: usize = 3;
const MEASURED_DEVIATION_PERCENT: f64 = 10.0;
const CRITERION: &str = "sits outside the permitted deviation from this subject's other trials";

/// A gate naming the trials whose id ends in an even digit, so some of the set is named and
/// some is not and the relation carries rows a reader can tell apart.
struct HalfTheTrials;

impl ValidityGate for HalfTheTrials {
    fn method_id(&self) -> &str {
        GATE_ID
    }
    fn examine(&self, trial_id: &str, _response: &AnalysisResponse) -> Option<GateFinding> {
        let digit = trial_id.chars().last()?.to_digit(10)?;
        (digit % 2 == 0).then(|| GateFinding {
            parameter: Some("permitted_deviation_percent".to_string()),
            value: Some(MEASURED_DEVIATION_PERCENT),
            criterion: CRITERION.to_string(),
        })
    }
}

fn run(apply: bool) -> (std::path::PathBuf, BatchResult) {
    let directory = tempdir(if apply {
        "exclusions-removed"
    } else {
        "exclusions-reported"
    });
    plateforce_batch::synthetic::write_corpus(&directory, 1, TRIALS, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let mut request = bound_request().with_gate(Box::new(HalfTheTrials));
    if apply {
        request.gates.apply(GATE_ID);
    }
    let result = analyse(&set, &request, &registry()).expect("every trial computes");
    (directory, result)
}

fn table(result: &BatchResult, directory: &std::path::Path) -> Vec<Vec<String>> {
    let out = directory.join("out");
    result.write_csv(&out).expect("the tables write");
    let text =
        std::fs::read_to_string(out.join("exclusions.csv")).expect("the relation is on disk");
    text.lines()
        .map(|line| line.split(',').map(str::to_string).collect())
        .collect()
}

#[test]
fn a_run_writes_its_gate_findings_beside_its_numbers() {
    let (directory, result) = run(true);
    let rows = table(&result, &directory);
    for row in &rows {
        println!("{row:?}");
    }

    assert_eq!(Relation::Exclusions.file_name(), "exclusions.csv");
    assert_eq!(rows[0], PopulationExclusion::header());
    assert_eq!(rows.len(), 1 + NAMED_BY_THE_GATE);
    std::fs::remove_dir_all(&directory).ok();
}

/// The column that carries what the results table cannot. Both runs examine the same trials
/// and find the same thing; only the request differs, and only this column shows it.
#[test]
fn the_outcome_column_says_whether_the_trial_left_the_population() {
    let (removed_dir, removed) = run(true);
    let (reported_dir, reported) = run(false);

    let outcome = |result: &BatchResult, directory: &std::path::Path| -> BTreeSet<String> {
        table(result, directory)
            .into_iter()
            .skip(1)
            .map(|row| row[3].clone())
            .collect()
    };
    let when_removed = outcome(&removed, &removed_dir);
    let when_reported = outcome(&reported, &reported_dir);
    println!("applied: {when_removed:?}, reporting: {when_reported:?}");

    assert_eq!(when_removed, BTreeSet::from(["removed".to_string()]));
    assert_eq!(when_reported, BTreeSet::from(["reported".to_string()]));

    // Both runs found the same trials. A reader cannot tell the two apart from `results`,
    // which is the whole reason the column exists.
    let named = |result: &BatchResult| -> BTreeSet<String> {
        result
            .exclusions
            .iter()
            .map(|row| row.trial_id.clone())
            .collect()
    };
    assert_eq!(named(&removed), named(&reported));
    assert_eq!(named(&removed).len(), NAMED_BY_THE_GATE);

    std::fs::remove_dir_all(&removed_dir).ok();
    std::fs::remove_dir_all(&reported_dir).ok();
}

/// Column by column, what the row tells a reader that the results table does not.
///
/// Asserted rather than described, because a relation that repeated what the reader already
/// had would be a file to open for nothing.
#[test]
fn every_column_carries_something_the_results_table_does_not() {
    let (directory, result) = run(true);
    let rows = table(&result, &directory);
    let header = &rows[0];
    let row = &rows[1];
    let trial_id = &row[0];

    let results_row = result
        .results
        .iter()
        .find(|entry| entry.trial_id == *trial_id)
        .expect("the removed trial keeps its row and its numbers");
    // The premise. A removed trial is not absent from the table, it is present and complete,
    // which is why nothing in the table can say it was removed.
    assert!(results_row.refusal_code.is_empty());
    assert!(
        results_row.values.values().any(|value| value.is_some()),
        "the removed trial carries numbers a reader would otherwise pool"
    );

    let in_the_results_table: BTreeSet<&str> =
        ["trial_id", "source_path", "provenance_id", "refusal_code"]
            .into_iter()
            .chain(result.quantities.iter().map(String::as_str))
            .collect();
    for name in header {
        if name == "trial_id" {
            continue;
        }
        assert!(
            !in_the_results_table.contains(name.as_str()),
            "{name} is already a column of the results table"
        );
    }

    // And each one is populated rather than reserved.
    println!("{header:?}");
    println!("{row:?}");
    assert_eq!(row[2], GATE_ID, "which rule examined the trial");
    assert_eq!(row[3], "removed", "whether it left the population");
    assert_eq!(row[4], "permitted_deviation_percent", "what the rule read");
    assert_eq!(row[5], "10.0", "the figure it measured");
    assert_eq!(row[6], CRITERION, "what it concluded, in the gate's words");
    std::fs::remove_dir_all(&directory).ok();
}

/// The columnar surface, for the reader who opens the run in R or pandas rather than a
/// spreadsheet.
#[cfg(feature = "parquet")]
#[test]
fn the_columnar_surface_carries_the_findings_too() {
    use plateforce_batch::write_parquet::read_relation;

    let (directory, result) = run(true);
    let out = directory.join("parquet");
    let written = result.write_parquet(&out).expect("the relations write");
    let path = written
        .iter()
        .find(|path| path.file_name().unwrap() == "exclusions.parquet")
        .expect("the run wrote an exclusions relation");

    let batches = read_relation(path).expect("the relation reads back");
    let rows: usize = batches.iter().map(|batch| batch.num_rows()).sum();
    println!("exclusions.parquet holds {rows} rows");
    assert_eq!(rows, NAMED_BY_THE_GATE);
    let schema = batches[0].schema();
    let columns: Vec<&str> = schema
        .fields()
        .iter()
        .map(|field| field.name().as_str())
        .collect();
    assert_eq!(columns, PopulationExclusion::header());
    std::fs::remove_dir_all(&directory).ok();
}
