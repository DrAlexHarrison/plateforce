//! A named value a caller states on a composed operator is honoured or refused, never dropped.
//!
//! An operator is not free on every rule that composes it. `onset.op.search_upper_bound`
//! publishes four landmarks to search back from and `onset.threshold.last_within_band`
//! implements one, because searching back from a different landmark is a different rule. Such
//! a value used to be written straight to the record, which does not mark the name consulted,
//! so a caller asking for one of the other three had it dropped and the rule ran its own. The
//! name came back in `unread_parameters`, which is a report rather than an answer, and on
//! `takeoff.op.crossing_selection` the two answers the caller was choosing between are 843 ms
//! apart on 155 of 244 trials.
//!
//! Refusing is the answer. It names the operator that publishes the alternatives and the value
//! that does run, so a caller who asked for something this rule is not learns which rule to ask
//! instead.
//!
//! Which operators a rule composes is read off real runs rather than listed, for the reason
//! `offered_parameters.rs` gives: a written list of ids goes stale in silence.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    run, AnalysisRequest, MethodChoice, WeighingChoice, BINDINGS, ONSET_OPERATOR_IDS,
    TAKEOFF_OPERATOR_IDS,
};
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use plateforce_wasm::demo::synthetic_countermovement_jump;
use plateforce_wasm::registry_embed;

fn base_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
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
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        ..Default::default()
    }
}

fn request_naming(slot: &str, method_id: &str, option: Option<(&str, &str)>) -> AnalysisRequest {
    let mut request = base_request();
    let mut choice = MethodChoice {
        method_id: method_id.to_string(),
        ..Default::default()
    };
    if let Some((name, value)) = option {
        choice.options.insert(name.to_string(), value.to_string());
    }
    match slot {
        "weighing" => {
            request.weighing.method_id = method_id.to_string();
            request.weighing.options = choice.options;
        }
        "takeoff" => request.takeoff = choice,
        _ => request.onset = choice,
    }
    request
}

/// What one rule did with one named value a caller stated on it.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The rule read the name and ran the value.
    Honoured,
    /// The rule declined, naming the parameter, so the caller learns the value did not run.
    Refused,
    /// The rule never read the name. The value is gone and the rule ran its own instead.
    Dropped,
}

fn state(slot: &str, method_id: &str, name: &str, value: &str) -> Outcome {
    let trial = synthetic_countermovement_jump();
    let Ok(response) = run(
        &trial,
        &request_naming(slot, method_id, Some((name, value))),
    ) else {
        return Outcome::Refused;
    };
    // The parameter the refusal names, not the sentence it wrote. A rule declining over some
    // other name has not answered for this one, and matching on prose would say it had.
    let refused = response.refusals.iter().any(|declined| {
        plateforce_core::Refusal::from(declined.refusal.clone())
            .parameter
            .as_deref()
            == Some(name)
    });
    if refused {
        return Outcome::Refused;
    }
    // Every row, because the whole unread set is recorded against the rule the caller named
    // and an operator's own row carries none of it.
    let unread = response
        .bound_methods
        .iter()
        .any(|bound| bound.unread_parameters.iter().any(|held| held == name));
    if unread {
        Outcome::Dropped
    } else {
        Outcome::Honoured
    }
}

/// One rule, one parameter of an operator it composes, and what it did with each value the
/// registry publishes for that parameter.
struct Asked {
    method_id: String,
    operator: String,
    parameter: String,
    outcomes: Vec<(String, Outcome)>,
}

impl Asked {
    fn dropped(&self) -> bool {
        self.outcomes
            .iter()
            .any(|(_, outcome)| *outcome == Outcome::Dropped)
    }

    fn name(&self) -> String {
        format!(
            "{} on {} via {}",
            self.parameter, self.operator, self.method_id
        )
    }
}

/// Every rule crossed with the named parameters of the operators it actually composes, each
/// asked for every value the registry publishes.
fn every_named_value_asked_for() -> Vec<Asked> {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let trial = synthetic_countermovement_jump();
    let is_a_binding: BTreeSet<&str> = BINDINGS.iter().map(|binding| binding.id).collect();
    let mut asked = Vec::new();

    for binding in BINDINGS {
        let Ok(bare) = run(&trial, &request_naming(binding.slot, binding.id, None)) else {
            continue;
        };
        let composed: BTreeSet<String> = bare
            .bound_methods
            .iter()
            .filter(|bound| !is_a_binding.contains(bound.method_id.as_str()))
            .map(|bound| bound.method_id.clone())
            .collect();

        for operator in ONSET_OPERATOR_IDS.iter().chain(TAKEOFF_OPERATOR_IDS) {
            let Some(entry) = loaded.registry.methods.get(*operator) else {
                continue;
            };
            // Both halves are load-bearing and dropping either one reads as a finding. Onset
            // and takeoff run on every request, so every rule's record carries the other
            // construct's operators and `composed` alone attributes them to whatever rule was
            // being swept. Asking a weighing rule for a crossing selection is a question it is
            // right to leave unread, and without the construct check 26 such questions come
            // back looking like dropped values.
            if entry.construct != binding.construct || !composed.contains(*operator) {
                continue;
            }
            for parameter in &entry.parameters {
                if parameter.named_values.is_empty() {
                    continue;
                }
                asked.push(Asked {
                    method_id: binding.id.to_string(),
                    operator: (*operator).to_string(),
                    parameter: parameter.name.clone(),
                    outcomes: parameter
                        .named_values
                        .iter()
                        .map(|named| {
                            (
                                named.key.clone(),
                                state(binding.slot, binding.id, &parameter.name, &named.key),
                            )
                        })
                        .collect(),
                });
            }
        }
    }
    asked
}

/// The parameter of a composed operator that no rule in this build reads, so a caller stating
/// one of its values is told nothing.
///
/// `onset.op.backtrack_to_tolerance` files two things together: the lookback window that
/// triggers a retreat, which `onset.threshold.last_within_band` reads and runs, and the
/// tolerance the retreat walks back to. Core implements both retreats as `PostCrossingRule`
/// and `plateforce-conformance` selects the tolerance one, while every analysis rule takes the
/// fixed offset. So the entry is composed for its lookback and its tolerance is unreachable
/// through a request.
///
/// An equality rather than a permitted exception, so it fails in both directions: a new name
/// here is a value a caller can ask for and cannot get, and implementing this one makes the
/// assertion fail until the name comes out.
const NOT_REACHABLE_THROUGH_A_REQUEST: &[&str] =
    &["tolerance on onset.op.backtrack_to_tolerance via onset.threshold.last_within_band"];

#[test]
fn a_named_value_a_composed_operator_publishes_is_honoured_or_refused_and_never_dropped() {
    let asked = every_named_value_asked_for();

    // A floor rather than the absence of hits. These operators are composed rather than named,
    // so a change that stopped composing them would leave this sweep walking nothing and
    // reporting that everything it walked was fine.
    assert!(
        asked.len() >= 13,
        "{} named parameters on composed operators were asked for, which is fewer than the operators this build composes, so the sweep no longer reaches them",
        asked.len()
    );

    let dropped: Vec<String> = asked
        .iter()
        .filter(|entry| entry.dropped())
        .map(|entry| entry.name())
        .collect();
    let expected: Vec<String> = NOT_REACHABLE_THROUGH_A_REQUEST
        .iter()
        .map(|name| (*name).to_string())
        .collect();

    assert_eq!(
        dropped, expected,
        "{} of {} named parameters on composed operators end up dropped rather than honoured or refused, and a caller stating one of those is told nothing while the rule runs its own",
        dropped.len(),
        asked.len()
    );

    println!(
        "{} named parameters over the operators this build composes, {} of them reaching a rule",
        asked.len(),
        asked.len() - dropped.len()
    );
}
