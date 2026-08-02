//! One surface, two renderings, and the same record under both.

mod common;

use common::{bound_request, declared_pattern, registry, synthetic_format, tempdir};
use plateforce_batch::render::Rendering;
use plateforce_batch::{analyse, with_aggregates, AggregationRequest, GroupKind, TrialSet};
use plateforce_core::DispersionEstimator;

const HEIGHT: &str = "jump_height_from_takeoff_meters";

fn run(name: &str) -> (std::path::PathBuf, TrialSet, plateforce_batch::BatchResult) {
    let directory = tempdir(name);
    plateforce_batch::synthetic::write_corpus(&directory, 4, 3, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(result.coverage.computed, 12, "every trial computed");
    let plan = AggregationRequest::declared(
        Some("mean_of_best_two"),
        Some(2),
        GroupKind::Subject,
        vec![HEIGHT.to_string()],
        DispersionEstimator::Sample,
    )
    .unwrap();
    let joined = with_aggregates(result, &set, &plan).unwrap();
    (directory, set, joined)
}

#[test]
fn both_renderings_record_the_same_provenance() {
    let (directory, _set, result) = run("render-same-record");

    let long = result.render(Rendering::WithProvenance);
    let short = result.render(Rendering::WithoutProvenance);
    println!("long header:  {}", long.header.join(", "));
    println!("short header: {}", short.header.join(", "));

    // Anchored to the result that entered the rendering rather than to a second rendering
    // pass, because two passes through one writer agree even when both are wrong.
    let written = directory.join("record");
    result.write_csv(&written).unwrap();
    let provenance = std::fs::read_to_string(written.join("provenance.csv")).unwrap();
    assert!(!provenance.trim().is_empty(), "the record is not empty");

    let chains: Vec<&String> = result
        .results
        .iter()
        .map(|row| &row.provenance_id)
        .filter(|id| !id.is_empty())
        .collect();
    assert!(!chains.is_empty(), "and the rows reach it");
    for id in &chains {
        assert!(
            provenance.contains(id.as_str()),
            "{id} names a chain the record does not carry"
        );
    }

    // Neither rendering may lose a trial the record accounts for.
    for (name, rendered) in [("long", &long), ("short", &short)] {
        assert_eq!(
            rendered.rows.len(),
            result.results.len(),
            "the {name} rendering shows every row the record carries"
        );
    }
    println!(
        "provenance rows recorded: {}, chains reaching them: {} of {} result rows",
        provenance.lines().count() - 1,
        chains.len(),
        result.results.len()
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_shorter_rendering_hides_a_column_and_drops_no_row() {
    let (directory, _set, result) = run("render-hides-a-column");

    let long = result.render(Rendering::WithProvenance);
    let short = result.render(Rendering::WithoutProvenance);

    println!(
        "long {} columns x {} rows, short {} columns x {} rows",
        long.header.len(),
        long.rows.len(),
        short.header.len(),
        short.rows.len()
    );
    assert_eq!(long.rows.len(), short.rows.len(), "no row is dropped");
    assert_eq!(
        long.header.len(),
        short.header.len() + 1,
        "exactly one column differs"
    );
    assert!(long.header.iter().any(|name| name == "provenance_id"));
    assert!(!short.header.iter().any(|name| name == "provenance_id"));

    // Every quantity column survives both renderings.
    for quantity in &result.quantities {
        assert!(short.header.contains(quantity), "{quantity} survives");
    }
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_summary_renders_beneath_the_table_with_its_count_beside_it() {
    let (directory, _set, result) = run("render-summary");
    let rendered = result.render(Rendering::WithoutProvenance);

    println!("coverage: {}", rendered.coverage);
    for line in &rendered.summary {
        println!("summary:  {line}");
    }
    assert_eq!(rendered.summary.len(), 4, "one line per subject");
    for line in &rendered.summary {
        assert!(line.contains("n = 2"), "the count travels: {line}");
        assert!(line.contains(HEIGHT), "and the quantity is named: {line}");
    }

    // The summary is a rendering of `aggregates` beneath the table, not a row inside it.
    assert_eq!(
        rendered.rows.len(),
        12,
        "twelve trials, and no mean among them"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn the_coverage_line_is_the_same_fact_under_both_renderings() {
    let (directory, _set, result) = run("render-coverage");
    assert_eq!(
        result.render(Rendering::WithProvenance).coverage,
        result.render(Rendering::WithoutProvenance).coverage
    );
    std::fs::remove_dir_all(&directory).ok();
}
