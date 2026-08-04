//! Whether a rule says the registry carries it follows the list the engine was handed.
//!
//! `registry_entry` is documented as false when no registry row carries an id, and the engine
//! does not read the registry: it is handed a list of ids and reports against that. So the
//! failure worth guarding is a surface or an engine that *asserts* this rather than *reads*
//! it, reporting one value whatever it is told. A surface building its list from the ids the
//! caller named reports false for every operator composed on top, while those operators sit in
//! the registry and another surface reports the truth for the same trial.
//!
//! Every id this build records resolves in the registry, which
//! `plateforce-wasm`'s `every_id_this_build_records_resolves_in_the_registry` holds, so the
//! false side is produced here on purpose. Producing it here rather than through a surface
//! reaches the case no surface-level test can: the engine ignoring the list rather than the
//! surface building it wrongly.
//!
//! The same id is run both ways and the answer has to change. A test that only withheld an id
//! would pass against an engine hardcoding false, and one that only supplied it would pass
//! against an engine hardcoding true.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::Registry;

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// An operator this build composes without being asked, so it is bound on the bare request
/// below and is not one of the three ids the caller names.
const WITHHELD: &str = "onset.op.persistence";

fn registry_ids() -> Vec<String> {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the shipped registry loads")
        .methods
        .keys()
        .cloned()
        .collect()
}

/// Quiet stance, an unweighting dip, a push, flight, then landing.
fn trial() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, value) in force.iter_mut().enumerate() {
        *value += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(1400.0, 240));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn request_backed_by(ids: Vec<String>) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
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
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: ids,
        ..Default::default()
    }
}

/// What the run said about one id, and `None` when the run never bound it. The difference
/// matters: a rule that did not run reports nothing, and reading that as false would let this
/// pass on a request that stopped composing the operator entirely.
fn backing_reported_for(response: &AnalysisResponse, id: &str) -> Option<bool> {
    response
        .bound_methods
        .iter()
        .find(|bound| bound.method_id == id)
        .map(|bound| bound.registry_backed)
}

#[test]
fn the_same_rule_reports_carried_or_not_according_to_the_list_alone() {
    let carried = registry_ids();
    assert!(
        carried.iter().any(|id| id == WITHHELD),
        "{WITHHELD} is not in the registry, so withholding it proves nothing about reading a list"
    );
    let withheld: Vec<String> = carried
        .iter()
        .filter(|id| *id != WITHHELD)
        .cloned()
        .collect();
    assert_eq!(
        withheld.len(),
        carried.len() - 1,
        "exactly one id is withheld between the two runs"
    );

    let told_everything = run(&trial(), &request_backed_by(carried)).expect("the request runs");
    let told_less = run(&trial(), &request_backed_by(withheld)).expect("the request runs");

    // The operator has to be bound in both runs, or the two answers are about a rule that ran
    // once and a rule that did not, which is not a comparison.
    let with = backing_reported_for(&told_everything, WITHHELD);
    let without = backing_reported_for(&told_less, WITHHELD);
    println!("{WITHHELD}: told everything {with:?}, told less {without:?}");
    assert_eq!(
        with,
        Some(true),
        "{WITHHELD} is bound and is in the list it was handed, so it is carried"
    );
    assert_eq!(
        without,
        Some(false),
        "{WITHHELD} is bound and was withheld from the list, so nothing entitles the engine to \
         call it carried"
    );
}

/// The counterpart, in the run that withholds one id: everything else still reports carried.
///
/// Without this the test above is satisfied by an engine that answers false for everything
/// once any id is missing, which reports the withheld rule correctly for the wrong reason.
#[test]
fn withholding_one_id_changes_the_answer_for_that_id_and_no_other() {
    let carried = registry_ids();
    let withheld: Vec<String> = carried
        .iter()
        .filter(|id| *id != WITHHELD)
        .cloned()
        .collect();
    let told_less = run(&trial(), &request_backed_by(withheld)).expect("the request runs");

    let mut said_not_carried = Vec::new();
    let mut said_carried = 0usize;
    for bound in &told_less.bound_methods {
        if bound.registry_backed {
            said_carried += 1;
        } else {
            said_not_carried.push(bound.method_id.clone());
        }
    }

    println!(
        "{} of {} bound rules report carried, the rest are {said_not_carried:?}",
        said_carried,
        told_less.bound_methods.len()
    );
    assert!(
        said_carried > 0,
        "every bound rule reports not carried, so the answer does not depend on the list"
    );
    assert_eq!(
        said_not_carried,
        vec![WITHHELD.to_string()],
        "only the withheld id loses its backing"
    );
}
