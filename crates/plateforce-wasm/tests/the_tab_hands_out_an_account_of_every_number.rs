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

/// Which construct each landmark quantity is the answer of. Held here, while which rule fills
/// a construct is read from the binding table, so neither half is the declaration under test.
const LANDMARK_CONSTRUCT: [(&str, &str); 4] = [
    ("system_weight_newtons", "system_weight"),
    ("system_mass_kilograms", "system_weight"),
    ("onset_time_seconds", "movement_onset"),
    ("takeoff_time_seconds", "takeoff"),
];

/// The construct a rule fills, or nothing where no row declares it.
fn construct_of(method_id: &str) -> Option<String> {
    plateforce_analysis::BINDINGS
        .iter()
        .find(|binding| binding.id == method_id)
        .map(|binding| binding.construct.to_string())
}

/// The rule an account opens with, which is the root of the chain it was written around.
///
/// The first line is the number and its unit; the second is the rule and the values it was
/// bound to.
fn opening_rule(account: &str) -> String {
    account
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_default()
        .to_string()
}

/// A selection in the shape `web/analysis.js` posts it.
///
/// The onset rule is one whose search is bounded by takeoff, so the takeoff rule is among the
/// rules the onset time rests on and an account naming any of them is not yet an account
/// naming the rule that produced the number.
const REQUEST: &str = r#"{
  "weighing": {
    "method_id": "bwepoch.fixed_window",
    "start_index": null,
    "parameters": { "duration": 1.0 },
    "options": {}
  },
  "onset": {
    "method_id": "onset.threshold.last_within_band",
    "parameters": {},
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
  "registry_backed_ids": ["bwepoch.fixed_window", "onset.threshold.last_within_band", "takeoff.threshold.absolute_force"]
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

    // The rule the account opens with is the rule that produced the number. This asked only
    // that the rule appear somewhere in the account, against the first id the number rests on,
    // which is the conditioning rule on every quantity and is named in every account: the
    // question could not be answered no.
    for metric in metrics.iter().filter(|metric| !metric["value"].is_null()) {
        let key = metric["key"].as_str().expect("a metric names its quantity");
        let account = accounts[key].as_str().expect("an account is a sentence");
        let opened_with = opening_rule(account);
        match metric["computed_by"].as_str() {
            Some(computed_by) => assert_eq!(
                opened_with, computed_by,
                "the account of {key} opens with {opened_with}: {account}"
            ),
            // A landmark quantity names no arithmetic and is the answer of the rule filling
            // its own construct, which is a different question from which rules fed it.
            None => {
                let construct = LANDMARK_CONSTRUCT
                    .iter()
                    .find(|(quantity, _)| *quantity == key)
                    .map(|(_, construct)| *construct)
                    .unwrap_or_else(|| panic!("{key} names no arithmetic and no construct here"));
                let filled = construct_of(&opened_with);
                assert_eq!(
                    filled.as_deref(),
                    Some(construct),
                    "the account of {key} opens with {opened_with}, which fills {filled:?}"
                );
            }
        }
    }
    println!(
        "{} of {} quantities carried a value and each opened its account with the rule that \
         produced it",
        valued.len(),
        metrics.len()
    );
}
