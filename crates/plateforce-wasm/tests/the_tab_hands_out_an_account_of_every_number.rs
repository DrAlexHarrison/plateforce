//! What leaves the tab carries the account each number gives of itself.
//!
//! This surface passed an empty block into the document it hands back, under a field the
//! wire skipped when it was empty, so a result carried out of a browser tab named its rules
//! and gave no account of any number. An R session, reading the same engine, gave one for
//! every number it reported.
//!
//! Asked through the entry point the page calls rather than by rebuilding the document here,
//! because what a reader carries away is what that call returns.

use plateforce_wasm::LoadedTrial;

/// The opening selection, in the shape `web/analysis.js` posts it.
const REQUEST: &str = r#"{
  "weighing": {
    "method_id": "bwepoch.fixed_window",
    "start_index": null,
    "parameters": { "duration": 1.0 },
    "options": {}
  },
  "onset": {
    "method_id": "onset.threshold.noise_relative",
    "parameters": { "k": 5.0 },
    "options": {},
    "manual_index": null
  },
  "takeoff": {
    "method_id": "takeoff.threshold.absolute_force",
    "parameters": { "threshold_n": 20.0, "persistence_ms": 15.0 },
    "options": {},
    "manual_index": null
  },
  "touchdown_index": null,
  "gravity_meters_per_second_squared": 9.80665,
  "registry_backed_ids": ["bwepoch.fixed_window", "onset.threshold.noise_relative", "takeoff.threshold.absolute_force"]
}"#;

fn analysed() -> serde_json::Value {
    let answered = LoadedTrial::demonstration()
        .analyse(REQUEST, Some("demonstration".to_string()), None)
        .unwrap_or_else(|_| panic!("the tab answers the request its own page posts"));
    let parsed: serde_json::Value = serde_json::from_str(&answered).expect("the document parses");
    parsed
        .get("ok")
        .cloned()
        .unwrap_or_else(|| panic!("the tab returned a refusal: {parsed}"))
}

/// Every number the tab reports gives an account of itself in the document it hands back.
#[test]
fn every_number_carried_out_of_the_tab_gives_an_account_of_itself() {
    let document = analysed();
    let metrics = document["metrics"].as_array().expect("metrics is a list");
    let accounts = document["descriptions"]
        .as_object()
        .expect("descriptions is a block");

    let valued: Vec<&str> = metrics
        .iter()
        .filter(|metric| !metric["value"].is_null())
        .map(|metric| metric["key"].as_str().expect("a metric names its quantity"))
        .collect();

    // The denominator the sentence below is over. A tab reporting almost nothing would
    // satisfy the comparison having looked at almost nothing.
    assert!(
        valued.len() >= 8,
        "only {} of {} quantities carried a value",
        valued.len(),
        metrics.len()
    );

    let silent: Vec<&&str> = valued
        .iter()
        .filter(|key| !accounts.contains_key(**key))
        .collect();
    assert!(
        silent.is_empty(),
        "{} of {} quantities carrying a value gave no account of themselves: {silent:?}",
        silent.len(),
        valued.len()
    );

    // The rule the record names is the rule the sentence names, so a block filled with
    // anything at all does not pass this.
    for metric in metrics.iter().filter(|metric| !metric["value"].is_null()) {
        let key = metric["key"].as_str().expect("a metric names its quantity");
        let named = metric["computed_by"]
            .as_str()
            .or_else(|| metric["contributing_method_ids"][0].as_str())
            .expect("a number names a rule behind it");
        let account = accounts[key].as_str().expect("an account is a sentence");
        assert!(
            account.contains(named),
            "the account of {key} never names {named}: {account}"
        );
    }
    println!(
        "{} of {} quantities carried a value and each gave an account naming its rule",
        valued.len(),
        metrics.len()
    );
}
