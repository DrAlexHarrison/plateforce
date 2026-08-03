//! A number a published pipeline produced carries the pipeline that produced it.
//!
//! The whole of what a preset buys over typing the same values is the record, so these are
//! tests about the record rather than about the numbers. A preset that filled a request and
//! left the provenance reading as though the caller typed it would produce results
//! indistinguishable from a caller having done so, which throws away the one fact worth
//! keeping.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::binding::{bindings_for_construct, executable_constructs};
use plateforce_analysis::request::preset_named;
use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::provenance::ParameterSource;
use plateforce_core::{RefusalCode, Trial};
use plateforce_registry::{Citation, CitationRole, Preset, PresetBinding, Registry};

/// Quiet stance, an unweighting dip, a push, flight, then a landing. The same shape the
/// other provenance tests in this crate use, so a rule that runs there runs here.
fn trial() -> Trial {
    let system_weight_newtons = 700.0;
    let sample_rate_hz = 1000.0;
    let stance_samples = 2000;
    let push_samples = 300;
    let flight_samples = 500;
    let mut force = Vec::new();
    force.extend(
        (0..stance_samples).map(|index| system_weight_newtons + ((index % 17) as f64 - 8.0) * 0.4),
    );
    force.extend(
        (0..push_samples)
            .map(|index| system_weight_newtons * (1.0 - 0.5 * index as f64 / push_samples as f64)),
    );
    force.extend(
        (0..push_samples)
            .map(|index| system_weight_newtons * (0.5 + 2.0 * index as f64 / push_samples as f64)),
    );
    force.extend(
        (0..flight_samples)
            .map(|index| ((index % 11) as f64 - 5.0) * system_weight_newtons * 0.0004),
    );
    force.extend(std::iter::repeat_n(
        system_weight_newtons * 2.4,
        push_samples,
    ));
    Trial::new(force, sample_rate_hz).expect("the fixture is a well formed trial")
}

/// A request naming the rule for every landmark and stating no value of its own, so every
/// value in the record came from a rule or from the pipeline under test.
fn bare_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
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
    }
}

fn citation() -> Citation {
    Citation {
        key: "owen2014".into(),
        role: CitationRole::Proposes,
        reference: "a published pairing".into(),
        doi: None,
        obtained: false,
    }
}

/// The shape `registry/presets/owen2014.toml` ships: a weighing window and an onset rule,
/// stating nothing about takeoff.
fn owen_shaped() -> Preset {
    Preset {
        id: "owen2014".into(),
        title: "A published pairing".into(),
        description: "A window and an onset rule.".into(),
        bindings: vec![
            PresetBinding {
                construct: "system_weight".into(),
                method_id: "bwepoch.fixed_window".into(),
                parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
                options: BTreeMap::new(),
                composed_from: None,
                note: None,
            },
            PresetBinding {
                construct: "movement_onset".into(),
                method_id: "onset.threshold.noise_relative".into(),
                parameters: BTreeMap::from([
                    ("k".to_string(), 5.0),
                    ("offset_ms".to_string(), 30.0),
                ]),
                options: BTreeMap::new(),
                composed_from: None,
                note: None,
            },
        ],
        citations: vec![citation()],
        states_nothing_about: vec!["takeoff".into()],
    }
}

fn bound_with(
    preset: &Preset,
    stated: BTreeMap<String, f64>,
) -> Vec<plateforce_analysis::BoundMethod> {
    let mut request = bare_request();
    request.onset.method_id = String::new();
    request.weighing.method_id = String::new();
    request.onset.parameters = stated;
    request
        .adopt(preset)
        .expect("the pipeline binds rules this build runs");
    run(&trial(), &request)
        .expect("the fixture produces a result")
        .bound_methods
}

fn row<'a>(
    bound: &'a [plateforce_analysis::BoundMethod],
    method_id: &str,
) -> &'a plateforce_analysis::BoundMethod {
    bound
        .iter()
        .find(|entry| entry.method_id == method_id)
        .unwrap_or_else(|| panic!("{method_id} ran"))
}

#[test]
fn a_value_the_pipeline_supplied_is_recorded_as_cited_rather_than_stated() {
    let bound = bound_with(&owen_shaped(), BTreeMap::new());
    let onset = row(&bound, "onset.threshold.noise_relative");

    assert_eq!(
        onset.parameter_sources.get("k"),
        Some(&ParameterSource::Cited),
        "the pipeline's own k reads as though the caller typed it"
    );
    assert_eq!(
        onset.preset.as_ref().map(|it| it.id.as_str()),
        Some("owen2014")
    );

    let chain = onset.into_provenance(None, None, false, Vec::new());
    assert_eq!(
        chain.method_source,
        ParameterSource::Cited,
        "a rule a pipeline named was not picked off a list by the reader"
    );
    assert_eq!(
        chain.preset.as_ref().map(|it| it.id.as_str()),
        Some("owen2014")
    );

    // A published pipeline is a choice somebody made, so a result resting on one leaves the
    // building. Only an unmade decision holds it back.
    assert!(!ParameterSource::Cited.taints_the_record());
}

#[test]
fn a_value_the_caller_stated_displaces_the_pipelines_and_the_record_carries_both() {
    let bound = bound_with(&owen_shaped(), BTreeMap::from([("k".to_string(), 3.0)]));
    let onset = row(&bound, "onset.threshold.noise_relative");

    let ran: BTreeMap<&str, &str> = onset
        .bound_parameters
        .iter()
        .map(|(name, shown)| (name.as_str(), shown.as_str()))
        .collect();
    assert_eq!(
        ran.get("k"),
        Some(&"3"),
        "the caller's value is the one that ran"
    );
    assert_eq!(
        onset.parameter_sources.get("k"),
        Some(&ParameterSource::Stated),
        "a value the caller typed is not attributed to the pipeline"
    );

    let adopted = onset
        .preset
        .as_ref()
        .expect("the pipeline still named the rule");
    assert_eq!(adopted.id, "owen2014");
    assert_eq!(
        adopted.superseded_parameters.get("k"),
        Some(&5.0),
        "the record does not say what the pipeline published for the name that was replaced"
    );
    assert!(adopted.was_overridden());
    assert_eq!(adopted.superseded_names(), vec!["k".to_string()]);
}

#[test]
fn a_construct_the_source_is_silent_about_is_not_attributed_to_it() {
    let bound = bound_with(&owen_shaped(), BTreeMap::new());
    let takeoff = row(&bound, "takeoff.threshold.absolute_force");
    assert!(
        takeoff.preset.is_none(),
        "a pipeline that states nothing about takeoff has its name on the takeoff rule"
    );
    assert_eq!(
        takeoff
            .into_provenance(None, None, false, Vec::new())
            .method_source,
        ParameterSource::Stated
    );
}

/// The onset binding names one composed rule and the composition splits it across the rows
/// the registry files separately. Only the rows the pipeline supplied a value for carry it.
#[test]
fn an_operator_the_pipeline_supplied_no_value_for_is_not_attributed_to_it() {
    let bound = bound_with(&owen_shaped(), BTreeMap::new());

    let offset = row(&bound, "onset.op.backward_offset_fixed");
    assert_eq!(
        offset.preset.as_ref().map(|it| it.id.as_str()),
        Some("owen2014"),
        "the pipeline published this offset and the record does not say so"
    );

    let direction = row(&bound, "onset.op.direction");
    assert!(
        direction.preset.is_none(),
        "a value the rule chose for itself is reported under a published author's name"
    );
}

/// The population guard. A construct that gains a rule after this was written is covered
/// without an edit, so a slot added later cannot record a pipeline's value as the caller's.
#[test]
fn every_construct_this_build_runs_records_the_pipeline_that_bound_it() {
    let constructs = executable_constructs();
    let mut checked = 0;
    let mut silent = Vec::new();

    for construct in &constructs {
        let Some(first) = bindings_for_construct(construct).next() else {
            continue;
        };
        let preset = Preset {
            id: "under_test".into(),
            title: "One construct".into(),
            description: "One binding.".into(),
            bindings: vec![PresetBinding {
                construct: (*construct).to_string(),
                method_id: first.id.to_string(),
                parameters: BTreeMap::new(),
                options: BTreeMap::new(),
                composed_from: None,
                note: None,
            }],
            citations: vec![citation()],
            states_nothing_about: Vec::new(),
        };

        let mut request = bare_request();
        request.adopt(&preset).expect("a rule this build runs");
        let target = match *construct {
            "system_weight" => request.weighing.preset.clone(),
            "movement_onset" => request.onset.preset.clone(),
            "takeoff" => request.takeoff.preset.clone(),
            other => request.derived.get(other).and_then(|it| it.preset.clone()),
        };
        checked += 1;
        match target {
            Some(adopted) => assert_eq!(adopted.id, "under_test"),
            None => silent.push((*construct).to_string()),
        }
    }

    assert!(
        silent.is_empty(),
        "{} of {} constructs this build runs take a pipeline's binding without recording it: {silent:?}",
        silent.len(),
        constructs.len()
    );
    assert_eq!(
        checked,
        constructs.len(),
        "the guard reached {checked} of the {} constructs this build runs, so it is measuring \
         a subset rather than the population",
        constructs.len()
    );
}

#[test]
fn naming_a_pipeline_this_registry_does_not_carry_is_refused_with_the_ones_it_does() {
    let mut registry = Registry::default();
    registry.presets.insert("owen2014".into(), owen_shaped());

    assert_eq!(preset_named(&registry, "owen2014").unwrap().id, "owen2014");

    let refusal = preset_named(&registry, "forcedecks").unwrap_err();
    assert_eq!(refusal.code, RefusalCode::MethodNotImplemented);
    assert_eq!(refusal.method_id, "forcedecks");
    assert_eq!(refusal.available, vec!["owen2014".to_string()]);
    assert_eq!(
        refusal.detail.get("presets_this_registry_carries"),
        Some(&1.0),
        "the count a caller branches on is inside the sentence rather than a field"
    );
    assert!(refusal.message().contains("owen2014"));
}

#[test]
fn a_rule_the_caller_named_in_a_slot_the_pipeline_binds_is_refused_rather_than_half_adopted() {
    let mut request = bare_request();
    request.onset.method_id = "onset.threshold.absolute_force".into();
    let refusal = request.adopt(&owen_shaped()).unwrap_err();

    assert_eq!(refusal.code, RefusalCode::ValueNotAccepted);
    assert_eq!(refusal.method_id, "owen2014");
    assert_eq!(refusal.parameter.as_deref(), Some("movement_onset"));
    assert_eq!(
        refusal.named_value.as_deref(),
        Some("onset.threshold.absolute_force")
    );
    assert_eq!(
        refusal.available,
        vec!["onset.threshold.noise_relative".to_string()]
    );
}

#[test]
fn a_binding_naming_a_rule_this_build_does_not_run_is_refused_by_name() {
    let mut preset = owen_shaped();
    preset.bindings.push(PresetBinding {
        construct: "movement_onset".into(),
        method_id: "onset.threshold.nothing_runs_this".into(),
        parameters: BTreeMap::new(),
        options: BTreeMap::new(),
        composed_from: None,
        note: None,
    });
    let mut request = bare_request();
    request.onset.method_id = String::new();
    request.weighing.method_id = String::new();

    let refusal = request.adopt(&preset).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::MethodNotImplemented);
    assert_eq!(refusal.method_id, "onset.threshold.nothing_runs_this");
    assert!(!refusal.available.is_empty());
}

/// Two requests reaching the same numbers, one by adopting a pipeline and one by typing its
/// values, are different records. Nothing downstream may collapse them into one.
#[test]
fn typing_a_pipelines_values_does_not_produce_the_record_adopting_it_produces() {
    let adopted = bound_with(&owen_shaped(), BTreeMap::new());
    let mut typed_request = bare_request();
    typed_request.onset.parameters =
        BTreeMap::from([("k".to_string(), 5.0), ("offset_ms".to_string(), 30.0)]);
    typed_request.weighing.parameters = BTreeMap::from([("duration".to_string(), 1.0)]);
    let typed = run(&trial(), &typed_request)
        .expect("the fixture produces a result")
        .bound_methods;

    let ran = |bound: &[plateforce_analysis::BoundMethod]| -> BTreeSet<String> {
        row(bound, "onset.threshold.noise_relative")
            .bound_parameters
            .iter()
            .map(|(name, shown)| format!("{name}={shown}"))
            .collect()
    };
    assert_eq!(ran(&adopted), ran(&typed), "the two ran different values");
    assert!(row(&adopted, "onset.threshold.noise_relative")
        .preset
        .is_some());
    assert!(row(&typed, "onset.threshold.noise_relative")
        .preset
        .is_none());
    assert_eq!(
        row(&typed, "onset.threshold.noise_relative")
            .parameter_sources
            .get("k"),
        Some(&ParameterSource::Stated)
    );
}

/// A rule accepted from the registry's recommendation was not picked by the reader either.
/// The field was declared, serialised and fingerprinted while nothing read it.
#[test]
fn a_rule_accepted_from_the_recommendation_is_not_recorded_as_one_the_caller_stated() {
    let mut request = bare_request();
    request.onset.method_from_recommendation = true;
    let bound = run(&trial(), &request)
        .expect("the fixture produces a result")
        .bound_methods;

    let onset = row(&bound, "onset.threshold.noise_relative");
    assert_eq!(
        onset
            .into_provenance(None, None, false, Vec::new())
            .method_source,
        ParameterSource::Recommended
    );

    let takeoff = row(&bound, "takeoff.threshold.absolute_force");
    assert_eq!(
        takeoff
            .into_provenance(None, None, false, Vec::new())
            .method_source,
        ParameterSource::Stated,
        "a rule nobody recommended is recorded as accepted from a recommendation"
    );
}
