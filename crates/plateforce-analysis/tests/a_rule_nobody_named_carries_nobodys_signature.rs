//! How a rule was chosen, recorded as one of four claims rather than as the reader's own.
//!
//! A rule the caller named, a rule accepted from the registry's recommendation, a rule a
//! published pipeline supplied, and a rule that ran because nobody said anything move the
//! number identically and answer different questions a methods section asks. The fourth used
//! to be recorded as the first, which put the reader's signature on 15 of the 18 rules a
//! request naming the three landmark rules runs.
//!
//! The population row at the bottom is the one that cannot be satisfied by a subset: it holds
//! every row of a plain analysis to the question of whether the request named it, and reports
//! its own denominator.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::request::preset_named;
use plateforce_analysis::{run, AnalysisRequest, BoundMethod, MethodChoice, WeighingChoice};
use plateforce_core::provenance::{ParameterSource, RegistryStamp};
use plateforce_core::Trial;
use plateforce_registry::{Citation, CitationRole, Preset, PresetBinding, Registry};

/// Quiet stance, an unweighting dip, a push, flight, then a landing. The same shape the other
/// provenance tests in this crate use, so a rule that runs there runs here.
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

/// A request naming a rule for each of the three landmark constructs and nothing else, which
/// is what every surface sends when a reader has picked their rules and left the rest alone.
fn named_landmarks() -> AnalysisRequest {
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

fn bound(request: &AnalysisRequest) -> Vec<BoundMethod> {
    run(&trial(), request)
        .expect("the fixture produces a result")
        .bound_methods
}

/// The claim this row leaves in the provenance a reader is handed, taken through
/// `into_provenance` rather than off the field beside it, so a mapping that lost the claim on
/// the way to the record fails here.
fn recorded_source(bound: &[BoundMethod], method_id: &str) -> ParameterSource {
    bound
        .iter()
        .find(|row| row.method_id == method_id)
        .unwrap_or_else(|| {
            panic!(
                "no row for {method_id}, and {} rows ran: {:?}",
                bound.len(),
                bound.iter().map(|row| &row.method_id).collect::<Vec<_>>()
            )
        })
        .into_provenance(&RegistryStamp::none(), false, Vec::new())
        .method_source
}

/// A value an interface filled and marked travels as the rule's own, and so does the word a
/// downstream rule inherits from it. The weighing's dispersion used to read presence in the
/// options map as the reader's statement, so a marked default put the reader's signature on
/// the onset rule's inherited `sd_convention`.
#[test]
fn a_marked_default_option_does_not_sign_the_inherited_convention() {
    let mut marked = named_landmarks();
    marked
        .weighing
        .options
        .insert("dispersion".into(), "sample".into());
    marked.weighing.from_registry_default = BTreeSet::from(["dispersion".to_string()]);
    let rows = bound(&marked);
    let onset = rows
        .iter()
        .find(|row| row.method_id == "onset.threshold.noise_relative")
        .expect("the onset rule is bound");
    assert_eq!(
        onset.parameter_sources.get("sd_convention"),
        Some(&ParameterSource::Assumed),
        "a dispersion the interface filled signed the inherited convention"
    );

    let mut stated = named_landmarks();
    stated
        .weighing
        .options
        .insert("dispersion".into(), "sample".into());
    let rows = bound(&stated);
    let onset = rows
        .iter()
        .find(|row| row.method_id == "onset.threshold.noise_relative")
        .expect("the onset rule is bound");
    assert_eq!(
        onset.parameter_sources.get("sd_convention"),
        Some(&ParameterSource::Stated),
        "a dispersion the reader typed lost their signature"
    );
}

/// Row one. The caller named this rule, so the record carries their signature and should.
#[test]
fn a_rule_the_caller_named_is_recorded_as_stated() {
    let bound = bound(&named_landmarks());
    for named in [
        "bwepoch.fixed_window",
        "onset.threshold.noise_relative",
        "takeoff.threshold.absolute_force",
    ] {
        assert_eq!(
            recorded_source(&bound, named),
            ParameterSource::Stated,
            "{named} is a rule the request named and the record does not credit the caller"
        );
    }
}

/// Row two. Taking the recommendation is an act somebody performed, and a different one from
/// picking the rule off a list.
#[test]
fn a_rule_accepted_from_the_recommendation_is_recorded_as_recommended() {
    let mut request = named_landmarks();
    request.onset.method_from_recommendation = true;
    let bound = bound(&request);

    assert_eq!(
        recorded_source(&bound, "onset.threshold.noise_relative"),
        ParameterSource::Recommended
    );
    assert_eq!(
        recorded_source(&bound, "takeoff.threshold.absolute_force"),
        ParameterSource::Stated,
        "a rule nobody recommended is recorded as accepted from a recommendation"
    );
}

/// Row three. A published pipeline stands behind the rule, and the caller chose the pipeline
/// by its id and its citation rather than choosing this rule.
#[test]
fn a_rule_a_published_pipeline_supplied_is_recorded_as_cited() {
    let mut registry = Registry::default();
    registry.presets.insert("under_test".into(), owen_shaped());
    let adopted = preset_named(&registry, "under_test").expect("the registry carries it");

    let mut request = named_landmarks();
    request.onset.method_id = String::new();
    request.weighing.method_id = String::new();
    request.adopt(adopted).expect("a rule this build runs");
    let bound = bound(&request);

    assert_eq!(
        recorded_source(&bound, "onset.threshold.noise_relative"),
        ParameterSource::Cited
    );
    assert_eq!(
        recorded_source(&bound, "takeoff.threshold.absolute_force"),
        ParameterSource::Stated,
        "a construct the pipeline states nothing about is attributed to it"
    );
}

/// Row four, in each of the three shapes it reaches the record by.
///
/// A construct nobody put on the path, an operator entailed by a rule the caller did name,
/// and a rule the spine runs for itself so that one id does not return two numbers on one
/// trial. Every one of them ran without anybody choosing it.
#[test]
fn a_rule_nobody_named_is_recorded_as_assumed() {
    let bound = bound(&named_landmarks());

    assert_eq!(
        recorded_source(&bound, "filter.none"),
        ParameterSource::Assumed,
        "the conditioning rule ran under the registry's declared default and the record reads \
         as though the reader chose it"
    );
    assert_eq!(
        recorded_source(&bound, "onset.op.backward_offset_fixed"),
        ParameterSource::Assumed,
        "an operator arrived with the rule that composes it and the record credits the reader \
         with picking it"
    );
    assert_eq!(
        recorded_source(&bound, "jumpheight.takeoff.impulse_momentum"),
        ParameterSource::Assumed,
        "the spine ran this rule for itself and the record credits the reader with picking it"
    );
}

/// The same row reached the other way: a caller who states a value against the conditioning
/// phase and names no rule for it. The values are theirs and the rule is not.
#[test]
fn stating_a_value_against_a_phase_does_not_name_its_rule() {
    let mut request = named_landmarks();
    request.conditioning.insert(
        "conditioned_force_signal".into(),
        MethodChoice {
            method_id: String::new(),
            options: BTreeMap::from([("passband_edge".to_string(), "none".to_string())]),
            ..Default::default()
        },
    );
    let bound = bound(&request);

    let row = bound
        .iter()
        .find(|row| row.method_id == "filter.none")
        .expect("the conditioning rule ran");
    assert_eq!(
        row.into_provenance(&RegistryStamp::none(), false, Vec::new())
            .method_source,
        ParameterSource::Assumed,
        "a caller who stated a value and named no rule is credited with picking the rule"
    );
    assert_eq!(
        row.parameter_sources.get("passband_edge"),
        Some(&ParameterSource::Stated),
        "the value the caller did state stopped being theirs"
    );
}

/// The population row. Every rule a plain analysis runs, sorted into the ones the request
/// named and the ones nobody did, with the denominator, so a row added to the pipeline is
/// covered here without an edit.
///
/// A rule the request named must be `Stated` and a rule it did not must not be, which is two
/// claims rather than one: an implementation that recorded every row as assumed would pass
/// half of this and fail the other half.
#[test]
fn every_rule_a_plain_analysis_runs_says_whether_anybody_picked_it() {
    let request = named_landmarks();
    let named: BTreeSet<&str> = BTreeSet::from([
        request.weighing.method_id.as_str(),
        request.onset.method_id.as_str(),
        request.takeoff.method_id.as_str(),
    ]);
    let bound = bound(&request);

    let mut signed_by_nobody = Vec::new();
    let mut unsigned_by_the_caller = Vec::new();
    for row in &bound {
        let source = row
            .into_provenance(&RegistryStamp::none(), false, Vec::new())
            .method_source;
        match named.contains(row.method_id.as_str()) {
            true if source != ParameterSource::Stated => {
                unsigned_by_the_caller.push((row.method_id.clone(), source))
            }
            false if source == ParameterSource::Stated => {
                signed_by_nobody.push(row.method_id.clone())
            }
            _ => {}
        }
    }

    assert!(
        signed_by_nobody.is_empty(),
        "{} of the {} rules this analysis ran carry the reader's signature on a rule the \
         request never named: {signed_by_nobody:?}",
        signed_by_nobody.len(),
        bound.len()
    );
    assert!(
        unsigned_by_the_caller.is_empty(),
        "{} of the {} rules the request named are recorded as somebody else's choice: \
         {unsigned_by_the_caller:?}",
        unsigned_by_the_caller.len(),
        named.len()
    );
    assert_eq!(
        named.len(),
        bound
            .iter()
            .filter(|row| named.contains(row.method_id.as_str()))
            .count(),
        "the request named {} rules and the record holds a different number of them, so this \
         guard is measuring a population the request did not ask for",
        named.len()
    );
    assert!(
        bound.len() > named.len(),
        "every rule this analysis ran was named by the request, so the unnamed case this guard \
         exists for is out of its reach"
    );
}

fn citation() -> Citation {
    Citation {
        key: "under_test".into(),
        role: CitationRole::Proposes,
        reference: "a published pairing".into(),
        doi: None,
        obtained: false,
    }
}

/// A window and an onset rule, stating nothing about takeoff, which is the shape the registry
/// ships and the shape that makes the third row's second assertion mean something.
fn owen_shaped() -> Preset {
    Preset {
        id: "under_test".into(),
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
                parameters: BTreeMap::from([("k".to_string(), 5.0)]),
                options: BTreeMap::new(),
                composed_from: None,
                note: None,
            },
        ],
        citations: vec![citation()],
        states_nothing_about: vec!["takeoff".into()],
    }
}
