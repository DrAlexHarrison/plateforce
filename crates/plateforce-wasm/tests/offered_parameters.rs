//! Every parameter a control offers has to reach the rule and move a number.
//!
//! The interface draws a control for a registry parameter when that parameter carries
//! published values or a default, so the same rule decides what this file sweeps. When the
//! name a rule reads and the name the registry publishes drift apart, the value is dropped
//! on the floor, the rule runs its own instead, and the record still reports the value the
//! user picked. Nothing else in the suite sees that, because every number stays plausible.

use std::collections::BTreeMap;

use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use plateforce_wasm::analysis::{
    run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice, BINDINGS,
};
use plateforce_wasm::demo::synthetic_countermovement_jump;
use plateforce_wasm::registry_embed;

/// One control the interface draws, and the values this file drives it through.
struct OfferedParameter {
    slot: &'static str,
    method_id: String,
    parameter: String,
    probes: Vec<f64>,
}

/// The registry entries this build runs, crossed with the parameters those entries publish
/// a value or a default for. A parameter with neither has nothing to bind and no control.
fn offered_parameters() -> Vec<OfferedParameter> {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let mut offered = Vec::new();

    for binding in BINDINGS {
        let Some(method) = loaded.registry.methods.get(binding.id) else {
            continue;
        };
        for parameter in &method.parameters {
            let published: Vec<f64> = parameter
                .published_values
                .iter()
                .copied()
                .filter(|value| value.is_finite())
                .collect();
            if published.is_empty() && parameter.default.is_none() {
                continue;
            }
            offered.push(OfferedParameter {
                slot: binding.slot,
                method_id: binding.id.to_string(),
                parameter: parameter.name.clone(),
                probes: probes_for(&published, parameter.default),
            });
        }
    }
    offered
}

/// The published values, widened at both ends. The extremes are not offered as choices and
/// are not claimed to be published: a parameter whose published values happen to land on
/// the same sample would otherwise read as inert when it is wired correctly.
fn probes_for(published: &[f64], default: Option<f64>) -> Vec<f64> {
    let mut probes: Vec<f64> = published.to_vec();
    if let Some(value) = default.filter(|value| !probes.contains(value)) {
        probes.push(value);
    }
    let low = probes.iter().copied().fold(f64::INFINITY, f64::min);
    let high = probes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    probes.push(low / 8.0);
    probes.push(high * 8.0);
    probes
}

fn base_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
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
    }
}

fn request_with(offered: &OfferedParameter, value: f64) -> AnalysisRequest {
    let mut request = base_request();
    let choice = MethodChoice {
        method_id: offered.method_id.clone(),
        parameters: BTreeMap::from([(offered.parameter.clone(), value)]),
        ..Default::default()
    };
    match offered.slot {
        "weighing" => {
            request.weighing.method_id = offered.method_id.clone();
            request.weighing.parameters = choice.parameters;
        }
        "onset" => request.onset = choice,
        _ => request.takeoff = choice,
    }
    request
}

/// Everything the interface puts in front of a user, and nothing that restates the request.
/// A parameter that only changes its own entry in the fingerprint has not changed a number.
fn numbers(outcome: &Result<AnalysisResponse, String>) -> String {
    match outcome {
        Err(message) => format!("refused: {message}"),
        Ok(response) => {
            let mut text = format!(
                "{} {} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
                response.weighing_start_index,
                response.weighing_end_index,
                response.onset_index,
                response.takeoff_index,
                response.touchdown_index,
                response.levels.system_weight_newtons,
                response.levels.weighing_standard_deviation_newtons,
                response.levels.onset_band_lower_newtons,
                response.levels.onset_band_upper_newtons,
                response.levels.takeoff_threshold_newtons,
            );
            for metric in &response.metrics {
                text.push_str(&format!(" {:?}", metric.value));
            }
            text
        }
    }
}

#[test]
fn every_parameter_a_control_offers_reaches_the_rule_it_belongs_to() {
    let trial = synthetic_countermovement_jump();
    let offered = offered_parameters();
    assert!(
        offered.len() >= 8,
        "{} parameters were swept, which is fewer than the rules this build runs, so the sweep has stopped covering the interface",
        offered.len()
    );

    for parameter in &offered {
        let outcome = run(&trial, &request_with(parameter, parameter.probes[0]))
            .unwrap_or_else(|error| panic!("{} could not run: {error}", parameter.method_id));
        let bound = outcome
            .bound_methods
            .iter()
            .find(|method| method.method_id == parameter.method_id)
            .unwrap_or_else(|| panic!("{} bound nothing", parameter.method_id));
        assert!(
            !bound.unread_parameters.contains(&parameter.parameter),
            "{} offers '{}' and {} does not read it, so the value is dropped and the rule runs its own",
            parameter.slot,
            parameter.parameter,
            parameter.method_id
        );
    }
}

#[test]
fn every_parameter_a_control_offers_moves_a_number() {
    let trial = synthetic_countermovement_jump();

    for parameter in offered_parameters() {
        let outcomes: Vec<String> = parameter
            .probes
            .iter()
            .map(|value| numbers(&run(&trial, &request_with(&parameter, *value))))
            .collect();
        let distinct = outcomes.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert!(
            distinct > 1,
            "'{}' on {} is inert: {} values from {} to {} all return the same numbers",
            parameter.parameter,
            parameter.method_id,
            parameter.probes.len(),
            parameter.probes.iter().copied().fold(f64::INFINITY, f64::min),
            parameter.probes.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
    }
}

/// The mechanism the test above leans on. A name no rule reads has to be reported, because
/// a request carrying it looks identical to one that was honoured.
#[test]
fn a_name_the_rule_does_not_read_is_reported_rather_than_dropped_in_silence() {
    let trial = synthetic_countermovement_jump();
    let mut request = base_request();
    request
        .takeoff
        .parameters
        .insert("threshold_newtons".to_string(), 30.0);
    let response = run(&trial, &request).unwrap();
    let takeoff = response
        .bound_methods
        .iter()
        .find(|method| method.method_id == "takeoff.threshold.absolute_force")
        .unwrap();
    assert!(takeoff
        .unread_parameters
        .contains(&"threshold_newtons".to_string()));
}
