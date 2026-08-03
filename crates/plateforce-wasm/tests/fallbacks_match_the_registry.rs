//! What a rule uses when nobody chooses has to be what the registry says it is.
//!
//! A fallback is a default with no paperwork. When the registry declares one and the code
//! carries a different number, a user who reads the entry and a user who runs the software
//! get different answers from the same named method, and the fingerprint reports whichever
//! the code holds.
//!
//! The registry is not linked by the binding layer, on purpose: it takes bound values and
//! knows nothing about where they came from. So this comparison lives here, in the one crate
//! that has both the rules and the registry.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice, BINDINGS};
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use plateforce_wasm::demo::synthetic_countermovement_jump;
use plateforce_wasm::registry_embed;

/// The rules composed onto an onset threshold rule, which carry parameters of their own.
const ONSET_OPERATORS: &[&str] = &[
    "onset.op.backward_offset_fixed",
    "onset.op.crossing_selection",
    "onset.op.direction",
    "onset.op.persistence",
    "onset.op.search_floor",
    "onset.op.search_floor_at_weighing_epoch_end",
];

fn base_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    }
}

/// Run every rule this build offers with nothing stated, and collect what each one used.
fn values_the_rules_assumed() -> BTreeMap<String, BTreeMap<String, String>> {
    let trial = synthetic_countermovement_jump();
    let mut assumed: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    for binding in BINDINGS {
        let mut request = base_request();
        match binding.slot {
            "weighing" => {
                request.weighing.method_id = binding.id.to_string();
                request.weighing.parameters = BTreeMap::new();
            }
            "onset" => {
                request.onset = MethodChoice {
                    method_id: binding.id.to_string(),
                    ..Default::default()
                }
            }
            "takeoff" => {
                request.takeoff = MethodChoice {
                    method_id: binding.id.to_string(),
                    ..Default::default()
                }
            }
            // A rule reached by construct id rather than by a named field. Assigning one to
            // the takeoff field instead left `run` refusing an id filed elsewhere, the loop
            // stepping over the refusal, and this comparison reading none of them.
            construct => {
                request.derived.insert(
                    construct.to_string(),
                    MethodChoice {
                        method_id: binding.id.to_string(),
                        ..Default::default()
                    },
                );
                // The window rules place what several of these read, so one is named beside
                // the rule under test. A rule that declines for want of it records nothing.
                request
                    .derived
                    .entry("analysis_window".to_string())
                    .or_insert(MethodChoice {
                        method_id: "window_end.takeoff.detected".to_string(),
                        ..Default::default()
                    });
            }
        }
        let Ok(response) = run(&trial, &request) else {
            continue;
        };
        for bound in &response.bound_methods {
            let entry = assumed.entry(bound.method_id.clone()).or_default();
            for (name, shown) in &bound.bound_parameters {
                if bound.assumed_parameters().contains(name) {
                    entry.insert(name.clone(), shown.clone());
                }
            }
        }
    }
    assumed
}

#[test]
fn every_value_a_rule_assumes_is_the_one_the_registry_declares() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let assumed = values_the_rules_assumed();

    let mut compared = 0usize;
    let mut disagreements = Vec::new();

    for id in BINDINGS
        .iter()
        .map(|binding| binding.id)
        .chain(ONSET_OPERATORS.iter().copied())
    {
        let Some(entry) = loaded.registry.methods.get(id) else {
            continue;
        };
        let Some(used) = assumed.get(id) else {
            continue;
        };
        for parameter in &entry.parameters {
            let Some(declared) = parameter.default else {
                continue;
            };
            let Some(shown) = used.get(&parameter.name) else {
                continue;
            };
            let Ok(taken) = shown.parse::<f64>() else {
                continue;
            };
            compared += 1;
            if (taken - declared).abs() > 1e-9 {
                disagreements.push(format!(
                    "{id} declares {} = {declared} and the rule used {taken}",
                    parameter.name
                ));
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "a rule ran on a value the registry does not declare:\n  {}",
        disagreements.join("\n  ")
    );
    // Two populations, counted apart. A rule can record a value and declare no default, so
    // the second number is not a denominator for the first.
    println!(
        "{compared} declared defaults compared, from {} rules that recorded one, of {} binding rows and {} composed operators",
        assumed.len(),
        BINDINGS.len(),
        ONSET_OPERATORS.len()
    );
    assert!(
        compared >= 5,
        "only {compared} declared defaults were reached, so this comparison has stopped covering the rules"
    );
    // The rules reached, rather than the values compared. A rule with no declared default
    // contributes nothing above and still says whether this comparison can see it at all,
    // which is the half that was silently zero for every rule reached by construct id.
    assert!(
        assumed.len() >= 34,
        "only {} rules were reached, so rules this build runs are outside this comparison",
        assumed.len()
    );
}

/// The interface must not turn a published value into a choice. A parameter the registry
/// publishes values for but declares no default on is unresolved, and sending the first of
/// the list makes the record report a decision nobody took.
#[test]
fn a_published_value_is_not_a_default() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let decision_model = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/registry.js"
    ))
    .expect("the decision model is where the interface keeps it");

    assert!(
        !decision_model.contains("= choices[0]"),
        "web/registry.js binds the first published value when a parameter declares no default, \
         which the record then reports as stated rather than assumed"
    );

    let without_default: Vec<String> = BINDINGS
        .iter()
        .filter_map(|binding| loaded.registry.methods.get(binding.id))
        .flat_map(|entry| {
            entry
                .parameters
                .iter()
                .filter(|parameter| {
                    parameter.default.is_none() && !parameter.published_values.is_empty()
                })
                .map(move |parameter| format!("{}.{}", entry.id, parameter.name))
        })
        .collect();

    assert!(
        !without_default.is_empty(),
        "no parameter publishes values without declaring a default, so this guard is watching nothing"
    );
}
