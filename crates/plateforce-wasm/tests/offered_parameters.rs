//! Every parameter a control offers has to reach the rule and move a number.
//!
//! The interface draws a control for a registry parameter when that parameter carries
//! published values or a default, so the same rule decides what this file sweeps. When the
//! name a rule reads and the name the registry publishes drift apart, the value is dropped
//! on the floor, the rule runs its own instead, and the record still reports the value the
//! user picked. Nothing else in the suite sees that, because every number stays plausible.
//!
//! A rule's own parameters are half of what a caller states. The other half are the
//! operators composed onto it, which the registry files as entries in their own right with
//! their own citations and their own defaults, and which carry the parameters this project
//! argues hardest about: the backtrack whose notes say omitting it "is not choosing 0 ms, it
//! is failing to implement the cited method". Those are swept here on the same terms.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    run, AnalysisRequest, AnalysisResponse, Binding, MethodChoice, WeighingChoice, BINDINGS,
    ONSET_OPERATOR_IDS, TAKEOFF_OPERATOR_IDS,
};
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::schema::Parameter;
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

/// The demonstration jump preceded by a deliberate preload: the athlete pushes down into the
/// plate for sixty milliseconds before unweighting.
///
/// The window an inverse-threshold rule looks back over decides nothing unless there is an
/// excursion the other side of the band to find, and the demonstration trace holds no sample
/// above the upper edge before its crossing at all. So every lookback from three milliseconds
/// to sixteen seconds returns one onset, and the parameter reads as inert while being wired
/// correctly. The registry says as much on `onset.op.backtrack_to_tolerance` itself: the
/// recording that would settle its default is a countermovement jump preceded by a deliberate
/// preload. Here the preload ends 188 ms before the crossing, so the published 100 ms lookback
/// passes over it and this build's 500 ms default finds it, the retreat fires from the
/// preload's peak instead of the crossing, and onset moves.
fn jump_preceded_by_a_deliberate_preload() -> Trial {
    let demonstration = synthetic_countermovement_jump();
    let rate = demonstration.sample_rate_hz();
    let mut force = demonstration.force().to_vec();
    let standing = standing_newtons(&force, rate);
    let preload_start = (3.20 * rate) as usize;
    let preload_samples = (0.060 * rate) as usize;
    for offset in 0..preload_samples {
        // A rise and a return rather than a step, so the excursion is the shape of somebody
        // loading the plate and not an edge the peak search would sit on. Ten percent of
        // system weight clears an upper band edge that sits 3 N above it at every published
        // multiplier, and stays far below the 1600 N propulsive peak that bounds the search.
        let through = (offset + 1) as f64 / preload_samples as f64;
        force[preload_start + offset] =
            standing * (1.0 + 0.10 * (through * std::f64::consts::PI).sin());
    }
    Trial::new(force, rate).expect("the modified trace is still a trial")
}

/// One control the interface draws, and the values this file drives it through.
struct OfferedParameter {
    slot: &'static str,
    /// The rule a request names to reach this parameter, which is always a binding. An
    /// operator is never named: it arrives because the rule the caller picked composed it.
    method_id: String,
    /// The registry entry that publishes this parameter, which for an operator is not the
    /// entry the request named. A reader looks the value up under this id, so this is the
    /// row a sweep has to prove ran.
    declared_by: String,
    parameter: String,
    probes: Vec<f64>,
}

/// The values this sweep drives one registry parameter through, or nothing where it carries
/// neither a published value nor a default and so has no control and nothing to bind.
///
/// One home for that rule, because the interface's decision to draw a control and this
/// file's decision to sweep one have to be the same decision. A parameter that varies by
/// name rather than by number carries `named_values` instead and is not offered here.
fn probes_for_registry_parameter(parameter: &Parameter) -> Option<Vec<f64>> {
    let published: Vec<f64> = parameter
        .published_values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if published.is_empty() && parameter.default.is_none() {
        return None;
    }
    Some(probes_for(&published, parameter.default))
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
            let Some(probes) = probes_for_registry_parameter(parameter) else {
                continue;
            };
            offered.push(OfferedParameter {
                slot: binding.slot,
                method_id: binding.id.to_string(),
                declared_by: binding.id.to_string(),
                parameter: parameter.name.clone(),
                probes,
            });
        }
    }
    offered
}

/// The operator entries one binding composes, read off a real result rather than listed.
///
/// A hand-written list of operator ids goes stale the moment a rule composes one more, and
/// nothing says so: the sweep keeps passing over the operators it already knew. `quality.rs`
/// carried an array whose doc comment claimed a match forced it to stay exhaustive, there was
/// no match, and a status was added and never added to the array.
fn operators_composed_by(
    trial: &Trial,
    binding: &Binding,
    stating: Option<(&str, f64)>,
) -> BTreeSet<String> {
    let is_a_binding: BTreeSet<&str> = BINDINGS.iter().map(|binding| binding.id).collect();
    let parameters = stating
        .map(|(name, value)| BTreeMap::from([(name.to_string(), value)]))
        .unwrap_or_default();
    let Ok(response) = run(
        trial,
        &request_stating(binding.slot, binding.id, parameters),
    ) else {
        return BTreeSet::new();
    };
    response
        .bound_methods
        .iter()
        .filter(|bound| !is_a_binding.contains(bound.method_id.as_str()))
        .map(|bound| bound.method_id.clone())
        .collect()
}

/// What one walk of every binding found about the operators composed onto them.
struct ComposedOperators {
    /// Every operator entry reached, including those publishing no parameter to sweep. Six
    /// of them vary by name rather than by number, so a set built from swept parameters
    /// alone would report two thirds of the operators as unreachable.
    reached: BTreeSet<String>,
    /// The parameters those entries offer a control for, crossed with the binding that
    /// composes them.
    offered: Vec<OfferedParameter>,
}

/// Every binding crossed with the operators it composes, crossed with the parameters those
/// operators publish a value or a default for.
///
/// Two rounds, because one is not enough. A rule composes most of its operators unasked, and
/// those a bare run reports. `onset.op.search_floor` is composed only once a caller states a
/// floor, its alternative running otherwise, so a bare run reports it as absent while its
/// parameter is exactly the shape this sweep exists for: published values, a default, and
/// required. Stating the name and asking again is what reaches it.
///
/// Both answers come out of the same runs rather than two walks, because two walks are free
/// to disagree about which operators ran.
fn composed_operators() -> ComposedOperators {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let trial = synthetic_countermovement_jump();
    let mut reached = BTreeSet::new();
    let mut offered = Vec::new();

    for binding in BINDINGS {
        let composed_unasked = operators_composed_by(&trial, binding, None);
        reached.extend(composed_unasked.iter().cloned());
        for operator in ONSET_OPERATOR_IDS.iter().chain(TAKEOFF_OPERATOR_IDS) {
            let Some(entry) = loaded.registry.methods.get(*operator) else {
                continue;
            };
            // An operator belongs to the construct it operates on. Onset and takeoff run on
            // every request, so their entries appear in the record of a run that was sweeping
            // a different construct's binding entirely, and binding one there would state a
            // name on a rule that never had it.
            if entry.construct != binding.construct {
                continue;
            }
            for parameter in &entry.parameters {
                let Some(probes) = probes_for_registry_parameter(parameter) else {
                    continue;
                };
                let composed = composed_unasked.contains(*operator)
                    || operators_composed_by(&trial, binding, Some((&parameter.name, probes[0])))
                        .contains(*operator);
                if !composed {
                    continue;
                }
                reached.insert((*operator).to_string());
                offered.push(OfferedParameter {
                    slot: binding.slot,
                    method_id: binding.id.to_string(),
                    declared_by: (*operator).to_string(),
                    parameter: parameter.name.clone(),
                    probes,
                });
            }
        }
    }
    ComposedOperators { reached, offered }
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
    let mut stated = siblings_the_rule_cannot_run_without(&offered.method_id);
    stated.insert(offered.parameter.clone(), value);
    request_stating(offered.slot, &offered.method_id, stated)
}

/// Every other number the rule under test states required and publishes no default for.
///
/// A rule needing two stated values declines at every probe of either one, so the sweep sees
/// one answer and reports a control that moves nothing. That is the sweep failing to reach the
/// question rather than the control failing to matter, and it reads identically. The first
/// rules requiring more than one such value arrived on 2026-08-04 and five parameters across
/// five of them read as inert at once, which is what this answers. The probed name is written
/// over whatever lands here, so the value under test is always the caller's.
fn siblings_the_rule_cannot_run_without(method_id: &str) -> BTreeMap<String, f64> {
    plateforce_analysis::binding::required_numbers(method_id)
        .iter()
        .map(|(name, value)| ((*name).to_string(), *value))
        .collect()
}

/// One rule named in its slot, carrying whatever the caller stated on it. An operator's
/// parameter goes on the binding's own map, because a request names rules and never names
/// the operators a rule composes.
fn request_stating(
    slot: &str,
    method_id: &str,
    parameters: BTreeMap<String, f64>,
) -> AnalysisRequest {
    let mut request = base_request();
    let choice = MethodChoice {
        method_id: method_id.to_string(),
        parameters,
        // An enumeration the rule states required with no default declines at every probe the
        // same way a number does, so it is answered here rather than left for the sweep to
        // read as a control that moves nothing.
        options: plateforce_analysis::binding::required_options(method_id)
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        ..Default::default()
    };
    match slot {
        "weighing" => {
            request.weighing.method_id = method_id.to_string();
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

/// Both populations, counted apart and swept together. A rule's own parameters and the
/// parameters of the operators composed onto it are separate registry entries answering to
/// separate citations, so a total over the two would hide either going to zero.
fn every_offered_parameter() -> Vec<OfferedParameter> {
    let mut offered = offered_parameters();
    offered.extend(composed_operators().offered);
    offered
}

#[test]
fn every_parameter_a_control_offers_reaches_the_rule_it_belongs_to() {
    let trial = synthetic_countermovement_jump();
    let bound_rule_parameters = offered_parameters();
    let operator_parameters = composed_operators().offered;
    assert!(
        bound_rule_parameters.len() >= 8,
        "{} parameters on bound rules were swept, which is fewer than the rules this build runs, so the sweep has stopped covering the interface",
        bound_rule_parameters.len()
    );
    // A floor rather than a check that the list is non-empty. The operators are composed
    // rather than named, so a change that stopped composing them would leave this sweep
    // walking nothing and reporting that everything it walked was fine.
    assert!(
        operator_parameters.len() >= 11,
        "{} parameters on composed operators were swept, which is fewer than the operators this build composes, so the sweep no longer reaches them",
        operator_parameters.len()
    );

    for parameter in bound_rule_parameters.iter().chain(&operator_parameters) {
        let outcome = run(&trial, &request_with(parameter, parameter.probes[0]))
            .unwrap_or_else(|error| panic!("{} could not run: {error}", parameter.method_id));
        assert!(
            outcome
                .bound_methods
                .iter()
                .any(|method| method.method_id == parameter.declared_by),
            "{} bound nothing, so '{}' was checked against a rule that did not run",
            parameter.declared_by,
            parameter.parameter
        );
        // Every row, not the row that declares the name. A name nobody read is reported once,
        // against the rule the request named, and an operator's own row never carries one.
        // Asked of the operator's row this assertion could not fail.
        let dropped_by = outcome
            .bound_methods
            .iter()
            .find(|method| method.unread_parameters.contains(&parameter.parameter));
        assert!(
            dropped_by.is_none(),
            "{} offers '{}' on {} and {} does not read it, so the value is dropped and the rule runs its own",
            parameter.slot,
            parameter.parameter,
            parameter.declared_by,
            parameter.method_id
        );
    }
}

/// The operators a sweep walks are exactly the operators this build says it composes.
///
/// Two directions, and each catches what the other cannot. An operator composed but not
/// declared is the `quality.rs` shape, where a list stopped matching the code and nothing
/// said so. An operator declared but reached by nothing is what a renamed parameter looks
/// like from here: `onset.op.search_floor` is composed only when its own parameter is
/// stated, so a name that drifted would take the entry out of the sweep in silence and every
/// remaining assertion would keep passing.
#[test]
fn the_operators_this_sweep_walks_are_the_operators_this_build_declares() {
    let declared: BTreeSet<&str> = ONSET_OPERATOR_IDS
        .iter()
        .chain(TAKEOFF_OPERATOR_IDS)
        .copied()
        .collect();

    let walked = composed_operators();
    let undeclared: Vec<&String> = walked
        .reached
        .iter()
        .filter(|id| !declared.contains(id.as_str()))
        .collect();
    assert!(
        undeclared.is_empty(),
        "{} of {} entries composed onto a rule are not among the {} this build declares, so the declared list has stopped describing what runs: {undeclared:?}",
        undeclared.len(),
        walked.reached.len(),
        declared.len()
    );

    let unreached: Vec<&&str> = declared
        .iter()
        .filter(|id| !walked.reached.contains(**id))
        .collect();
    assert!(
        unreached.is_empty(),
        "{} of {} declared operators were composed by no binding in this sweep, so a rename would take one out of the sweep rather than turning it red: {unreached:?}",
        unreached.len(),
        declared.len()
    );

    // What this sweep covers, as a query rather than a figure written down somewhere. The two
    // populations are counted apart and never added: a rule's own parameters and the
    // parameters of the operators composed onto it answer to different registry entries.
    println!(
        "{} parameters over {} bound rules, and {} parameters over {} of {} declared operator entries",
        offered_parameters().len(),
        BINDINGS.len(),
        walked.offered.len(),
        walked.reached.len(),
        declared.len(),
    );
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
        (
            "a jump preceded by a deliberate preload",
            jump_preceded_by_a_deliberate_preload(),
        ),
    ];

    let offered = every_offered_parameter();
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
