//! The browser reduces, and refuses rather than taking a mean nobody chose.
//!
//! This surface looped the analysis and could not reduce, so `plateforce batch --aggregate`
//! reached `trial.aggregation` from the terminal and a tab could not reach it at all. That is
//! the peer gap under P3, and the manifest could not report it either until `Operation` gained
//! a value for the reduction.
//!
//! Driven through `batch_json` with the request the interface posts, because the reduction is
//! a field of that request rather than a second entry point. `scripts/check-batch.mjs` drives
//! the real page; this holds the shape the page posts.

use plateforce_wasm::batch::batch_document;

/// The rules the page opens on, with each one's parameters under the names the registry
/// publishes. A request naming no rule refuses every trial with `method_not_implemented`,
/// which is what the first draft of this fixture did while its control still read green.
const ANALYSIS: &str = r#"{
  "weighing": {
    "method_id": "bwepoch.fixed_window",
    "start_index": null,
    "parameters": { "duration": 1.0 },
    "options": { "dispersion": "sample", "centre": "mean", "window_anchor": "trial_start" }
  },
  "onset": {
    "method_id": "onset.threshold.noise_relative",
    "parameters": { "k": 5.0 },
    "options": { "selection": "first", "direction": "below_only" },
    "manual_index": null
  },
  "takeoff": {
    "method_id": "takeoff.threshold.absolute_force",
    "parameters": { "threshold_n": 20.0, "persistence_ms": 15.0 },
    "options": { "comparison": "signed", "short_run_handling": "rank_then_filter" },
    "manual_index": null
  },
  "touchdown_index": null,
  "gravity_meters_per_second_squared": 9.80665,
  "registry_backed_ids": ["bwepoch.fixed_window", "onset.threshold.noise_relative", "takeoff.threshold.absolute_force", "window_end.takeoff.detected", "force.peak.net"],
  "derived": {
    "analysis_window": { "method_id": "window_end.takeoff.detected" },
    "net_peak_force": { "method_id": "force.peak.net" }
  }
}"#;

/// Three trials of one athlete, generated arithmetic, named so a declared pattern yields a
/// subject. Each carries a different force scale, so the ranking criterion orders them
/// without a tie. Nothing here is athlete data.
fn dropped_files() -> String {
    let trial = plateforce_wasm::demo::synthetic_countermovement_jump();
    (1..=3)
        .map(|ordinal| {
            let text = trial
                .force()
                .iter()
                .map(|force| format!("{:.6}", force * (1.0 + ordinal as f64 / 100.0)))
                .collect::<Vec<String>>()
                .join("\n");
            format!(
                r#"{{"name":"AT01_{ordinal}.txt","text":{}}}"#,
                serde_json::to_string(&text).expect("a JSON string")
            )
        })
        .collect::<Vec<String>>()
        .join(",")
}

/// The request the page posts, with the reduction block filled in by the caller.
fn request(aggregate: &str) -> String {
    format!(
        r#"{{
          "files": [{files}],
          "format": {{
            "delimiter": "\t",
            "force_column_index": 0,
            "sample_rate_hz": 1200.0,
            "trial_file_suffixes": [".txt"],
            "sentinel": null
          }},
          "identity": {{ "kind": "declared_pattern", "template": "AT{{subject}}_{{trial}}" }},
          "analysis": {ANALYSIS},
          "resolved": ["system_weight", "movement_onset", "takeoff"]
          {aggregate}
        }}"#,
        files = dropped_files(),
    )
}

fn envelope(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("the envelope is JSON")
}

#[test]
fn a_tab_that_names_a_published_rule_reduces_and_records_which() {
    // The control, and the half that makes the rest mean anything: the same files with no
    // rule named come back with no reduction, so the assertion below is about the reduction
    // rather than about a tab that reduces whatever it is handed.
    let unreduced = envelope(&batch_document(&request("")).expect("the run returns an envelope"));
    let none = unreduced["ok"]["aggregates"]
        .as_array()
        .expect("the envelope carries the relation");
    assert!(
        none.is_empty(),
        "a run naming no rule reduced anyway, so the tab is taking a mean nobody chose",
    );
    // Without this the control is met by a run that computed nothing at all, which is a
    // different fact from a run that was asked to reduce nothing.
    // Counted off the rows the run produced rather than off a coverage field, because the
    // field's path is a property of the envelope and the rows are the thing being claimed.
    let computed = unreduced["ok"]["results"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|row| !row["provenance_id"].as_str().unwrap_or_default().is_empty())
                .count()
        })
        .unwrap_or(0);
    assert!(
        computed > 0,
        "the control run computed nothing, so an empty reduction says nothing about whether \
         this tab can reduce: {}",
        unreduced["ok"]["refusals"],
    );
    println!("control: the unreduced run computed {computed} trials and reduced none");

    let reduced = envelope(
        &batch_document(&request(
            r#", "aggregate": { "rule": "mean_of_best_two", "n": 2, "ranked_by": "net_peak_force" }"#,
        ))
        .expect("the run returns an envelope"),
    );
    let rows = reduced["ok"]["aggregates"]
        .as_array()
        .expect("the envelope carries the relation");
    assert!(
        !rows.is_empty(),
        "the tab named a published rule and reduced nothing: {}",
        reduced["ok"]["run"],
    );

    // The bound rule travels with the value. A reduction recording no method would be a mean
    // wearing a citation it never earned.
    for row in rows {
        assert_eq!(row["method_id"], "trial.aggregation");
        assert_eq!(row["n"], 2);
    }
    println!(
        "the tab reduced {} rows under trial.aggregation",
        rows.len()
    );
}

/// A rule the registry does not publish is refused rather than approximated.
///
/// `trial.aggregation` publishes three and none of them is the arithmetic mean of a session,
/// so the arithmetic mean is not the near-enough answer to a rule this registry does not carry.
#[test]
fn a_tab_naming_an_unpublished_rule_is_refused_rather_than_given_a_mean() {
    let refused = batch_document(&request(
        r#", "aggregate": { "rule": "arithmetic_mean", "n": 2 }"#,
    ));
    let message = refused.expect_err(
        "the tab accepted a rule the registry does not publish and returned a number for it",
    );
    // The refusal has to name the word the caller wrote. Asserting only that something failed
    // accepted a request that never parsed, which is how this test first passed.
    assert!(
        message.contains("arithmetic_mean"),
        "the refusal does not repeat the rule the caller named, so they cannot correct it: {message}",
    );
    assert!(
        !message.contains("did not parse"),
        "the request never reached the reduction, so this measures the fixture: {message}",
    );
}

/// A count the rule cannot run under is refused, because best of five and best of three are
/// different numbers and neither is defaultable.
#[test]
fn a_tab_naming_a_count_the_rule_cannot_run_under_is_refused() {
    let refused = batch_document(&request(
        r#", "aggregate": { "rule": "mean_of_best_two", "n": 1, "ranked_by": "net_peak_force" }"#,
    ));
    let message =
        refused.expect_err("the tab reduced two trials' worth of rule over a count of one");
    assert!(
        !message.contains("did not parse"),
        "the request never reached the reduction, so this measures the fixture: {message}",
    );
    println!("a count the rule cannot run under is refused: {message}");
}
