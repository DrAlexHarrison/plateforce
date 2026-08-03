//! What was done to the signal, on the record, including when the answer is nothing.
//!
//! A tool that filters and does not say so publishes a number nobody can reproduce. A tool
//! that does not filter and does not say so publishes one nobody can tell from the first.
//! Before this phase existed the software applied `filter.none` and said nothing, on every
//! metric it emitted.

use plateforce_analysis::{run, BINDINGS};

use crate::common::{committed_trial, default_request, COMMITTED_TRIALS};

const CONDITIONING_ID: &str = "filter.none";

/// Every metric names the rule that conditioned the signal it was measured on, on a request
/// that asked for no conditioning at all. That request is the common case and it is the one
/// the defect lived in.
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
        !bound.manual_override,
        "nobody dragged anything to arrive at this"
    );
    // The rule ran and read a value, so the record carries what it read rather than an
    // empty binding under a rule's name.
    assert!(
        !bound.bound_parameters.is_empty(),
        "the conditioning rule is on the record with nothing recorded against it"
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
