//! Every parameter a control offers has to reach the rule and move a number.
//!
//! The interface draws a control for a registry parameter when that parameter carries
//! published values or a default, so the same rule decides what this file sweeps. When the
//! name a rule reads and the name the registry publishes drift apart, the value is dropped
//! on the floor, the rule runs its own instead, and the record still reports the value the
//! user picked. Nothing else in the suite sees that, because every number stays plausible.

use std::collections::BTreeMap;

use plateforce_analysis::{
    run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice, BINDINGS,
};
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_wasm::demo::synthetic_countermovement_jump;
use plateforce_wasm::registry_embed;

/// The demonstration jump with a brief brush against the plate's floor during the
/// countermovement, before the athlete actually leaves.
///
/// One clean trace cannot exercise every parameter. A rule that asks how long a crossing must
/// hold has nothing to decide on a trace that crosses once and stays across, so the parameter
/// reads as inert when it is wired correctly. This is the trace where it decides something,
/// and it is the shape of the real failure: an athlete who unloads the plate without leaving it.
fn jump_with_a_brief_dip_before_takeoff() -> Trial {
    let demonstration = synthetic_countermovement_jump();
    let rate = demonstration.sample_rate_hz();
    let mut force = demonstration.force().to_vec();
    let dip_start = (3.58 * rate) as usize;
    let dip_samples = (0.010 * rate) as usize;
    for sample in force.iter_mut().skip(dip_start).take(dip_samples) {
        *sample = 5.0;
    }
    Trial::new(force, rate).expect("the modified trace is still a trial")
}

/// The demonstration jump followed by a long, quiet stretch with the athlete mostly off the
/// plate, which is what an untrimmed recording holds after someone steps away.
///
/// That stretch is quieter than quiet standing, so a lowest-variance search takes it and reads
/// system weight as a fraction of the real one unless a gate refuses it. The gate is a fraction
/// of weight, so a trace whose leftover load is near zero rejects at every fraction and decides
/// nothing. Here the leftover load sits inside the range the sweep drives, so the fraction picks
/// between two different weighing windows.
fn jump_followed_by_the_athlete_stepping_mostly_off() -> Trial {
    let demonstration = synthetic_countermovement_jump();
    let rate = demonstration.sample_rate_hz();
    let mut force = demonstration.force().to_vec();
    let standing_newtons = force[..rate as usize].iter().sum::<f64>() / rate;
    let leftover_newtons = standing_newtons * 0.08;
    force.extend(
        (0..(2.0 * rate) as usize)
            .map(|index| leftover_newtons + ((index % 7) as f64 - 3.0) * 0.002),
    );
    Trial::new(force, rate).expect("the modified trace is still a trial")
}

/// The system weight the demonstration trace stands at, which every trace below scales its
/// inserted forces by.
fn standing_newtons(force: &[f64], rate: f64) -> f64 {
    force[..rate as usize].iter().sum::<f64>() / rate
}

/// The demonstration jump with a brief airborne-shaped event before it: the plate unloads for
/// ten milliseconds and force returns through a collision rather than through a push.
///
/// A rule that filters runs by length has nothing to decide unless a short run would otherwise
/// be taken, and the trace above cannot supply one: its dip returns through the countermovement,
/// which rises at 7.6 bodyweights per second and is rejected on shape whatever its length. Here
/// the short run is landing-shaped, so length is the only thing left deciding.
fn jump_after_a_short_run_that_ends_in_a_collision() -> Trial {
    let demonstration = synthetic_countermovement_jump();
    let rate = demonstration.sample_rate_hz();
    let mut force = demonstration.force().to_vec();
    let standing = standing_newtons(&force, rate);
    let unloaded_start = (1.5 * rate) as usize;
    let unloaded_samples = (0.010 * rate) as usize;
    let rise_samples = (0.020 * rate) as usize;
    for offset in 0..unloaded_samples {
        force[unloaded_start + offset] = 0.0;
    }
    for offset in 0..rise_samples {
        let fraction = (offset + 1) as f64 / rise_samples as f64;
        force[unloaded_start + unloaded_samples + offset] = 3.0 * standing * fraction;
    }
    for offset in 0..rise_samples {
        let fraction = (offset + 1) as f64 / rise_samples as f64;
        force[unloaded_start + unloaded_samples + rise_samples + offset] =
            standing + (3.0 * standing - standing) * (1.0 - fraction);
    }
    Trial::new(force, rate).expect("the modified trace is still a trial")
}

/// The demonstration jump whose flight begins with the unloaded plate chattering back across
/// the threshold for four milliseconds.
///
/// Bridged, the flight is one run and takeoff sits where the plate first unloaded. Unbridged it
/// is two, the first is too short to be a flight phase at all, and takeoff moves ten samples
/// later to the second. The chatter is brief enough and small enough to sit inside one probe of
/// each bridging parameter and outside the other, so each decides on its own.
fn jump_whose_flight_opens_with_chatter() -> Trial {
    let demonstration = synthetic_countermovement_jump();
    let rate = demonstration.sample_rate_hz();
    let mut force = demonstration.force().to_vec();
    let unloads_at = force
        .iter()
        .position(|newtons| *newtons < 20.0)
        .expect("the demonstration trace holds a flight phase");
    for offset in 0..5 {
        force[unloads_at + 5 + offset] = 25.0;
    }
    Trial::new(force, rate).expect("the modified trace is still a trial")
}

/// The demonstration jump whose landing rises slowly enough that a short window cannot see its
/// peak.
///
/// The rise reaches three bodyweights over 120 ms, which clears the rate floor and the height
/// floor. Read through a window of a few milliseconds it reaches under half a bodyweight, falls
/// below the height floor, and the landing stops being one, which is the reason the window has a
/// length at all.
fn jump_whose_landing_rises_slowly() -> Trial {
    let demonstration = synthetic_countermovement_jump();
    let rate = demonstration.sample_rate_hz();
    let mut force = demonstration.force().to_vec();
    let standing = standing_newtons(&force, rate);
    let landing_start = (4.76 * rate) as usize;
    let rise_samples = (0.120 * rate) as usize;
    for (elapsed, sample) in force.iter_mut().skip(landing_start).enumerate() {
        *sample = if elapsed < rise_samples {
            3.0 * standing * (elapsed + 1) as f64 / rise_samples as f64
        } else {
            3.0 * standing
        };
    }
    Trial::new(force, rate).expect("the modified trace is still a trial")
}

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
    probes.push(low / REACH_BEYOND_THE_PUBLISHED_VALUE);
    probes.push(high * REACH_BEYOND_THE_PUBLISHED_VALUE);
    probes
}

/// How far past the published values a sweep drives, and it has to clear the largest value the
/// traces present rather than the largest a rule was tuned for.
///
/// A published value is often deliberately low: `landing_peak_floor_bodyweights` is 0.5 because
/// a truncated landing has to still be accepted, while the demonstration trace peaks at 4.42
/// bodyweights after takeoff. At a reach of 8 the sweep topped out at 4.0, every value accepted
/// the landing, and a parameter that decides the verdict at 5 read as inert.
const REACH_BEYOND_THE_PUBLISHED_VALUE: f64 = 32.0;

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
        "takeoff" => request.takeoff = choice,
        // A rule reached by construct id, plus the first rule of every construct declared
        // before its own so anything it reads has been placed. Without that second half
        // every parameter on such a rule reads as inert here, because the rule declines
        // identically at every value and the sweep sees one answer. Whatever an earlier
        // entry states required with no default is answered for the same reason: a rule that
        // declines for want of it places nothing, and everything downstream reads as inert.
        construct => {
            for earlier in plateforce_analysis::binding::derived_bindings() {
                if earlier.construct == construct {
                    break;
                }
                request
                    .derived
                    .entry(earlier.construct.to_string())
                    .or_insert_with(|| MethodChoice {
                        method_id: earlier.id.to_string(),
                        options: plateforce_analysis::binding::required_options(earlier.id)
                            .iter()
                            .map(|(name, value)| (name.to_string(), value.to_string()))
                            .collect(),
                        ..Default::default()
                    });
            }
            request.derived.insert(construct.to_string(), choice);
        }
    }
    request
}

/// Everything the interface puts in front of a user, and nothing that restates the request.
/// A parameter that only changes its own entry in the fingerprint has not changed a number.
fn numbers(outcome: &Result<AnalysisResponse, Box<plateforce_core::Refusal>>) -> String {
    match outcome {
        // The code as well as the sentence, so a parameter that moves a refusal from one
        // class to another shows up here as a changed line.
        Err(refusal) => format!("refused: {} {refusal}", refusal.code.wire_name()),
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
    let traces = [
        ("the demonstration jump", synthetic_countermovement_jump()),
        (
            "a jump with a brief dip",
            jump_with_a_brief_dip_before_takeoff(),
        ),
        (
            "a jump followed by the athlete stepping mostly off",
            jump_followed_by_the_athlete_stepping_mostly_off(),
        ),
        (
            "a jump after a short run that ends in a collision",
            jump_after_a_short_run_that_ends_in_a_collision(),
        ),
        (
            "a jump whose flight opens with chatter",
            jump_whose_flight_opens_with_chatter(),
        ),
        (
            "a jump whose landing rises slowly",
            jump_whose_landing_rises_slowly(),
        ),
    ];

    let offered = offered_parameters();
    let offered_count = offered.len();
    let mut inert: Vec<String> = Vec::new();

    for parameter in offered {
        // Moving a number on one trace is enough. Requiring it on every trace would demand
        // that a parameter matter even where the recording gives it nothing to decide.
        let moved_on = traces.iter().find(|(_, trial)| {
            let outcomes: Vec<String> = parameter
                .probes
                .iter()
                .map(|value| numbers(&run(trial, &request_with(&parameter, *value))))
                .collect();
            outcomes
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1
        });
        if moved_on.is_none() {
            let low = parameter
                .probes
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let high = parameter
                .probes
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            inert.push(format!(
                "'{}' on {}: {} values from {low} to {high} return the same numbers",
                parameter.parameter,
                parameter.method_id,
                parameter.probes.len(),
            ));
        }
    }

    // Every inert parameter, not the first. A guard that stops at one makes the next run find
    // the next, and hides how many traces the fixtures are short of.
    assert!(
        inert.is_empty(),
        "{} of {} offered parameters are inert over {} traces:\n  {}",
        inert.len(),
        offered_count,
        traces.len(),
        inert.join("\n  ")
    );
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
