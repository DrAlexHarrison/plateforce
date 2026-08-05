//! An operator entry the registry publishes is a choice a caller can make, or the registry
//! is offering something nothing behind it will take.
//!
//! An operator is never selected as a rule. `--takeoff takeoff.op.crossing_selection` refuses
//! in exactly the words `--takeoff takeoff.op.no_such_thing` refuses in, so selection cannot
//! tell a live operator from a dead one and any check built on it would be measuring the
//! wrong thing. What reaches an operator is stating a parameter it declares: the rule reads
//! the name, `bound_with_operators` splits the value onto the operator's own row, and the
//! record names the entry beside the number it moved.
//!
//! So this states every name every operator declares, against every rule that could compose
//! one, and reads back which entries the record named. A name no rule reads comes back in
//! `unread_parameters` instead, which is the honest answer the software already gives and the
//! signal this reads.
//!
//! `takeoff.op.hysteresis` is the one entry no rule reaches, ruled AI-LEAD to keep its place:
//! it carries a published citation with two stated thresholds, an unbound entry is the
//! ordinary state of a registry this build runs a fraction of, and the two ways to make the
//! count read whole are deleting a published method or writing a rule nobody asked for.
//! Naming it here rather than excluding the family is what makes a fourteenth unreachable
//! operator fail instead of joining it quietly.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::Trial;
use plateforce_registry::Registry;

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// The one operator no rule reads, with the ruling that keeps it. A list rather than a
/// filter on the family, so an operator that goes dark tomorrow fails here.
const UNREACHED_BY_RULING: &[&str] = &["takeoff.op.hysteresis"];

/// The recordings this sweeps, because one recording exercises only what it happens to
/// contain. Measured: `onset.threshold.last_within_band` reads the bound its operator owns on
/// the committed trial and not on the synthetic one, so a sweep over the synthetic trace alone
/// reports a live operator as unreachable.
fn recordings() -> Vec<Trial> {
    let (subject01_trial1, _) = plateforce_core::read::read_trial_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
        ),
        '\t',
        0,
        SAMPLE_RATE_HZ,
    )
    .expect("the committed trial reads");
    vec![a_jump_that_lands(), subject01_trial1]
}

/// Quiet stance, an unweighting dip, a push, flight, then a landing, so every landmark is
/// placed and the takeoff rules that read the trace after flight have something to read.
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

/// Every rule this build binds for one construct, so the sweep is over what the build can
/// run rather than over a list somebody keeps up to date.
fn rules_for(construct: &str) -> Vec<&'static str> {
    plateforce_analysis::binding::BINDINGS
        .iter()
        .filter(|binding| binding.construct == construct && !binding.id.contains(".op."))
        .map(|binding| binding.id)
        .collect()
}

#[test]
fn an_operator_entry_is_reachable_by_a_name_a_rule_reads() {
    let registry = Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the shipped registry loads");
    let recordings = recordings();

    let operators: BTreeSet<&str> = registry
        .methods
        .values()
        .filter(|method| method.id.contains(".op."))
        .map(|method| method.id.as_str())
        .collect();

    // One operator at a time. Stating every operator's names at once overrides the very
    // composition a rule is defined as: `onset.threshold.last_within_band` is
    // `onset.op.crossing_selection` bound to `last`, and a sweep that also states
    // `selection = first` unbinds it and then reports the search bound it would have
    // consulted as a name no rule reads. That reading cost an hour and is the reason this
    // probes each entry alone.
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    let mut names_some_rule_read: BTreeSet<String> = BTreeSet::new();
    let mut named_by_the_record: BTreeSet<String> = BTreeSet::new();

    for operator in registry.methods.values().filter(|m| m.id.contains(".op.")) {
        let mut numbers = BTreeMap::new();
        let mut options = BTreeMap::new();
        for parameter in &operator.parameters {
            if let Some(first) = parameter.named_values.first() {
                options.insert(parameter.name.clone(), first.key.clone());
            } else {
                numbers.insert(
                    parameter.name.clone(),
                    parameter
                        .published_values
                        .first()
                        .copied()
                        .or(parameter.default)
                        .unwrap_or(1.0),
                );
            }
        }
        let declares: BTreeSet<&str> = operator
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        let on_onset = operator.construct == plateforce_analysis::binding::ONSET_CONSTRUCT;

        for recording in &recordings {
            for onset_id in rules_for(plateforce_analysis::binding::ONSET_CONSTRUCT) {
                for takeoff_id in rules_for(plateforce_analysis::binding::TAKEOFF_CONSTRUCT) {
                    let request = AnalysisRequest {
                        weighing: WeighingChoice {
                            method_id: "bwepoch.fixed_window".into(),
                            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
                            ..Default::default()
                        },
                        onset: MethodChoice {
                            method_id: onset_id.into(),
                            parameters: if on_onset { numbers.clone() } else { BTreeMap::new() },
                            options: if on_onset { options.clone() } else { BTreeMap::new() },
                            ..Default::default()
                        },
                        takeoff: MethodChoice {
                            method_id: takeoff_id.into(),
                            parameters: if on_onset { BTreeMap::new() } else { numbers.clone() },
                            options: if on_onset { BTreeMap::new() } else { options.clone() },
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    // A combination the build refuses carries no bound methods, which is a
                    // reading this sweep does not get rather than a failure. The floor below
                    // is what keeps a sweep where every combination refused from passing.
                    let Ok(response) = run(recording, &request) else {
                        continue;
                    };
                    for bound in &response.bound_methods {
                        if bound.method_id.contains(".op.") && !bound.bound_parameters.is_empty() {
                            named_by_the_record.insert(bound.method_id.clone());
                        }
                        for (name, _) in &bound.bound_parameters {
                            if declares.contains(name.as_str()) {
                                reached.insert(operator.id.as_str());
                                names_some_rule_read.insert(name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // The floor. A sweep that read nothing reports the same empty failure list as a build
    // where every operator is reachable, so the population it actually reached is asserted.
    assert!(
        names_some_rule_read.len() >= 8,
        "{} names an operator declares were read by a rule across the sweep, over {} \
         operator entries, which is too few to have exercised the resolution this reads: \
         {names_some_rule_read:?}",
        names_some_rule_read.len(),
        operators.len()
    );
    assert!(
        named_by_the_record.len() >= 6,
        "the record filed values onto {} operator rows across the sweep, so the routing that \
         puts a value on the entry that owns it is not being exercised: {named_by_the_record:?}",
        named_by_the_record.len()
    );

    // Both namespaces, because a sweep that reached only onset would report every takeoff
    // operator as unreachable and read exactly like a build where they are.
    for namespace in ["onset.op.", "takeoff.op."] {
        assert!(
            named_by_the_record
                .iter()
                .any(|id| id.starts_with(namespace)),
            "no {namespace} entry was named by the record, so this sweep cannot see that \
             half of the family: {named_by_the_record:?}"
        );
    }

    // An entry is reached when a rule read one of the names it declares. Which entry the
    // record then files the value under is a separate promise, held by
    // `a_value_recorded_against_an_entry_is_findable_on_that_entry`; read as the verdict here
    // it would depend on whether either recording happened to make each composing rule fire,
    // which it does not for four live operators.
    let unreached: Vec<&str> = operators
        .iter()
        .copied()
        .filter(|id| !reached.contains(id))
        .collect();

    assert_eq!(
        unreached, UNREACHED_BY_RULING,
        "{} of {} operator entries publish a parameter no rule reads. An operator is reached \
         by stating a name it declares, so one nothing reads is a choice the registry offers \
         and no caller can make. Wire the rule that composes it, or delete the entry, or add \
         it to UNREACHED_BY_RULING with the ruling that keeps it.",
        unreached.len(),
        operators.len()
    );
}
