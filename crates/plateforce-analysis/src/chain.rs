//! The chain of rules behind each number an analysis produced.
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

use plateforce_core::provenance::RegistryStamp;
use plateforce_core::{Provenance, ProvenanceChain};

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
        inputs.iter().map(|input| input.provenance.clone()).collect(),
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
        depends_on: inputs.iter().map(|input| input.provenance.clone()).collect(),
        ..Provenance::of(method_id)
    };
    ProvenanceChain {
        provenance,
        enumerated_choices: Vec::new(),
        depends_on: inputs,
    }
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
        .map(|bound| step(bound, registry, acquisition_complete, inputs_of(bound, &operators)))
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

    match (arithmetic, landmark_root) {
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
    }
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
