//! Rules computed from what the landmark rules resolved.
//!
//! The three landmark rules are an ordered chain with real inter-dependencies: the weighing
//! window settles first, the takeoff rule reads it, and one onset rule searches back from a
//! point only takeoff bounds. They stay a spine and are reached by name. Everything filed
//! under the registry's other constructs runs after them, over what they placed, and is
//! reached by construct id through a map on the request.
//!
//! WS-E1 classified the fifty-eight level-one entries mechanically and found they are not
//! one problem: four constructs and fifteen entries condition the signal before the spine,
//! one entry is a declaration on the spine itself, and twenty constructs and forty-two
//! entries are computed after it. Generalising the spine buys none of them anything.
//!
//! Adding a rule is a file here and a row in `BINDINGS`. Nothing in `pipeline.rs` changes.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::provenance::ParameterSource;
use plateforce_core::{Landmarks, Trial, WeighingEpoch};

use crate::request::MethodChoice;
use crate::resolution::{BoundValues, RuleRefusal};

/// A sample one rule placed, and the rule that placed it. The second half is what lets a
/// number computed from it name the rule it rests on rather than every rule that ran first.
#[derive(Debug, Clone, Copy)]
pub struct PlacedSample {
    pub index: usize,
    pub placed_by: &'static str,
}

/// What every rule computed from the landmarks is handed.
pub struct DerivedContext<'a> {
    pub trial: &'a Trial,
    pub epoch: &'a WeighingEpoch,
    /// What each landmark rule placed, each `None` when that rule produced nothing. Held
    /// one by one rather than as a bundle, because a rule that needs only takeoff can run on
    /// a recording whose onset rule declined, and a bundle would deny it the answer it has.
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub gravity_meters_per_second_squared: f64,
    /// What the request claims about the number above. Carried because a rule whose entry
    /// publishes its own gravity has to tell a value somebody chose for this analysis, which
    /// it must honour, from the constant the request type fills in for everybody, which no
    /// entry declares.
    pub gravity_source: ParameterSource,
    /// The athlete's mass, which is not the weighed system mass: system weight includes the
    /// bar and bodyweight does not. `None` when the caller stated none, and a rule that
    /// divides by it declines rather than dividing by the other one.
    pub body_mass_kilograms: Option<f64>,
    /// Samples placed by rules that ran before this one, under the name the placing rule
    /// published them by. `run` resolves in `BINDINGS` declaration order, so a rule reading
    /// one of these is declared after the rule that places it.
    ///
    /// Ordering alone would be decorative without this: a rule that cannot see what an
    /// earlier rule placed cannot consume it, and every alternative channel was a second
    /// place for one fact to live.
    pub placed: &'a BTreeMap<&'static str, PlacedSample>,
    /// The constructs this request chose a rule for. A rule reads it to tell a choice
    /// nobody made from a choice that was made and declined, which are different faults
    /// with different remedies.
    pub requested: &'a BTreeMap<String, MethodChoice>,
    /// The names this rule asked for, recorded as it asks. The chain behind its number is
    /// built from these, so a rule names the rules it rests on rather than every rule that
    /// happened to run before it.
    read: RefCell<BTreeSet<&'static str>>,
}

impl<'a> DerivedContext<'a> {
    /// Everything a rule is handed, with nothing read yet.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trial: &'a Trial,
        epoch: &'a WeighingEpoch,
        onset_index: Option<usize>,
        takeoff_index: Option<usize>,
        touchdown_index: Option<usize>,
        gravity_meters_per_second_squared: f64,
        gravity_source: ParameterSource,
        body_mass_kilograms: Option<f64>,
        placed: &'a BTreeMap<&'static str, PlacedSample>,
        requested: &'a BTreeMap<String, MethodChoice>,
    ) -> Self {
        Self {
            trial,
            epoch,
            onset_index,
            takeoff_index,
            touchdown_index,
            gravity_meters_per_second_squared,
            gravity_source,
            body_mass_kilograms,
            placed,
            requested,
            read: RefCell::new(BTreeSet::new()),
        }
    }

    /// The rules whose samples this one read, in the order the names sort, without repeats.
    pub fn rules_read(&self) -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = self
            .read
            .borrow()
            .iter()
            .filter_map(|name| self.placed.get(name).map(|sample| sample.placed_by))
            .collect();
        ids.dedup();
        ids
    }

    /// The three landmarks as one set, on the same condition the pipeline applies: both
    /// bounds placed and takeoff after onset. Derived here rather than passed in, so the
    /// condition has one home.
    pub fn landmarks(&self) -> Option<Landmarks> {
        match (self.onset_index, self.takeoff_index) {
            (Some(onset), Some(takeoff)) if takeoff > onset => Some(Landmarks {
                onset_index: onset,
                takeoff_index: takeoff,
                touchdown_index: self.touchdown_index.unwrap_or(self.trial.len() - 1),
            }),
            _ => None,
        }
    }

    /// The gravity somebody chose for this whole analysis, or nothing where the request holds
    /// the constant its own type fills in.
    ///
    /// Only a rule whose entry publishes a gravity of its own needs this, and it needs it to
    /// avoid two opposite faults: running on a number nobody chose, and discarding a number
    /// somebody did. Gravity varies by half a percent across the Earth's surface, fifteen times
    /// the gap between the two constants the tools argue over, so a plate's own value is better
    /// information than any published one and has to win when it is offered.
    ///
    /// `Assumed` is the only claim that means nobody acted. A provisional value is one somebody
    /// put there to look at, and `taints_the_record` already stops a result resting on one from
    /// leaving the building, so it runs and says what it is.
    pub fn chosen_gravity(&self) -> Option<(f64, ParameterSource)> {
        (self.gravity_source != ParameterSource::Assumed)
            .then_some((self.gravity_meters_per_second_squared, self.gravity_source))
    }

    /// A sample an earlier rule placed, or nothing when no rule placed one under that name.
    ///
    /// Asking is recorded whether or not there is an answer, which is the same discipline
    /// `Resolution` applies to a parameter: what a rule consulted is a fact about the rule,
    /// not about what it happened to find.
    pub fn sample(&self, name: &'static str) -> Option<usize> {
        self.read.borrow_mut().insert(name);
        self.placed.get(name).map(|sample| sample.index)
    }

    /// Whether the request chose a rule for this construct.
    ///
    /// The spine's three are named on every request by their own fields and are validated
    /// before anything runs, so they are always chosen. Reading only the map would report a
    /// takeoff rule that ran and found nothing as a takeoff choice nobody made.
    pub fn was_chosen(&self, construct: &str) -> bool {
        crate::binding::SPINE_CONSTRUCTS.contains(&construct)
            || self.requested.contains_key(construct)
    }

    /// A refusal naming the constructs this rule needed and did not get.
    ///
    /// Two situations and two codes, told apart by whether the caller chose a rule at all.
    /// Chosen means it was chosen and declined, so the remedy is upstream. Unchosen means
    /// the caller left open a choice this rule forces, and the remedy is to make it. A
    /// single code for both would send half the readers to the wrong repair.
    pub fn unavailable(&self, method_id: &str, needs: &[&str]) -> RuleRefusal {
        let named: Vec<String> = needs.iter().map(|name| (*name).to_string()).collect();
        let every_one_was_chosen = needs.iter().all(|name| self.was_chosen(name));
        RuleRefusal::Refused(Box::new(if every_one_was_chosen {
            plateforce_core::Refusal::dependency_unresolved(method_id, named)
        } else {
            plateforce_core::Refusal::decision_not_made(method_id, named)
        }))
    }
}

/// What one rule computed from the landmarks, and the record of what it read.
pub struct DerivedOutcome {
    /// Quantities produced, by declared key. `None` against a key means the rule ran and
    /// this quantity has no value on this recording, which is a different report from the
    /// rule declining. Empty for a declaration, which contributes provenance and no number.
    pub values: Vec<(&'static str, Option<f64>)>,
    /// Samples placed, under names later rules read them by. A phase boundary is an index
    /// rather than a number, and both are answers a rule can produce.
    pub placed: Vec<(&'static str, usize)>,
    pub bound: BoundValues,
    pub refusal: Option<RuleRefusal>,
}

impl DerivedOutcome {
    /// A rule that ran and produced nothing, carrying what it read while declining.
    pub fn declined(bound: BoundValues, refusal: RuleRefusal) -> Self {
        Self {
            values: Vec::new(),
            placed: Vec::new(),
            bound,
            refusal: Some(refusal),
        }
    }
}

/// The signature every rule computed from the landmarks has.
///
/// `&mut Vec<String>` for warnings is what all three landmark rules already take, so a rule
/// that has something to tell a reader tells it the same way wherever it sits.
pub type DerivedRule = fn(&DerivedContext, &MethodChoice, &mut Vec<String>) -> DerivedOutcome;

#[cfg(test)]
mod tests {
    use super::*;
    use plateforce_core::WeighingEpoch;

    fn a_trial() -> Trial {
        Trial::new(vec![600.0; 1200], 1200.0).unwrap()
    }

    fn an_epoch() -> WeighingEpoch {
        WeighingEpoch {
            start_index: 0,
            end_index: 600,
            system_weight_newtons: 600.0,
            standard_deviation_newtons: 1.0,
            tied_window_count: 1,
            tied_weight_low_newtons: 600.0,
            tied_weight_high_newtons: 600.0,
        }
    }

    /// The chain behind a number is the rules it read, not the rules that ran before it.
    ///
    /// Guarded here rather than through the pipeline, because through the pipeline the two
    /// are the same set: one construct places samples, so everything placed is everything
    /// read and an assertion about the difference could not fail. It becomes observable the
    /// moment a second construct places anything, which is what the fifty-eight entries this
    /// dispatch unblocks will do.
    #[test]
    fn the_chain_names_the_rules_a_rule_read_and_not_the_rules_that_merely_ran() {
        let trial = a_trial();
        let epoch = an_epoch();
        let placed = BTreeMap::from([
            (
                "analysis_window.start",
                PlacedSample {
                    index: 0,
                    placed_by: "window_end.takeoff.detected",
                },
            ),
            (
                "analysis_window.end",
                PlacedSample {
                    index: 900,
                    placed_by: "window_end.takeoff.detected",
                },
            ),
            (
                "braking_phase_start",
                PlacedSample {
                    index: 400,
                    placed_by: "phase.braking_start.zero_net_force",
                },
            ),
        ]);
        let requested = BTreeMap::new();
        let context = DerivedContext::new(
            &trial,
            &epoch,
            Some(100),
            Some(900),
            None,
            9.80665,
            ParameterSource::Assumed,
            None,
            &placed,
            &requested,
        );

        assert!(context.rules_read().is_empty(), "nothing has been read yet");

        assert_eq!(context.sample("analysis_window.start"), Some(0));
        assert_eq!(context.sample("analysis_window.end"), Some(900));
        assert_eq!(
            context.rules_read(),
            vec!["window_end.takeoff.detected"],
            "a rule that read one construct's samples named another's"
        );

        // Asking for a name nothing placed is still an ask, and it adds no rule to the
        // chain, because there is no rule behind it to name.
        assert_eq!(context.sample("propulsion_phase_end"), None);
        assert_eq!(context.rules_read(), vec!["window_end.takeoff.detected"]);
    }
}
