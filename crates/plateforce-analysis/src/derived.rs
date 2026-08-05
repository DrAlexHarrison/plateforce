//! Rules computed from what the landmark rules resolved.
//!
//! The three landmark rules are an ordered chain with real inter-dependencies: the weighing
//! window settles first, the takeoff rule reads it, and one onset rule searches back from a
//! point only takeoff bounds. They stay a spine and are reached by name. Everything filed
//! under the registry's other constructs runs after them, over what they placed, and is
//! reached by construct id through a map on the request.
//!
//! The level-one entries are not one problem: some condition the signal before the spine
//! runs, one is a declaration on the spine itself, and the rest are computed after it over
//! what it placed. `registry census` counts each population.
//!
//! Adding a rule is a file here and a row in `BINDINGS`. Nothing in `pipeline.rs` changes.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::provenance::ParameterSource;
use plateforce_core::{Landmarks, Trial, WeighingEpoch};

use crate::request::MethodChoice;
use crate::resolution::{BoundValues, RuleRefusal};

/// The names the spine's own landmarks are read by.
///
/// A rule reaches one of these exactly as it reaches a sample another rule placed, so asking
/// is recorded either way and the chain behind a number is built from what its rule asked
/// for. Before this the spine's landmarks were fields a rule read in silence, which is why
/// every chain had to open with the same prefix of conditioning, weighing, onset and takeoff:
/// with no record of what was read, naming everything was the only shape that could not omit
/// a rule that contributed. It named several that had not.
pub const WEIGHING_EPOCH: &str = "weighing_epoch";
pub const MOVEMENT_ONSET: &str = "movement_onset";
pub const TAKEOFF: &str = "takeoff";
pub const TOUCHDOWN: &str = "touchdown";

/// A sample the analysis placed, the entries that placed it, and what those entries read.
///
/// `placed_by` is a list because a landmark rule hands back a threshold entry followed by
/// every operator entry it bound, and a node naming the threshold alone hides which crossing
/// each operator selected. `rests_on` is what makes the chain a graph rather than a prefix: a
/// number that read this sample rests on everything under it and on nothing beside it.
#[derive(Debug, Clone)]
pub struct PlacedSample {
    /// `None` where the rules for this sample ran and placed nothing. The node stays, because
    /// a number that asked for it still rests on the rules that tried.
    pub index: Option<usize>,
    pub placed_by: Vec<String>,
    pub rests_on: Vec<&'static str>,
    /// Values belonging to the analysis rather than to any registry entry that moved this
    /// sample. A rule that places a boundary off a velocity series read the gravity that
    /// scaled it, and a number reading the boundary rests on that gravity without naming it
    /// itself, exactly as it rests on the rules in `rests_on` without naming those.
    pub globals: Vec<&'static str>,
    /// Where this node's entries sit in the order the result records its rules, which is the
    /// order `bound_methods` lists them in. Held on the node because a closure returns a set
    /// and a reader needs one order, and reusing the record's own order means a chain and the
    /// record it came from read alike.
    pub order: usize,
}

/// The entries behind every named sample, and behind the samples those rest on.
///
/// One home for the walk, because `DerivedContext` closes over what a rule asked for and the
/// spine closes over what it reports directly, and two copies of a transitive closure are
/// free to answer the same question differently.
///
/// Presented in node order rather than in the order the walk reached them, so the chain
/// behind a number lists its entries in the order the result records them.
pub fn rules_behind(
    placed: &BTreeMap<&'static str, PlacedSample>,
    names: &[&'static str],
) -> Vec<String> {
    let mut nodes: Vec<&PlacedSample> = nodes_behind(placed, names);
    nodes.sort_by_key(|node| node.order);

    let mut ids: Vec<String> = Vec::new();
    for node in nodes {
        for id in &node.placed_by {
            if !ids.iter().any(|held| held == id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

/// The analysis-level values behind every named sample, and behind the samples those rest on.
///
/// The same closure `rules_behind` walks, over the other thing a node carries. A rule that
/// placed a boundary off a velocity series read a gravity, and a number reading that boundary
/// rests on the gravity through the sample rather than by reading one itself, so the two facts
/// travel together or the record names one and not the other.
pub fn globals_behind(
    placed: &BTreeMap<&'static str, PlacedSample>,
    names: &[&'static str],
) -> BTreeSet<&'static str> {
    nodes_behind(placed, names)
        .into_iter()
        .flat_map(|node| node.globals.iter().copied())
        .collect()
}

/// Every node behind the named samples, closed transitively and reached once each.
///
/// Written once because the two walks above ask the same question of the graph and differ
/// only in what they read off the nodes they reach.
fn nodes_behind<'a>(
    placed: &'a BTreeMap<&'static str, PlacedSample>,
    names: &[&'static str],
) -> Vec<&'a PlacedSample> {
    let mut reached: BTreeSet<&'static str> = BTreeSet::new();
    let mut pending: Vec<&'static str> = names.to_vec();
    while let Some(name) = pending.pop() {
        if !reached.insert(name) {
            continue;
        }
        if let Some(node) = placed.get(name) {
            pending.extend(node.rests_on.iter().copied());
        }
    }
    reached.iter().filter_map(|name| placed.get(name)).collect()
}

/// The landing the caller placed, written onto the row of every rule that read it.
///
/// A landing the software found is the return above the threshold the takeoff rule resolved,
/// and the chain already names that rule. A landing the caller stated came from no rule at
/// all, and without this the two reach identical records: on a jump that lands, moving the
/// stated landing 300 samples took flight time from 0.676 s to 0.926 s and the flight-time
/// height from 0.560 m to 1.051 m, and both pairs fingerprinted the same. The entry the record
/// names says the landing is "the first sample at which force returned above the threshold that
/// placed takeoff", so the unrecorded case was the record asserting a rule that had not run.
///
/// Written as a value rather than as a step, because no entry places it: the sample is the
/// caller's, and `Stated` is what the record already says about a value a caller supplied. The
/// index itself and not a flag, because two different hand-placed landings give two different
/// flight times and a flag would report them as one.
///
/// Written per rule that read the landing, so a number that never asked for it does not carry
/// it. One home called from both phases, on the model of `bound_with_operators`: a rule the
/// spine runs for itself and a rule a caller named record the same fact the same way.
pub fn record_stated_touchdown(
    context: &DerivedContext,
    bound: &mut BoundValues,
    stated_index: Option<usize>,
) {
    let Some(index) = stated_index else { return };
    if !context.names_read().contains(&TOUCHDOWN) {
        return;
    }
    let name = crate::request::TOUCHDOWN_GLOBAL.to_string();
    bound.parameters.push((name.clone(), index.to_string()));
    bound.numbers.insert(name.clone(), index as f64);
    bound.sources.insert(name, ParameterSource::Stated);
}

/// What every rule computed from the landmarks is handed.
pub struct DerivedContext<'a> {
    /// The signal every rule reads, which is what the conditioning phase produced. Public and
    /// unrecorded because every rule reads it, so naming the conditioning entries at the head
    /// of every chain states a fact rather than assuming one.
    pub trial: &'a Trial,
    epoch: &'a WeighingEpoch,
    /// What each landmark rule placed, each `None` when that rule produced nothing. Held
    /// one by one rather than as a bundle, because a rule that needs only takeoff can run on
    /// a recording whose onset rule declined, and a bundle would deny it the answer it has.
    ///
    /// Private, and reached through the accessors below, because reading one is what puts the
    /// rules behind it into the chain. A field a rule could read in silence is a rule whose
    /// number cannot say what produced it.
    onset_index: Option<usize>,
    takeoff_index: Option<usize>,
    touchdown_index: Option<usize>,
    /// The gravity this analysis is bound to.
    ///
    /// Private, and reached through the accessors below, for the reason the landmark indices
    /// above are: reading it is what puts it into the chain behind the number that read it.
    /// A field a rule could read in silence is a number that cannot say what moved it, and
    /// the whole of what the record used to say about gravity was a hand-written list of the
    /// keys somebody believed were affected.
    gravity_meters_per_second_squared: f64,
    /// What the request claims about the number above. Carried because a rule whose entry
    /// publishes its own gravity has to tell a value somebody chose for this analysis, which
    /// it must honour, from the constant the request type fills in for everybody, which no
    /// entry declares.
    gravity_source: ParameterSource,
    /// The athlete's mass, which is not the weighed system mass: system weight includes the
    /// bar and bodyweight does not. `None` when the caller stated none, and a rule that
    /// divides by it declines rather than dividing by the other one.
    pub body_mass_kilograms: Option<f64>,
    /// Samples placed by rules that ran before this one, under the name the placing rule
    /// published them by. `run` resolves in `BINDINGS` declaration order, so a rule reading
    /// one of these is declared after the rule that places it.
    pub placed: &'a BTreeMap<&'static str, PlacedSample>,
    /// The constructs this request chose a rule for. A rule reads it to tell a choice
    /// nobody made from a choice that was made and declined, which are different faults
    /// with different remedies.
    pub requested: &'a BTreeMap<String, MethodChoice>,
    /// The names this rule asked for, recorded as it asks. The chain behind its number is
    /// built from these, so a rule names the rules it rests on rather than every rule that
    /// happened to run before it.
    read: RefCell<BTreeSet<&'static str>>,
    /// Registry entries one number rests on that placed no sample, against the key of the
    /// number that rests on them.
    ///
    /// The four integration entries are what this exists for. They are choices made inside
    /// the arithmetic rather than rules that place a landmark, so `read` cannot reach them,
    /// and two of them give different velocities from one recording.
    rested_on: RefCell<BTreeMap<&'static str, Vec<String>>>,
    /// Values belonging to the analysis rather than to any registry entry, against the key of
    /// the number that rests on each.
    ///
    /// Per quantity rather than per rule, because one rule can report a number that moves with
    /// a global beside one that does not.
    /// `impulse.net_vertical.as_performance_determinant` is the case: the net impulse is
    /// integrated over the interval and does not move with gravity, and the takeoff velocity
    /// is read off the integrated series and does. A record on the rule's row would give the
    /// net impulse a dependence it has not got.
    globals_rested_on: RefCell<BTreeMap<&'static str, BTreeSet<&'static str>>>,
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
            rested_on: RefCell::new(BTreeMap::new()),
            globals_rested_on: RefCell::new(BTreeMap::new()),
        }
    }

    /// Entries one number this rule produces rests on, beyond the rules that placed samples
    /// for it.
    ///
    /// Declared per quantity rather than per rule, because one entry can describe two numbers
    /// that rest on different things. `impulse.net_vertical.as_performance_determinant` covers
    /// both the net impulse and the takeoff velocity, and the velocity is read off an
    /// integrated series while the impulse is integrated directly, so only one of them rests
    /// on the integration entries.
    pub fn rests_on(&self, quantity_key: &'static str, entry_ids: &[&str]) {
        let mut recorded = self.rested_on.borrow_mut();
        let behind = recorded.entry(quantity_key).or_default();
        for id in entry_ids {
            if !behind.iter().any(|held| held == id) {
                behind.push((*id).to_string());
            }
        }
    }

    /// The entries this rule declared one of its numbers rests on, in declaration order.
    pub fn entries_behind(&self, quantity_key: &str) -> Vec<String> {
        self.rested_on
            .borrow()
            .get(quantity_key)
            .cloned()
            .unwrap_or_default()
    }

    /// The entries behind every sample this rule read, and behind the samples those rest on.
    ///
    /// Closed transitively, because a rule that read a sample rests on whatever placed it and
    /// on whatever that rule read in turn. The onset rule that searches back from the
    /// countermovement dip is why: a number resting on its onset rests on the takeoff rule
    /// that bounded the search, and a chain naming the onset rule alone would stop one step
    /// short of the choice that moved the sample.
    pub fn rules_read(&self) -> Vec<String> {
        rules_behind(self.placed, &self.names_read())
    }

    /// The names this rule asked for, whether or not anything was placed under them.
    ///
    /// The pipeline records these against a sample this rule places, so a later rule reading
    /// that sample reaches what this one read without either of them naming it twice.
    pub fn names_read(&self) -> Vec<&'static str> {
        self.read.borrow().iter().copied().collect()
    }

    /// The weighing epoch, and the ask that puts the weighing rule into this number's chain.
    pub fn epoch(&self) -> &'a WeighingEpoch {
        self.read.borrow_mut().insert(WEIGHING_EPOCH);
        self.epoch
    }

    /// Where the jump started, and the ask that puts the onset rule into this number's chain.
    pub fn onset_index(&self) -> Option<usize> {
        self.read.borrow_mut().insert(MOVEMENT_ONSET);
        self.onset_index
    }

    pub fn takeoff_index(&self) -> Option<usize> {
        self.read.borrow_mut().insert(TAKEOFF);
        self.takeoff_index
    }

    pub fn touchdown_index(&self) -> Option<usize> {
        self.read.borrow_mut().insert(TOUCHDOWN);
        self.touchdown_index
    }

    /// The three landmarks as one set, on the same condition the pipeline applies: both
    /// bounds placed and takeoff after onset. Derived here rather than passed in, so the
    /// condition has one home.
    ///
    /// Asks for onset and takeoff, which are the two this reads. It does not ask for
    /// touchdown: the field below fills an unstated one with the last sample of the
    /// recording, which no rule placed, so a caller of this has not read a touchdown. A rule
    /// that wants the placed one asks for it, and `flight_time` does.
    pub fn landmarks(&self) -> Option<Landmarks> {
        match (self.onset_index(), self.takeoff_index()) {
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
    ///
    /// Recorded against the quantity whether or not the value survives: a rule that consulted
    /// the analysis gravity and then ran its entry's own published one still resolved which of
    /// the two to use by looking, and `jumpheight.takeoff.flight_time` is the rule where the
    /// answer decides the number.
    pub fn chosen_gravity_behind(
        &self,
        quantity_key: &'static str,
    ) -> Option<(f64, ParameterSource)> {
        self.record_global(crate::request::GRAVITY_GLOBAL, Some(quantity_key));
        (self.gravity_source != ParameterSource::Assumed)
            .then_some((self.gravity_meters_per_second_squared, self.gravity_source))
    }

    /// The gravity this analysis is bound to, recorded as a value the named number rests on.
    ///
    /// The ask is the record. A rule reaches this exactly as it reaches a sample another rule
    /// placed, so what a number rests on is derived from what its rule asked for rather than
    /// from a list somebody kept beside the rules. The list this replaced was measured against
    /// the eleven quantities one request reported and was a key short of the population this
    /// build computes when it was last widened.
    ///
    /// `None` is a rule that reports no number of its own: a phase boundary read off a
    /// velocity series moves with the gravity that scaled it, and the numbers that move are
    /// other rules'. The record then travels with the samples this rule placed, so the
    /// dependence closes over the graph the way the rules behind a sample already do. A rule
    /// that reports nothing and places nothing moves nothing, and records nothing anywhere.
    pub fn gravity_behind(&self, quantity_key: Option<&'static str>) -> f64 {
        self.record_global(crate::request::GRAVITY_GLOBAL, quantity_key);
        self.gravity_meters_per_second_squared
    }

    /// One home for both spellings of the ask, so a rule that places a sample and a rule that
    /// reports a number record the same fact the same way.
    fn record_global(&self, name: &'static str, quantity_key: Option<&'static str>) {
        let Some(key) = quantity_key else { return };
        self.globals_rested_on
            .borrow_mut()
            .entry(key)
            .or_default()
            .insert(name);
    }

    /// The analysis-level values this rule declared one of its numbers rests on.
    pub fn globals_behind(&self, quantity_key: &str) -> BTreeSet<&'static str> {
        self.globals_rested_on
            .borrow()
            .get(quantity_key)
            .cloned()
            .unwrap_or_default()
    }

    /// The analysis-level values this rule recorded against any number it reports, which the
    /// pipeline writes onto every sample it placed.
    ///
    /// What it recorded rather than what it read. A rule can read the gravity and place a
    /// boundary that does not rest on it: `phase.propulsion_start.zero_velocity` takes the
    /// zero crossing of a velocity series scaled by `1/g`, and scaling a series moves neither
    /// its zeros nor its extrema, so the sample is the same at any gravity. Writing what it
    /// read would put the gravity on that sample and on every number measured across it, and
    /// `propulsion_subdivision_seconds` reddened for exactly that: it named a gravity that
    /// moved it by nothing.
    pub fn globals_recorded(&self) -> Vec<&'static str> {
        self.globals_rested_on
            .borrow()
            .values()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// The analysis-level values behind every sample this rule read, and behind the samples
    /// those rest on.
    ///
    /// The counterpart of `rules_read`, over the same closure. A rule that reads a phase
    /// boundary another rule placed off a velocity series rests on the gravity that scaled
    /// that series, and it never read a gravity itself.
    pub fn globals_behind_the_samples_read(&self) -> BTreeSet<&'static str> {
        globals_behind(self.placed, &self.names_read())
    }

    /// A sample an earlier rule placed, or nothing when no rule placed one under that name.
    ///
    /// Asking is recorded whether or not there is an answer, which is the same discipline
    /// `Resolution` applies to a parameter: what a rule consulted is a fact about the rule,
    /// not about what it happened to find.
    pub fn sample(&self, name: &'static str) -> Option<usize> {
        self.read.borrow_mut().insert(name);
        self.placed.get(name).and_then(|sample| sample.index)
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
    /// moment a second construct places anything, which is what the entries this dispatch
    /// unblocks will do.
    #[test]
    fn the_chain_names_the_rules_a_rule_read_and_not_the_rules_that_merely_ran() {
        let trial = a_trial();
        let epoch = an_epoch();
        let placed = BTreeMap::from([
            (
                "analysis_window.start",
                PlacedSample {
                    index: Some(0),
                    placed_by: vec!["window_end.takeoff.detected".to_string()],
                    rests_on: Vec::new(),
                    globals: Vec::new(),
                    order: 4,
                },
            ),
            (
                "analysis_window.end",
                PlacedSample {
                    index: Some(900),
                    placed_by: vec!["window_end.takeoff.detected".to_string()],
                    rests_on: Vec::new(),
                    globals: Vec::new(),
                    order: 4,
                },
            ),
            (
                "braking_phase_start",
                PlacedSample {
                    index: Some(400),
                    placed_by: vec!["phase.braking_start.zero_net_force".to_string()],
                    rests_on: Vec::new(),
                    globals: Vec::new(),
                    order: 5,
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
            vec!["window_end.takeoff.detected".to_string()],
            "a rule that read one construct's samples named another's"
        );

        // Asking for a name nothing placed is still an ask, and it adds no rule to the
        // chain, because there is no rule behind it to name.
        assert_eq!(context.sample("propulsion_phase_end"), None);
        assert_eq!(
            context.rules_read(),
            vec!["window_end.takeoff.detected".to_string()]
        );
    }

    /// A number resting on one sample rests on everything under that sample.
    ///
    /// The onset rule that searches back from the countermovement dip is the case: a rule
    /// that read its onset rests on the takeoff rule that bounded the search, and a walk one
    /// step deep would name the onset rule and stop, one choice short of the one that moved
    /// the sample. Asserted against a graph three deep, because a two-deep graph is answered
    /// correctly by a walk that does not recurse at all.
    #[test]
    fn a_number_resting_on_a_sample_rests_on_everything_under_it() {
        let placed = BTreeMap::from([
            (
                WEIGHING_EPOCH,
                PlacedSample {
                    index: Some(600),
                    placed_by: vec!["bwepoch.fixed_window".to_string()],
                    rests_on: Vec::new(),
                    globals: Vec::new(),
                    order: 0,
                },
            ),
            (
                MOVEMENT_ONSET,
                PlacedSample {
                    index: Some(100),
                    placed_by: vec!["onset.threshold.last_within_band".to_string()],
                    rests_on: vec![WEIGHING_EPOCH, TAKEOFF],
                    globals: Vec::new(),
                    order: 1,
                },
            ),
            (
                TAKEOFF,
                PlacedSample {
                    index: Some(900),
                    placed_by: vec!["takeoff.threshold.absolute_force".to_string()],
                    rests_on: vec![WEIGHING_EPOCH],
                    globals: Vec::new(),
                    order: 2,
                },
            ),
            (
                TOUCHDOWN,
                PlacedSample {
                    index: Some(1100),
                    placed_by: Vec::new(),
                    rests_on: vec![TAKEOFF],
                    globals: Vec::new(),
                    order: 3,
                },
            ),
        ]);

        // Three deep from one ask, and in the order the result records the rules rather than
        // the order the walk reached them.
        assert_eq!(
            rules_behind(&placed, &[MOVEMENT_ONSET]),
            vec![
                "bwepoch.fixed_window".to_string(),
                "onset.threshold.last_within_band".to_string(),
                "takeoff.threshold.absolute_force".to_string(),
            ]
        );

        // And a number that read only the takeoff and the touchdown names neither the onset
        // rule nor anything reached only through it. This is the assertion the fixed prefix
        // could not satisfy: flight time is measured from takeoff to touchdown and rests on
        // no onset rule at all.
        assert_eq!(
            rules_behind(&placed, &[TAKEOFF, TOUCHDOWN]),
            vec![
                "bwepoch.fixed_window".to_string(),
                "takeoff.threshold.absolute_force".to_string(),
            ]
        );

        // A node that placed a sample and no entry of its own contributes nothing to name.
        // Touchdown is that node: it is the return above the threshold the takeoff rule
        // resolved, so it is not a choice and it is not offered as one.
        assert_eq!(
            rules_behind(&placed, &[TOUCHDOWN]),
            vec![
                "bwepoch.fixed_window".to_string(),
                "takeoff.threshold.absolute_force".to_string(),
            ]
        );
    }
}
