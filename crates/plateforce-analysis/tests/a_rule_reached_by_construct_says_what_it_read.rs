//! What the construct dispatch has to hold, on a trace built to exercise each of them.
//!
//! A request reaches a rule by naming its construct, so every registry entry at depth one is
//! addressable rather than only the three the spine reaches by their own field names. The
//! properties below are the ones that make the difference between a dispatch and a lookup
//! table.

use std::collections::BTreeMap;

use plateforce_analysis::spread::{run as sweep, Axis, SpreadRequest};
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{RefusalCode, Trial};

mod common;

/// A countermovement jump with a landing, so both window rules have something to place and
/// the landing sits above the propulsive peak. Peak force over the whole recording and peak
/// force over the jump are then different numbers, which is what makes the window a choice
/// rather than a formality.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    // The landing, higher than anything in the jump.
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, 1200.0).unwrap()
}

fn base() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
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
    })
}

fn naming(pairs: &[(&str, &str)]) -> AnalysisRequest {
    let mut request = base();
    for (construct, method_id) in pairs {
        request.derived.insert(
            (*construct).to_string(),
            MethodChoice {
                method_id: (*method_id).to_string(),
                ..Default::default()
            },
        );
    }
    // Again, because the slots above arrived after the read in `base`.
    common::prepared(request)
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

/// A request that names no construct gets the spine's own eleven quantities. The field is
/// additive or it is a change to every caller.
#[test]
fn a_request_naming_no_construct_reports_what_the_spine_reports() {
    let response = run(&a_jump_that_lands(), &base()).expect("the spine runs");
    assert!(response.derived_is_absent());
    assert_eq!(
        response.metrics.len(),
        11,
        "the spine's own eleven quantities: {:?}",
        response
            .metrics
            .iter()
            .map(|metric| metric.key.as_str())
            .collect::<Vec<_>>()
    );
    assert!(value(&response, "peak_force_newtons").is_none());
}

trait SpineOnly {
    fn derived_is_absent(&self) -> bool;
}

impl SpineOnly for AnalysisResponse {
    fn derived_is_absent(&self) -> bool {
        !self
            .bound_methods
            .iter()
            .any(|bound| bound.method_id.starts_with("force.peak"))
    }
}

/// Peak force is not one number, and which window it is taken over moves it further than any
/// parameter on the peak rule does.
#[test]
fn the_window_a_peak_is_taken_over_moves_it_further_than_the_peak_rule_does() {
    let trial = a_jump_that_lands();

    let over_the_jump = run(
        &trial,
        &naming(&[
            ("analysis_window", "window_end.takeoff.detected"),
            ("peak_force", "force.peak.gross"),
        ]),
    )
    .expect("the request is well formed");
    let two_seconds_from_onset = run(
        &trial,
        &naming(&[
            ("analysis_window", "window_end.fixed_duration.isometric"),
            ("peak_force", "force.peak.gross"),
        ]),
    )
    .expect("the request is well formed");

    let jump = value(&over_the_jump, "peak_force_newtons").expect("a peak over the jump");
    let fixed = value(&two_seconds_from_onset, "peak_force_newtons").expect("a peak over 2 s");
    println!("to takeoff {jump:.1} N, two seconds from onset {fixed:.1} N");
    assert!(
        (fixed - jump).abs() > 100.0,
        "the two windows gave {jump:.1} N and {fixed:.1} N, so this trace does not tell them apart"
    );
    // The fixed window runs past takeoff into the landing on this trace, so it reads the
    // larger of the two. Stated as the direction rather than only the size, because a
    // difference with no sign would pass if the rules were swapped.
    assert!(fixed > jump);
}

/// Each rule's number carries the rule that produced it, not the first rule declared for the
/// construct. Two rules report `peak_force_newtons` and a shared declaration would name one of
/// them on both results. The third is reached under its own construct, and is asked here beside
/// them so the property is read across every rule the pair of constructs holds.
#[test]
fn each_peak_rule_reports_its_own_arithmetic() {
    let trial = a_jump_that_lands();
    for (construct, id, key) in [
        ("peak_force", "force.peak.gross", "peak_force_newtons"),
        ("net_peak_force", "force.peak.net", "net_peak_force_newtons"),
        ("peak_force", "force.peak.estimator", "peak_force_newtons"),
    ] {
        let response = run(
            &trial,
            &naming(&[
                ("analysis_window", "window_end.takeoff.detected"),
                (construct, id),
            ]),
        )
        .expect("the request is well formed");
        let metric = response
            .metrics
            .iter()
            .find(|metric| metric.key == key)
            .unwrap_or_else(|| panic!("{id} reported no {key}"));
        assert_eq!(metric.computed_by.as_deref(), Some(id));
        // The chain names the rule that placed the window it read, so a reader can see which
        // stretch of the recording the maximum was taken over. That the chain names nothing
        // else is a different property, and it is held in `derived.rs` rather than here:
        // through this path one construct places samples, so everything placed is everything
        // read, and an assertion about the difference could not fail.
        assert!(
            metric
                .contributing_method_ids
                .contains(&"window_end.takeoff.detected".to_string()),
            "{id} did not name the rule that placed its window: {:?}",
            metric.contributing_method_ids
        );
    }
}

/// Gross and net differ by exactly one system weight, which is the registry's own claim about
/// the pair, and the two numbers arrive on one result rather than on two.
///
/// Asked as one analysis because that is what the constructs being separate buys a caller. Under
/// one construct a request carries one of them, so the difference the registry states could only
/// be read by running the trial twice and trusting that nothing else moved between the runs.
#[test]
fn net_is_gross_less_one_system_weight_on_one_result() {
    let trial = a_jump_that_lands();
    let both = run(
        &trial,
        &naming(&[
            ("analysis_window", "window_end.takeoff.detected"),
            ("peak_force", "force.peak.gross"),
            ("net_peak_force", "force.peak.net"),
        ]),
    )
    .expect("one analysis carries both");

    let gross_peak = value(&both, "peak_force_newtons").unwrap();
    let net_peak = value(&both, "net_peak_force_newtons").unwrap();
    let system_weight = value(&both, "system_weight_newtons").unwrap();
    println!("gross {gross_peak:.4} N, net {net_peak:.4} N, system weight {system_weight:.4} N");
    assert!((gross_peak - net_peak - system_weight).abs() < 1e-9);
}

/// The averaging estimator separates from the raw maximum in one direction only, and at the
/// width its entry publishes it does not separate at all. Both halves, because a rule that
/// always agreed and a rule that always differed would each satisfy one of them.
#[test]
fn the_averaging_estimator_agrees_at_its_published_width_and_reads_lower_above_it() {
    let trial = a_jump_that_lands();
    let window = ("analysis_window", "window_end.takeoff.detected");

    let gross = value(
        &run(
            &trial,
            &naming(&[window, ("peak_force", "force.peak.gross")]),
        )
        .unwrap(),
        "peak_force_newtons",
    )
    .unwrap();

    let at_the_default = value(
        &run(
            &trial,
            &naming(&[window, ("peak_force", "force.peak.estimator")]),
        )
        .unwrap(),
        "peak_force_newtons",
    )
    .unwrap();
    assert_eq!(
        at_the_default, gross,
        "the entry publishes averaging_window_seconds = 0, which is the raw maximum"
    );

    let mut widened = naming(&[window, ("peak_force", "force.peak.estimator")]);
    widened
        .derived
        .get_mut("peak_force")
        .unwrap()
        .parameters
        .insert("averaging_window_seconds".to_string(), 0.1);
    let averaged = value(&run(&trial, &widened).unwrap(), "peak_force_newtons").unwrap();
    println!(
        "raw {gross:.1} N, 0.1 s averaged {averaged:.1} N, gap {:.1} N",
        gross - averaged
    );
    assert!(averaged < gross);
}

/// A rule that needs a window and was given no choice of one says which choice is open, rather
/// than computing a number over a window nobody chose.
#[test]
fn a_peak_asked_for_with_no_window_chosen_says_which_choice_is_open() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[("peak_force", "force.peak.gross")]),
    )
    .expect("the request is well formed");

    let declined = response
        .refusals
        .iter()
        .find(|rule| rule.method_id == "force.peak.gross")
        .expect("the rule declined");
    let refusal = plateforce_analysis::document::refusal_from_rule(declined);
    println!("{}", refusal.message());
    assert_eq!(refusal.code, RefusalCode::DecisionNotMade);
    assert!(refusal.available.contains(&"analysis_window".to_string()));
    assert_eq!(declined.construct, "peak_force");
    assert!(value(&response, "peak_force_newtons").is_none());
}

/// The other half of the pair, and the two are different faults with different repairs. Here
/// the window was chosen and its rule produced nothing, so the remedy is upstream.
#[test]
fn a_peak_whose_chosen_window_placed_nothing_points_upstream() {
    // A trace with no takeoff at all, so the window rule that reads takeoff places nothing.
    let flat = Trial::new(vec![600.0; 2400], 1200.0).unwrap();
    let response = run(
        &flat,
        &naming(&[
            ("analysis_window", "window_end.takeoff.detected"),
            ("peak_force", "force.peak.gross"),
        ]),
    )
    .expect("the request is well formed");

    let declined = response
        .refusals
        .iter()
        .find(|rule| rule.method_id == "force.peak.gross")
        .expect("the peak rule declined");
    let refusal = plateforce_analysis::document::refusal_from_rule(declined);
    println!("{}", refusal.message());
    assert_eq!(refusal.code, RefusalCode::DependencyUnresolved);
    assert!(refusal.available.contains(&"analysis_window".to_string()));

    // And the window rule itself says the same thing about takeoff, so the chain of remedies
    // reads all the way back to the rule that failed.
    let window = response
        .refusals
        .iter()
        .find(|rule| rule.method_id == "window_end.takeoff.detected")
        .expect("the window rule declined");
    assert_eq!(
        plateforce_analysis::document::refusal_from_rule(window).code,
        RefusalCode::DependencyUnresolved
    );
}

/// A construct this build runs no rule for is refused by name, listing what it does run. The
/// alternative is a result missing the number that was asked for and saying nothing.
#[test]
fn a_construct_with_no_rule_behind_it_is_refused_by_name() {
    let error = run(
        &a_jump_that_lands(),
        &naming(&[("waveform_inference", "waveform.spm1d.pataky")]),
    )
    .expect_err("a construct with no rule is refused");
    println!("{error}");
    assert_eq!(error.code, RefusalCode::MethodNotImplemented);
    assert_eq!(error.method_id, "waveform_inference");
    assert!(
        error.available.iter().any(|name| name == "peak_force"),
        "{error}"
    );
}

/// An id that is a real rule filed under a different construct is refused too. Named for the
/// construct it was not filed under, it would match no binding and be skipped in silence.
#[test]
fn an_id_filed_under_another_construct_is_refused_rather_than_skipped() {
    let error = run(
        &a_jump_that_lands(),
        &naming(&[("peak_force", "window_end.takeoff.detected")]),
    )
    .expect_err("an id filed elsewhere is refused");
    println!("{error}");
    assert_eq!(error.code, RefusalCode::MethodNotImplemented);
    assert_eq!(error.method_id, "window_end.takeoff.detected");
    assert_eq!(error.slot.as_deref(), Some("peak_force"));
    assert!(
        error.available.iter().any(|id| id == "force.peak.gross"),
        "{error}"
    );
}

/// A construct is a sweep axis when the request carries it, which is what makes the spread
/// panel reach the rules this dispatch added.
#[test]
fn a_parameter_of_a_rule_reached_by_construct_sweeps() {
    let request = SpreadRequest {
        base: naming(&[
            ("analysis_window", "window_end.takeoff.detected"),
            ("peak_force", "force.peak.estimator"),
        ]),
        axes: vec![Axis {
            slot: "peak_force".into(),
            parameter: Some("averaging_window_seconds".into()),
            values: vec![0.0, 0.05, 0.1, 0.2],
            options: Vec::new(),
            method_ids: Vec::new(),
        }],
        quantity_key: "peak_force_newtons".into(),
        maximum_combinations: 512,
    };
    let response = sweep(&a_jump_that_lands(), &request).expect("a known axis sweeps");
    println!(
        "{} of {} variants succeeded, spread {:?} N",
        response.succeeded, response.combinations_run, response.spread_absolute
    );
    assert_eq!(response.succeeded, 4);
    assert!(
        response.spread_absolute.is_some_and(|spread| spread > 0.0),
        "the swept width did not move the peak"
    );
}

/// The method axis reaches them too, so a panel can put the three peak rules side by side.
#[test]
fn the_rules_of_a_construct_the_request_carries_sweep_against_each_other() {
    let request = SpreadRequest {
        base: naming(&[
            ("analysis_window", "window_end.takeoff.detected"),
            ("peak_force", "force.peak.gross"),
        ]),
        axes: vec![Axis {
            slot: "peak_force".into(),
            parameter: None,
            values: Vec::new(),
            options: Vec::new(),
            method_ids: vec!["force.peak.gross".into(), "force.peak.estimator".into()],
        }],
        quantity_key: "peak_force_newtons".into(),
        maximum_combinations: 512,
    };
    let response = sweep(&a_jump_that_lands(), &request).expect("a known axis sweeps");
    assert_eq!(response.succeeded, 2);
}

/// A construct the build runs a rule for and this request did not name is not an axis:
/// sweeping it would run a rule nobody chose. The refusal names what could have been swept.
#[test]
fn a_construct_the_request_did_not_name_is_refused_as_an_axis() {
    let request = SpreadRequest {
        base: naming(&[("analysis_window", "window_end.takeoff.detected")]),
        axes: vec![Axis {
            slot: "peak_force".into(),
            parameter: Some("averaging_window_seconds".into()),
            values: vec![0.0, 0.1],
            options: Vec::new(),
            method_ids: Vec::new(),
        }],
        quantity_key: "peak_force_newtons".into(),
        maximum_combinations: 512,
    };
    let refusal =
        sweep(&a_jump_that_lands(), &request).expect_err("an unnamed construct is refused");
    println!("{refusal}");
    // The axis is the name nothing on this request reads, so it arrives in `parameter` and a
    // caller reads it from there rather than out of the sentence.
    assert_eq!(refusal.code, RefusalCode::UnknownParameter);
    assert_eq!(
        refusal.parameter.as_deref(),
        Some("peak_force.averaging_window_seconds")
    );
    assert!(
        refusal
            .available
            .iter()
            .any(|axis| axis == "analysis_window"),
        "{refusal}"
    );
    // The count the sentence quotes is a number a caller branches on, and it is the
    // denominator the list of axes is taken over.
    assert_eq!(
        refusal.detail["axes_offered"],
        refusal.available.len() as f64
    );
}

/// A refusal that cannot cross the wire is a result without its method.
///
/// A rule that declines crosses as a record rather than as a sentence in `warnings`, so the
/// thirteen condition classes the R package publishes can be raised on a landmark rule. Every
/// field a caller branches on crosses as a field.
#[test]
fn a_declining_rule_crosses_the_wire_as_the_record_rather_than_the_sentence() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[("peak_force", "force.peak.gross")]),
    )
    .expect("the request is well formed");

    let wire: serde_json::Value = serde_json::to_value(&response).expect("the response serialises");
    let refusals = wire["refusals"].as_array().expect("refusals crossed");
    println!(
        "{}",
        serde_json::to_string_pretty(&wire["refusals"]).unwrap()
    );

    let declined = refusals
        .iter()
        .find(|refusal| refusal["method_id"] == "force.peak.gross")
        .expect("the peak rule's refusal crossed");
    assert_eq!(declined["code"], "decision_not_made");
    assert_eq!(declined["slot"], "peak_force");
    assert!(declined["message"]
        .as_str()
        .expect("a sentence")
        .contains("analysis_window"));
    assert!(declined["available"]
        .as_array()
        .expect("what is outstanding")
        .contains(&serde_json::Value::String("analysis_window".to_string())));
}

/// The control. An analysis where every rule ran carries an empty list rather than an
/// absent field, so a caller reads the same shape either way.
#[test]
fn an_analysis_where_every_rule_ran_carries_no_refusals() {
    let response = run(
        &a_jump_that_lands(),
        &naming(&[
            ("analysis_window", "window_end.takeoff.detected"),
            ("peak_force", "force.peak.gross"),
        ]),
    )
    .expect("the request is well formed");
    let wire: serde_json::Value = serde_json::to_value(&response).unwrap();
    assert_eq!(wire["refusals"].as_array().map(Vec::len), Some(0));
}
