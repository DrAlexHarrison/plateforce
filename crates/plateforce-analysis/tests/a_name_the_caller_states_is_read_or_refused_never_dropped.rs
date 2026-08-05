//! Every enumerated name a bound rule's entry publishes, stated at each of its values.
//!
//! A rule that records a name without consulting it answers the caller's word with its own
//! and lists the name under `unread_parameters`, which is a published choice a caller cannot
//! make wearing an honest record. The sweep below is over the registry rather than over a
//! list written here, so a name added to an entry is covered the day it lands and a rule
//! that stops reading one fails here rather than going quiet.
//!
//! Three outcomes are allowed and a fourth is not. The rule reads the value and records it as
//! the caller's; the rule declines it by name; or the whole analysis declines it by name. A
//! name that comes back in `unread_parameters`, or beside a value the caller did not write,
//! is the drop this file exists to catch.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::{run, AnalysisRequest, DeclaredDefaults, MethodChoice, WeighingChoice};
use plateforce_analysis::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::{RefusalCode, Trial};
use plateforce_registry::schema::Surfacing;
use plateforce_registry::Registry;

const SAMPLE_RATE_HZ: f64 = 1200.0;

fn registry() -> Registry {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the shipped registry loads")
}

/// Quiet stance, an unweighting dip, a push, flight, then a landing above the push, so every
/// landmark rule has something to find and the trailing-window rules have somewhere to trail.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

/// A request with one construct's rule replaced and one name stated on it.
///
/// The other two spine slots keep rules that run on this trace, because a request whose
/// takeoff declines leaves the onset rule nothing to search back from and the sweep would
/// read a dependency failure as a name that was refused.
fn stating(construct: &str, method_id: &str, name: Option<(&str, &str)>) -> AnalysisRequest {
    let options: BTreeMap<String, String> = name
        .into_iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    let mut request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
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
        ..Default::default()
    };
    match construct {
        WEIGHING_CONSTRUCT => {
            request.weighing.method_id = method_id.to_string();
            request.weighing.options = options;
        }
        ONSET_CONSTRUCT => {
            request.onset.method_id = method_id.to_string();
            request.onset.options = options;
        }
        TAKEOFF_CONSTRUCT => {
            request.takeoff.method_id = method_id.to_string();
            request.takeoff.options = options;
        }
        other => panic!("this sweep reaches the three spine constructs, not {other}"),
    }
    request.declared_from(std::sync::Arc::clone(&DECLARED));
    request
}

/// Read once. `stating` is called several hundred times across the sweep below, and the
/// declarations are the same registry's on every one of them.
static DECLARED: std::sync::LazyLock<std::sync::Arc<DeclaredDefaults>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(DeclaredDefaults::of(&registry())));

/// Which rules the sweep binds: every spine binding this build carries.
fn spine_rules() -> Vec<(&'static str, &'static str)> {
    plateforce_analysis::BINDINGS
        .iter()
        .filter(|binding| {
            [WEIGHING_CONSTRUCT, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT].contains(&binding.construct)
        })
        .map(|binding| (binding.construct, binding.id))
        .collect()
}

/// Every enumerated name this rule answers for, taken from the entries a run under it
/// records rather than from the one entry the caller named.
///
/// An operator is a registry entry in its own right and its names arrive on the rule that
/// composed it, so a population built from the named entry alone misses `bound`, `selection`
/// and `tolerance`, which is most of what this file is about. Entries the registry marks as
/// never a user's choice are left out, because a name no surface offers is not a choice a
/// caller was invited to make.
fn names_answered_for(
    registry: &Registry,
    construct: &str,
    method_id: &str,
) -> Vec<(String, Vec<String>)> {
    let Ok(response) = run(&a_jump_that_lands(), &stating(construct, method_id, None)) else {
        return Vec::new();
    };
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for bound in &response.bound_methods {
        let Some(entry) = registry.methods.get(&bound.method_id) else {
            continue;
        };
        if entry.construct != construct {
            continue;
        }
        let surfacing = entry.gui.as_ref().map(|gui| gui.surfacing);
        if surfacing == Some(Surfacing::NeverAUserChoice) || surfacing == Some(Surfacing::Refuse) {
            continue;
        }
        for parameter in &entry.parameters {
            if parameter.named_values.is_empty() {
                continue;
            }
            found.insert(
                parameter.name.clone(),
                parameter
                    .named_values
                    .iter()
                    .map(|value| value.key.clone())
                    .collect(),
            );
        }
    }
    found.into_iter().collect()
}

/// What became of one stated name.
#[derive(Debug, PartialEq, Eq)]
enum Answer {
    /// The rule read it and the record carries the caller's word under it.
    Read,
    /// The rule declined it by name, inside a result or as the whole analysis.
    Declined,
    /// The name reached no rule, or reached one that answered with its own value.
    Dropped(String),
}

/// Narrowed to the construct under test, because a name is not unique across the build:
/// `dispersion` is declared on a weighing rule, an onset rule and a takeoff rule, and
/// `selection` on two operator families. A scan over every bound row finds a neighbouring
/// construct's row carrying the same name at its own default and reads it as this rule's
/// answer, which is a green sweep over the wrong rule.
fn what_became_of(
    registry: &Registry,
    construct: &str,
    method_id: &str,
    name: &str,
    value: &str,
) -> Answer {
    let request = stating(construct, method_id, Some((name, value)));
    let response = match run(&a_jump_that_lands(), &request) {
        Ok(response) => response,
        // A refusal that names the parameter is the rule declining the value. One that does
        // not is the analysis failing for another reason, and the sweep says so rather than
        // counting it as a decline.
        Err(refusal) => {
            return if refusal.parameter.as_deref() == Some(name) {
                Answer::Declined
            } else {
                Answer::Dropped(format!(
                    "the analysis declined under {:?} without naming {name}: {}",
                    refusal.code,
                    refusal.message()
                ))
            }
        }
    };
    if response
        .refusals
        .iter()
        .any(|declined| refusal_from_rule(declined).parameter.as_deref() == Some(name))
    {
        return Answer::Declined;
    }
    let mut unread_by = Vec::new();
    let under = response.bound_methods.iter().filter(|bound| {
        registry
            .methods
            .get(&bound.method_id)
            .is_some_and(|entry| entry.construct == construct)
    });
    for bound in under {
        if bound.unread_parameters.iter().any(|held| held == name) {
            unread_by.push(bound.method_id.clone());
        }
        let Some((_, written)) = bound.bound_parameters.iter().find(|(held, _)| held == name)
        else {
            continue;
        };
        if written == value {
            // Recorded under the caller's own word, which is the claim a reader acts on.
            return match bound.parameter_sources.get(name) {
                Some(ParameterSource::Stated) => Answer::Read,
                other => Answer::Dropped(format!(
                    "{} recorded {name} = {written} as {other:?} rather than as the caller's",
                    bound.method_id
                )),
            };
        }
    }
    Answer::Dropped(if unread_by.is_empty() {
        format!("{method_id} answered nothing for {name}")
    } else {
        format!("{} listed {name} unread", unread_by.join(", "))
    })
}

/// The population this file sweeps, asked before anything is asserted about it.
///
/// Every assertion below compares a run against a run, so a sweep that reached no name at
/// all, or reached one construct's worth, would be green and empty. The registry is the one
/// side no filter here touches, so the count and the constructs are read off it.
#[test]
fn the_sweep_reaches_enumerated_names_on_every_spine_construct() {
    let registry = registry();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    let mut names = 0;
    for (construct, method_id) in spine_rules() {
        let answered = names_answered_for(&registry, construct, method_id);
        if !answered.is_empty() {
            reached.insert(construct);
        }
        names += answered.len();
    }
    println!("{names} enumerated names over {reached:?}");
    assert_eq!(
        reached.len(),
        3,
        "the sweep reaches enumerated names on {reached:?} alone, so every count in this file \
         is over a slice of the build while reading as a count over it"
    );
    assert!(
        names >= 20,
        "{names} enumerated names is fewer than the registry declares on the spine, so the \
         population this file sweeps has quietly narrowed"
    );
}

/// The property. Each published value of each name, stated on each rule that answers for it.
#[test]
fn every_published_value_of_every_offered_name_is_read_or_declined() {
    let registry = registry();
    let mut read = 0;
    let mut declined = 0;
    let mut dropped: Vec<String> = Vec::new();

    for (construct, method_id) in spine_rules() {
        for (name, keys) in names_answered_for(&registry, construct, method_id) {
            for key in keys {
                match what_became_of(&registry, construct, method_id, &name, &key) {
                    Answer::Read => read += 1,
                    Answer::Declined => declined += 1,
                    Answer::Dropped(why) => {
                        dropped.push(format!("{method_id} {name} = {key}: {why}"))
                    }
                }
            }
        }
    }

    println!(
        "{read} read, {declined} declined, {} dropped",
        dropped.len()
    );
    assert!(
        read + declined > 40,
        "{} values swept is fewer than the spine publishes, so this assertion is over the \
         wrong population",
        read + declined
    );
    assert!(
        read > 0 && declined > 0,
        "{read} read and {declined} declined: a sweep where every value lands on one arm \
         cannot tell a rule that reads its names from one that refuses all of them"
    );
    assert!(
        dropped.is_empty(),
        "{} stated values reached a rule that answered with its own:\n  {}",
        dropped.len(),
        dropped.join("\n  ")
    );
}

/// A read that does not move the number is a name recorded and not used, which is the same
/// silence one layer down.
///
/// `tolerance` is the sharpest case: the core has carried the retreat to system weight since
/// the rule was written, and no caller could reach it. The two retreats end at different
/// samples on a trace that unweights, or the choice is a word with no number behind it.
#[test]
fn the_retreat_a_caller_names_lands_somewhere_else_than_the_fixed_step() {
    let trial = a_jump_that_lands();
    let fixed = run(
        &trial,
        &stating(ONSET_CONSTRUCT, "onset.threshold.last_within_band", None),
    )
    .expect("the fixed step runs");
    let to_weight = run(
        &trial,
        &stating(
            ONSET_CONSTRUCT,
            "onset.threshold.last_within_band",
            Some(("tolerance", "at_system_weight")),
        ),
    )
    .expect("the retreat to system weight runs");

    println!(
        "fixed step placed onset at {:?}, retreat to system weight at {:?}",
        fixed.onset_index, to_weight.onset_index
    );
    assert!(
        fixed.onset_index.is_some() && to_weight.onset_index.is_some(),
        "one of the two retreats placed no onset, so the comparison is between a number and \
         nothing: {:?} against {:?}",
        fixed.onset_index,
        to_weight.onset_index
    );
    assert_ne!(
        fixed.onset_index, to_weight.onset_index,
        "both retreats placed onset at the same sample, so the name a caller states is \
         recorded and changes nothing"
    );
}

/// A bound the entry publishes and the rule runs without.
///
/// `retreat_cap_samples` is not a choice between names, so the sweep above cannot see it, and
/// it is the one name on this operator whose absence changes how far the retreat walks. The
/// entry's own notes say the published variant has no cap and can walk arbitrarily far back on
/// a noisy quiet phase, so a caller who states one and has it dropped gets exactly the walk
/// the cap was written to stop.
#[test]
fn a_retreat_cap_this_rule_runs_without_is_declined_by_name() {
    let trial = a_jump_that_lands();
    let retreating = || {
        stating(
            ONSET_CONSTRUCT,
            "onset.threshold.last_within_band",
            Some(("tolerance", "at_system_weight")),
        )
    };
    let mut capped = retreating();
    capped
        .onset
        .parameters
        .insert("retreat_cap_samples".to_string(), 50.0);

    let response = run(&trial, &capped).expect("the trial analyses");
    let declined: Vec<plateforce_core::Refusal> =
        response.refusals.iter().map(refusal_from_rule).collect();
    println!(
        "a stated cap: onset {:?}, {:?}",
        response.onset_index,
        declined
            .iter()
            .map(|refusal| refusal.message())
            .collect::<Vec<_>>()
    );

    let named = declined
        .iter()
        .find(|refusal| refusal.parameter.as_deref() == Some("retreat_cap_samples"));
    assert_eq!(
        named.map(|refusal| refusal.code),
        Some(RefusalCode::UnknownParameter),
        "a stated cap reached a rule that walks back uncapped without saying so: onset {:?}, \
         refusals {declined:?}",
        response.onset_index
    );
    assert_eq!(
        named.map(|refusal| refusal.method_id.as_str()),
        Some("onset.op.backtrack_to_tolerance"),
        "the refusal names a rule other than the operator that publishes the cap"
    );

    // Silence is the published walk, and stating nothing has to leave it exactly there. A
    // refusal that fired on every request would satisfy the assertion above and take the rule
    // out of the build.
    let uncapped = run(&trial, &retreating()).expect("the retreat runs");
    assert!(
        uncapped.onset_index.is_some() && uncapped.refusals.is_empty(),
        "declining a cap nobody stated would take the rule down on every request: onset {:?}",
        uncapped.onset_index
    );
}

/// The two rules that share `onset.threshold.noise_relative` answer a collapsed band the
/// same way, because a reader picking between them is picking a search direction rather than
/// a policy on a window with no spread.
#[test]
fn both_rules_under_one_entry_decline_a_band_with_no_width() {
    // A plate holding one bit-identical value through the weighing window, which is the state
    // the registry says collapses the band, then a jump.
    let mut force = vec![600.0; 1200];
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(2400.0, 240));
    let flat = Trial::new(force, SAMPLE_RATE_HZ).unwrap();

    for rule in [
        "onset.threshold.noise_relative",
        "onset.threshold.last_within_band",
    ] {
        let response =
            run(&flat, &stating(ONSET_CONSTRUCT, rule, None)).expect("the trial analyses");
        let declined = response
            .refusals
            .iter()
            .map(refusal_from_rule)
            .find(|refusal| refusal.slot.as_deref() == Some(ONSET_CONSTRUCT));
        println!(
            "{rule}: onset {:?}, {:?}",
            response.onset_index,
            declined.as_ref().map(|refusal| refusal.message())
        );
        assert_eq!(
            declined.map(|refusal| refusal.code),
            Some(RefusalCode::CollapsedBand),
            "{rule} placed an onset on a window with no spread rather than declining, so the \
             sample it reports was decided by the converter: onset {:?}",
            response.onset_index
        );
    }
}
