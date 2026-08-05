//! The chain of rules behind each number an analysis produced, and the account each number
//! gives of itself, which is written from that chain.
//!
//! A response says which rule computed a quantity and which rules its answer rests on, as two
//! flat lists. The tree those two lists describe is what a reader reads and what
//! `plateforce_core::reporting::fingerprint` takes, and it was being rebuilt at four sites: the
//! folder run's `provenance` relation, the R package's own reader, the R boundary's account of
//! each number, and the Python package. The four disagreed. Only the folder run carried the
//! arithmetic rule's own values, so the gravity behind the flight-time height reached a folder
//! run's record and no other. The Python package built two records for one number: a getter
//! returned a chain naming the whole pipeline and `value()` returned the same quantity with no
//! chain at all.
//!
//! Four derivations of one tree is the disease this project documents, one layer out from the
//! maths: not two implementations of a quantity, but four implementations of one derivation,
//! feeding a function none of them could call.

use std::collections::BTreeMap;

use plateforce_core::provenance::{ParameterRecord, ParameterSource, RegistryStamp};
use plateforce_core::reporting::describe;
use plateforce_core::{Measured, Provenance, ProvenanceChain};

use crate::binding::Dispatch;
use crate::resolution::BoundMethod;
use crate::response::{AnalysisResponse, Metric};
use crate::{BINDINGS, ONSET_CONSTRUCT, ONSET_OPERATOR_IDS, TAKEOFF_OPERATOR_IDS};

/// One reported quantity and the chain of rules behind it.
///
/// Keyed by the quantity rather than returned as a map, because a response may report one key
/// twice and a map would silently keep one of them. The order is the response's own.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricChain {
    pub quantity: String,
    pub chain: ProvenanceChain,
}

/// Where a rule sits in the order `run` resolves them: what conditions the signal, then what
/// weighs, then what places the landmarks, then what computes a quantity from them.
///
/// Read off the binding table rather than from the order a response happens to list ids in. A
/// rule added under a construct is ranked by the row that declares it, and a response whose
/// list order changed would not move a root.
fn rank(method_id: &str) -> u8 {
    match BINDINGS.iter().find(|binding| binding.id == method_id) {
        Some(binding) => match binding.dispatch {
            Dispatch::Conditioning(_) => 0,
            Dispatch::Spine if binding.construct == crate::WEIGHING_CONSTRUCT => 1,
            Dispatch::Spine => 2,
            Dispatch::Derived(_) => 3,
        },
        // A contributing id no row declares is ranked below everything, so it can never
        // displace a rule this build knows the position of.
        None => 0,
    }
}

/// Whether this rule is an operator composed onto a landmark rule rather than a step of its
/// own, and which construct it composes onto.
fn composes_onto(method_id: &str) -> Option<&'static str> {
    if ONSET_OPERATOR_IDS.contains(&method_id) {
        return Some(ONSET_CONSTRUCT);
    }
    if TAKEOFF_OPERATOR_IDS.contains(&method_id) {
        return Some(crate::TAKEOFF_CONSTRUCT);
    }
    None
}

/// The construct a rule fills, or nothing where no row declares it.
fn construct_of(method_id: &str) -> Option<&'static str> {
    BINDINGS
        .iter()
        .find(|binding| binding.id == method_id)
        .map(|binding| binding.construct)
}

/// One rule's record, with the records of the rules feeding it already built.
fn step(
    bound: &BoundMethod,
    registry: &RegistryStamp,
    acquisition_complete: bool,
    inputs: Vec<ProvenanceChain>,
) -> ProvenanceChain {
    let provenance = bound.into_provenance(
        registry,
        acquisition_complete,
        inputs
            .iter()
            .map(|input| input.provenance.clone())
            .collect(),
    );
    ProvenanceChain {
        enumerated_choices: bound.enumerated_choices(),
        provenance,
        depends_on: inputs,
    }
}

/// A rule the response named and left no bound record for, which still opens the chain.
///
/// Dropping it would put the rules under it beneath nothing, so a number whose arithmetic left
/// no record would come back claiming its landmarks produced it directly.
fn unbound_step(
    method_id: &str,
    registry: &RegistryStamp,
    acquisition_complete: bool,
    inputs: Vec<ProvenanceChain>,
) -> ProvenanceChain {
    // Destructured without a rest pattern, so a fact added to the stamp is a compile error here
    // rather than one this step quietly stops carrying.
    let RegistryStamp {
        version,
        declared_version,
        digest,
    } = registry.clone();
    let provenance = Provenance {
        registry_version: version,
        registry_declared_version: declared_version,
        registry_digest: digest,
        acquisition_complete,
        depends_on: inputs
            .iter()
            .map(|input| input.provenance.clone())
            .collect(),
        ..Provenance::of(method_id)
    };
    ProvenanceChain {
        provenance,
        enumerated_choices: Vec::new(),
        depends_on: inputs,
    }
}

/// The quantities whose number moves when the gravity the analysis was bound to moves.
///
/// A rule may record only a parameter its own registry entry declares, and of the twelve rules
/// reading this value one declares it, so for these the number that moved them reaches no
/// rule's row. `AnalysisResponse::bound_globals` carries it once for the whole analysis, and a
/// `Measured` travels away from the result it came out of, so the chain behind each number
/// carries it too.
///
/// Per quantity rather than per rule, because one rule can produce two numbers that rest on
/// different things: `impulse.net_vertical.as_performance_determinant` reports both the net
/// impulse, integrated over the interval directly, and the takeoff velocity, read off the
/// integrated series, and only the second moves with gravity. A record on the rule's row would
/// give the first a dependence it does not have.
///
/// Held to a measurement in both directions rather than trusted:
/// `every_number_the_analysis_gravity_moves_carries_it_in_its_chain` moves the gravity, reads
/// which numbers followed, and requires that set and this list to be the same set. That guard
/// runs one rule for every construct this build offers rather than the spine's alone, which is
/// what found the sixth entry below: it moves with gravity, it was on no surface's list, and two
/// analyses at two gravities gave it one fingerprint.
const QUANTITIES_RESTING_ON_THE_ANALYSIS_GRAVITY: &[&str] = &[
    "jump_height_from_flight_time_meters",
    "jump_height_from_standing_meters",
    "jump_height_from_takeoff_meters",
    "reactive_strength_index_modified",
    "system_mass_kilograms",
    "takeoff_velocity_meters_per_second",
];

/// The gravity one number ran under, for the quantities that move with it, read off the record
/// rather than off the request.
///
/// A rule whose registry entry publishes a gravity of its own records it on its own row and may
/// have run at a value the request never held. `jumpheight.takeoff.flight_time` is that rule: on
/// a request nobody stated a gravity for, it runs at the 9.81 its entry declares while the
/// request carries 9.80665, so its own row and the analysis value are two different numbers and
/// only one of them produced the height.
///
/// This lived in the Python package, which put the record on one surface. Every consumer reads
/// the tree from here, so here is where one account of what produced a number reaches all of
/// them.
fn analysis_gravity_behind(
    response: &AnalysisResponse,
    metric: &Metric,
) -> Option<ParameterRecord> {
    use crate::slots::jh_takeoff_frame::flight_time::GRAVITY_PARAMETER;

    if !QUANTITIES_RESTING_ON_THE_ANALYSIS_GRAVITY.contains(&metric.key.as_str()) {
        return None;
    }
    let published_by_the_rule = metric
        .computed_by
        .as_deref()
        .and_then(|id| {
            response
                .bound_methods
                .iter()
                .find(|bound| bound.method_id == id)
        })
        .and_then(|bound| {
            let value = *bound.numeric_values.get(GRAVITY_PARAMETER)?;
            let source = bound
                .parameter_sources
                .get(GRAVITY_PARAMETER)
                .copied()
                .unwrap_or(ParameterSource::Assumed);
            Some((value, source))
        });
    let bound_for_the_analysis = response
        .bound_globals
        .iter()
        .find(|bound| bound.name == crate::GRAVITY_GLOBAL)
        .map(|bound| (bound.value, bound.source));
    let (value, source) = published_by_the_rule.or(bound_for_the_analysis)?;
    Some(ParameterRecord {
        name: crate::GRAVITY_GLOBAL.to_string(),
        value,
        source,
    })
}

/// The chain behind one number: the rule that computed it, the rules its answer rests on, and
/// the operators those rules composed.
///
/// The root is the arithmetic the response names in `computed_by`, carrying its own bound
/// values. Where the response names none, the quantity is a landmark rule's own answer and the
/// root is the last rule to run among the ones that contributed, which is the one whose answer
/// this quantity is.
///
/// A contributing id with no bound record and no row of its own is not made a step. Every one
/// of them is a value of a choice the root already records, so a step for it would report one
/// decision twice, and `every_contributing_rule_is_somewhere_in_the_chain` holds that.
pub fn chain_of(
    response: &AnalysisResponse,
    metric: &Metric,
    registry: &RegistryStamp,
    acquisition_complete: bool,
) -> ProvenanceChain {
    let bound_for = |method_id: &str| {
        response
            .bound_methods
            .iter()
            .find(|bound| bound.method_id == method_id)
    };

    let contributing: Vec<&BoundMethod> = metric
        .contributing_method_ids
        .iter()
        .filter_map(|id| bound_for(id))
        .collect();

    let (operators, mut steps): (Vec<&BoundMethod>, Vec<&BoundMethod>) = contributing
        .into_iter()
        .partition(|bound| composes_onto(&bound.method_id).is_some());

    // The arithmetic roots the chain when the response names one. Otherwise the quantity is a
    // rule's own answer, and the rule that produced it is the last of the contributors to run.
    let arithmetic = metric.computed_by.as_deref();
    let root_of_the_landmarks = arithmetic.is_none().then(|| {
        steps
            .iter()
            .enumerate()
            .max_by_key(|(position, bound)| (rank(&bound.method_id), *position))
            .map(|(position, _)| position)
    });
    let landmark_root = match root_of_the_landmarks {
        Some(Some(position)) => Some(steps.remove(position)),
        _ => None,
    };

    // Each operator sits under the landmark rule it composes onto, and under the root where
    // that rule did not contribute, so it is never dropped and never floats to a depth that
    // claims it composed onto something else.
    let inputs_of = |bound: &BoundMethod, operators: &[&BoundMethod]| -> Vec<ProvenanceChain> {
        let construct = construct_of(&bound.method_id);
        operators
            .iter()
            .filter(|operator| composes_onto(&operator.method_id) == construct)
            .map(|operator| step(operator, registry, acquisition_complete, Vec::new()))
            .collect()
    };

    let built: Vec<ProvenanceChain> = steps
        .iter()
        .map(|bound| {
            step(
                bound,
                registry,
                acquisition_complete,
                inputs_of(bound, &operators),
            )
        })
        .collect();

    // Operators whose landmark rule is not among the contributors, which would otherwise be
    // left out of the record entirely.
    let claimed: Vec<Option<&'static str>> = steps
        .iter()
        .map(|bound| construct_of(&bound.method_id))
        .collect();
    let orphaned: Vec<ProvenanceChain> = operators
        .iter()
        .filter(|operator| !claimed.contains(&composes_onto(&operator.method_id)))
        .map(|operator| step(operator, registry, acquisition_complete, Vec::new()))
        .collect();

    let under_the_root: Vec<ProvenanceChain> = built.into_iter().chain(orphaned).collect();

    let mut rooted = match (arithmetic, landmark_root) {
        (Some(id), _) => match bound_for(id) {
            Some(bound) => step(bound, registry, acquisition_complete, under_the_root),
            None => unbound_step(id, registry, acquisition_complete, under_the_root),
        },
        (None, Some(bound)) => {
            let mut inputs = inputs_of(bound, &operators);
            inputs.extend(under_the_root);
            step(bound, registry, acquisition_complete, inputs)
        }
        // A quantity naming no arithmetic and no rule that ran. The record says so rather than
        // inventing a step, and `Provenance::of("")` is the shape the R boundary already wrote
        // for it.
        (None, None) => unbound_step("", registry, acquisition_complete, under_the_root),
    };

    // The value the analysis was bound to that produced this number, which belongs to the
    // analysis and to no rule's registry entry, so it reaches the root of the chain rather than
    // any row. On the root because that is the step a reader meets first and the one every
    // consumer publishes.
    rooted
        .provenance
        .parameters
        .extend(analysis_gravity_behind(response, metric));
    rooted
}

/// Whether one number's chain names a rule: the arithmetic that computed it, or one of the
/// rules its answer rests on.
///
/// The two lists this reads are the two `chain_of` builds its tree from, so a reader asking
/// which numbers a rule moved and a reader reading the tree under one number are answered from
/// one place.
///
/// A contributing id the response left no bound record for still counts. It is a value of a
/// choice the root records rather than a step of its own, which is a fact about how the tree is
/// shaped and not about whether the number rests on the rule: the four integration entries
/// behind takeoff velocity are named by no other route.
pub fn chain_names(metric: &Metric, method_id: &str) -> bool {
    metric.computed_by.as_deref() == Some(method_id)
        || metric
            .contributing_method_ids
            .iter()
            .any(|id| id == method_id)
}

/// Every quantity whose chain names this rule, in the response's own order.
///
/// What a signal about a rule is about. A list of keys written beside the rule instead is the
/// same fact spelled twice and the two are free to disagree: measured on subject 01's first
/// trial under `onset.threshold.noise_relative` and `takeoff.threshold.absolute_force`, the two
/// hand-written lists this replaced named 2 and 3 keys against the 6 and 8 of 11 metrics whose
/// chains name those rules.
pub fn metrics_resting_on(response: &AnalysisResponse, method_id: &str) -> Vec<String> {
    response
        .metrics
        .iter()
        .filter(|metric| chain_names(metric, method_id))
        .map(|metric| metric.key.clone())
        .collect()
}

/// The chain behind every number the analysis reported, in the response's own order.
///
/// Every metric, including the ones that carry no value: a surface that reports what a rule
/// would have produced needs the same record as one that reports what it did, and a caller
/// that wants only the numbers filters on `Metric::value` itself.
pub fn chains_of(
    response: &AnalysisResponse,
    registry: &RegistryStamp,
    acquisition_complete: bool,
) -> Vec<MetricChain> {
    response
        .metrics
        .iter()
        .map(|metric| MetricChain {
            quantity: metric.key.clone(),
            chain: chain_of(response, metric, registry, acquisition_complete),
        })
        .collect()
}

/// The account each quantity gives of itself, keyed by the quantity.
///
/// The one site that writes them. The sentence itself is
/// `plateforce_core::reporting::describe`, and this is where the arguments it takes are
/// assembled, so a terminal, a browser tab, a notebook and an R session cannot assemble them
/// differently. It lived in the R boundary and nowhere else, so R was the only surface whose
/// reader ever received one.
///
/// The chain each account is written around is `chains_of`'s, walked beside `metrics` because
/// that call returns one entry per metric in the response's own order. The R boundary used to
/// build a tree of its own, which put every contributing rule at one depth under a root
/// carrying none of the arithmetic's own values, so the gravity behind the flight-time height
/// was absent from the sentence a reader was shown.
///
/// A quantity with no value gets no account, because an account is written around a
/// `Measured` and there is nothing measured to write one about. This used to be the one place
/// in the whole product where a number that is not a number could be told from a number
/// nobody computed: a metric could hold a NaN, so it reached this loop, and the account read
/// "NaN newtons" with a full provenance chain behind it, asserting a measurement that was
/// never made. That distinction now lives on the metric itself, on every surface, as
/// `carried_no_number`, so this loop is free to say nothing about a quantity that has no
/// value without taking the only account of it away from a reader.
pub fn descriptions_of(
    response: &AnalysisResponse,
    chains: &[MetricChain],
) -> BTreeMap<String, String> {
    let mut accounts = BTreeMap::new();
    for (metric, derived) in response.metrics.iter().zip(chains) {
        let Some(value) = metric.value else { continue };
        let Some(unit) = declared_unit(metric) else {
            continue;
        };
        let measured = Measured {
            value,
            unit,
            provenance: derived.chain.provenance.clone(),
        };
        accounts.insert(metric.key.to_string(), describe(&measured, &derived.chain));
    }
    accounts
}

/// The account each quantity gives of itself, for a caller holding no chains of its own.
///
/// The chains and the accounts come from one derivation either way, so a surface that wants
/// only the accounts cannot reach them through a second one.
pub fn accounts_of(
    response: &AnalysisResponse,
    registry: &RegistryStamp,
    acquisition_complete: bool,
) -> BTreeMap<String, String> {
    descriptions_of(
        response,
        &chains_of(response, registry, acquisition_complete),
    )
}

/// The unit a metric reports, taken from the one declaration that spells it, and only when
/// the metric agrees with that declaration.
///
/// A metric owns its unit so a quantity can take one from the registry, and `Measured` holds
/// a static one. Reading the declaration instead of the metric would print a unit the number
/// does not carry the moment those two differ, which `unit_of_every_metric_is_the_declared_one`
/// is what stops.
fn declared_unit(metric: &Metric) -> Option<&'static str> {
    let declared = crate::response::quantity(&metric.key)?;
    (declared.unit == metric.unit).then_some(declared.unit)
}
