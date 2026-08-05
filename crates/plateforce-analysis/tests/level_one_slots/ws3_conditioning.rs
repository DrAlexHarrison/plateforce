//! What was done to the signal, on the record, including when the answer is nothing.
//!
//! A tool that filters and does not say so publishes a number nobody can reproduce. A tool
//! that does not filter and does not say so publishes one nobody can tell from the first. So
//! `filter.none` is declared on every metric like any other rule.

use std::collections::BTreeMap;

use plateforce_analysis::{run, MethodChoice, BINDINGS};
use plateforce_core::provenance::ParameterSource;

use crate::common::{committed_trial, default_request, COMMITTED_TRIALS};

const CONDITIONING_ID: &str = "filter.none";
const CONSTRUCT: &str = "conditioned_force_signal";
const EDGE: &str = "passband_edge";

/// A request stating one name against the conditioning phase, without naming a rule for it.
/// Naming none is what a caller who wants the phase's own rule and their own value sends, and
/// it is the shape every surface builds.
fn stating(name: &str, value: &str) -> plateforce_analysis::AnalysisRequest {
    let mut request = default_request();
    request.conditioning.insert(
        CONSTRUCT.to_string(),
        MethodChoice {
            options: BTreeMap::from([(name.to_string(), value.to_string())]),
            ..Default::default()
        },
    );
    request
}

/// Every metric names the rule that conditioned the signal it was measured on, on a request
/// that asked for no conditioning at all, which is the common case.
#[test]
fn conditioning_is_always_declared() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    let request = default_request();
    assert!(
        request.conditioning.is_empty(),
        "this guard is about a request that asks for no conditioning"
    );

    let response = run(&trial, &request).expect("the default request runs");

    let undeclared: Vec<&str> = response
        .metrics
        .iter()
        .filter(|metric| {
            !metric
                .contributing_method_ids
                .iter()
                .any(|id| id == CONDITIONING_ID)
        })
        .map(|metric| metric.key.as_str())
        .collect();
    println!(
        "{} of {} metrics name what conditioned the signal they were measured on",
        response.metrics.len() - undeclared.len(),
        response.metrics.len()
    );
    assert!(
        undeclared.is_empty(),
        "{} of {} metrics travel with no conditioning declared: {undeclared:?}",
        undeclared.len(),
        response.metrics.len()
    );
    // The population this was written against, so a guard whose subject shrank cannot pass
    // by having less to read.
    assert!(
        response.metrics.len() >= 11,
        "only {} metrics were checked",
        response.metrics.len()
    );

    // And once on the bound-method list, so the rule is reported as a step that ran rather
    // than only as a name inside other rules' chains.
    let declared = response
        .bound_methods
        .iter()
        .filter(|bound| bound.method_id == CONDITIONING_ID)
        .count();
    assert_eq!(
        declared,
        1,
        "the conditioning rule is bound once and named once: {:?}",
        response
            .bound_methods
            .iter()
            .map(|bound| bound.method_id.as_str())
            .collect::<Vec<_>>()
    );
}

/// The rule that conditions the signal is declared ahead of every rule that reads it, in
/// each chain and in the bound-method list. A record that named them in the other order
/// would say the landmarks were placed on a signal they were not.
#[test]
fn what_conditioned_the_signal_is_named_before_what_read_it() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    let response = run(&trial, &default_request()).expect("the default request runs");

    assert_eq!(
        response.bound_methods.first().map(|b| b.method_id.as_str()),
        Some(CONDITIONING_ID),
        "the first rule on the record is not the one that produced the signal"
    );
    for metric in &response.metrics {
        assert_eq!(
            metric.contributing_method_ids.first().map(String::as_str),
            Some(CONDITIONING_ID),
            "{} names its chain out of order: {:?}",
            metric.key,
            metric.contributing_method_ids
        );
    }
}

/// The default is recorded as the software's choice rather than the caller's, so a reader
/// can tell a filter somebody picked from one nobody was asked about.
#[test]
fn a_conditioning_default_says_it_came_from_the_registry_rather_than_the_caller() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    let response = run(&trial, &default_request()).expect("the default request runs");

    let bound = response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == CONDITIONING_ID)
        .expect("the conditioning rule is on the record");
    assert!(
        bound.placed_by_hand_at_sample.is_none(),
        "nobody dragged anything to arrive at this"
    );
    // The rule ran and read a value, so the record carries what it read rather than an
    // empty binding under a rule's name.
    assert!(
        !bound.bound_parameters.is_empty(),
        "the conditioning rule is on the record with nothing recorded against it"
    );
}

/// The three answers one conditioning rule gives about the edge it reads, which is the whole
/// of what `filter.none` declares.
///
/// Unstated it is the rule's own and says so. Stated as the value the rule takes it keeps the
/// caller's signature, because a reader who wrote the edge down chose it and a record that
/// calls their choice an assumption loses the one fact this software exists to carry. Stated
/// as any other edge the run declines by name: `none` in a record beside a caller who asked
/// for a filter is the software answering a question it was not asked.
#[test]
fn the_edge_a_conditioning_rule_reads_is_recorded_however_the_caller_arrived() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);

    let source_of = |request| {
        let response = run(&trial, &request).expect("a request stating the edge this rule takes");
        let bound = response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == CONDITIONING_ID)
            .expect("the conditioning rule is on the record")
            .clone();
        assert!(
            bound.unread_parameters.is_empty(),
            "the rule read nothing the request stated: {:?}",
            bound.unread_parameters
        );
        *bound
            .parameter_sources
            .get(EDGE)
            .unwrap_or_else(|| panic!("{EDGE} is on the record: {:?}", bound.bound_parameters))
    };

    assert_eq!(
        source_of(default_request()),
        ParameterSource::Assumed,
        "an edge nobody stated is the rule's own"
    );
    assert_eq!(
        source_of(stating(EDGE, "none")),
        ParameterSource::Stated,
        "an edge the caller stated carries the caller's signature"
    );

    let refusal = run(&trial, &stating(EDGE, "20"))
        .expect_err("a caller asking this rule for a 20 Hz edge is asking it for a filter");
    assert_eq!(refusal.code, plateforce_core::RefusalCode::ValueNotAccepted);
    assert_eq!(refusal.method_id, CONDITIONING_ID);
    assert_eq!(refusal.parameter.as_deref(), Some(EDGE));
    assert_eq!(
        refusal.available,
        vec!["none".to_string()],
        "the refusal names what this rule does take"
    );
    println!("{}", refusal.message());
}

/// A conditioning rule that declines takes the whole run with it, rather than being noted
/// beside numbers that were computed anyway.
///
/// Every landmark below reads the signal this phase produces. A run that carried on would
/// place them on a signal no rule stands behind and publish the heights, with the refusal
/// filed as a footnote on a result that looks complete.
#[test]
fn a_conditioning_rule_that_declines_leaves_no_number_behind_it() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    assert!(
        run(&trial, &stating(EDGE, "20")).is_err(),
        "a declined conditioning rule returned a result"
    );
}

/// A name this rule does not read comes back as one it did not read, rather than being
/// dropped. The record says the caller wrote it and the rule never looked at it.
#[test]
fn a_name_the_conditioning_rule_does_not_read_comes_back_unread() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    let response = run(&trial, &stating("cutoff_hz", "20")).expect("an unread name runs");

    let bound = response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == CONDITIONING_ID)
        .expect("the conditioning rule is on the record");
    assert_eq!(
        bound.unread_parameters,
        vec!["cutoff_hz".to_string()],
        "a name no rule read is reported as unread"
    );
}

/// Putting the key in the map without naming a rule leaves the record a request that omits
/// the construct entirely leaves. The key buys somewhere to put the values, never a different
/// account of what ran.
#[test]
fn holding_a_place_for_values_without_naming_a_rule_records_the_run_unchanged() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    let mut unnamed = default_request();
    unnamed
        .conditioning
        .insert(CONSTRUCT.to_string(), MethodChoice::default());

    let record = |request| {
        let response = run(&trial, &request).expect("the request runs");
        format!("{:?}", response.bound_methods)
    };
    assert_eq!(record(unnamed), record(default_request()));
}

/// Naming the rule this phase runs anyway changes one thing about the record and nothing
/// else: who chose the rule.
///
/// The values, their sources and every other rule's row are identical, because the same rule
/// ran on the same signal. What differs is that one caller picked it and the other never
/// mentioned it, and a record that spelled both `stated` would put a reader's signature on a
/// rule they never saw.
#[test]
fn naming_the_conditioning_rule_credits_the_caller_and_changes_nothing_else() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);
    let mut named = default_request();
    named.conditioning.insert(
        CONSTRUCT.to_string(),
        MethodChoice {
            method_id: CONDITIONING_ID.to_string(),
            ..Default::default()
        },
    );

    let rows = |request| {
        run(&trial, &request)
            .expect("the request runs")
            .bound_methods
    };
    let picked = rows(named);
    let unmentioned = rows(default_request());

    assert_eq!(
        picked.len(),
        unmentioned.len(),
        "naming the rule the phase runs anyway changed which rules ran"
    );
    let mut differing = Vec::new();
    for (picked, unmentioned) in picked.iter().zip(unmentioned.iter()) {
        // Compared with the one field under test held equal, so this reports every other
        // difference rather than being satisfied by the one it expects.
        let mut levelled = picked.clone();
        levelled.method_source = unmentioned.method_source;
        assert_eq!(
            format!("{levelled:?}"),
            format!("{unmentioned:?}"),
            "naming the rule moved something other than the claim about who chose it"
        );
        if picked.method_source != unmentioned.method_source {
            differing.push((
                picked.method_id.clone(),
                picked.method_source,
                unmentioned.method_source,
            ));
        }
    }

    assert_eq!(
        differing,
        vec![(
            CONDITIONING_ID.to_string(),
            ParameterSource::Stated,
            ParameterSource::Assumed
        )],
        "one of the {} rules differs in who chose it, and it is the one the request named",
        picked.len()
    );
}

/// The table says which rules condition, and this build binds exactly the one the spec
/// names. A second conditioning rule arriving unbound to a construct would run nothing.
#[test]
fn every_conditioning_binding_fills_the_construct_the_registry_declares() {
    let conditioning: Vec<&str> = BINDINGS
        .iter()
        .filter(|binding| binding.construct == "conditioned_force_signal")
        .map(|binding| binding.id)
        .collect();
    println!("conditioning rules bound: {conditioning:?}");
    assert!(conditioning.contains(&CONDITIONING_ID));
    let registry = crate::common::registry();
    for id in &conditioning {
        assert!(
            registry.methods.contains_key(*id),
            "{id} is bound and the registry does not carry it"
        );
    }
}
