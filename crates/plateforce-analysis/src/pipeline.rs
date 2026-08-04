//! One analysis, from a validated request to the numbers and the record of what produced
//! them.

use plateforce_core::{Landmarks, Trial};

use std::collections::BTreeMap;

use crate::binding::{expect_bound, Dispatch, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::derived::{DerivedContext, PlacedSample};
use crate::request::{AnalysisRequest, MethodChoice};
use crate::resolution::{bound_method, DeclinedRule};
use crate::response::{AnalysisResponse, Levels, Metric};
use crate::slots::flight_time::takeoff_to_touchdown as flight_time_seconds_rule;
use crate::slots::jh_takeoff_frame::{
    flight_time as flight_time_rule, impulse_momentum as impulse_momentum_rule, FLIGHT_TIME_KEY,
};
use crate::slots::net_impulse::as_performance_determinant as net_impulse_rule;
use crate::slots::reactive_strength_index::jh_tov_over_ttt as rsimod_rule;
use crate::slots::time_to_takeoff::onset_to_takeoff as time_to_takeoff_rule;
use crate::slots::{
    jh_takeoff_frame, movement_onset, net_impulse, reactive_strength_index, system_weight,
    takeoff as takeoff_slot,
};

/// Boxed on the error side because a `Refusal` carries every field a caller branches on,
/// which is wider than the error-size lint's threshold and rides on every call that
/// succeeds. `a_refusal_reports_what_it_costs_to_carry` prints the figure.
pub fn run(
    trial: &Trial,
    request: &AnalysisRequest,
) -> Result<AnalysisResponse, Box<plateforce_core::Refusal>> {
    expect_bound(&request.weighing.method_id, "weighing")?;
    expect_bound(&request.onset.method_id, "onset")?;
    expect_bound(&request.takeoff.method_id, "takeoff")?;

    let mut warnings = Vec::new();
    let gravity = request.gravity_meters_per_second_squared;

    // Counted over the recording as it arrived, before any conditioning rule has had a look
    // at it. A filter run over a trace with a gap in it spreads that gap across its window,
    // so counting afterwards would report the filter rather than the recording.
    let samples_carrying_no_number =
        plateforce_core::signal::reported_samples(trial.force(), None).carried_no_number;

    // Everything below reads the signal this phase produced, so it runs first and its rules
    // are named ahead of every other rule in each metric's chain.
    let conditioned = run_conditioning_phase(trial, request, &mut warnings)?;
    let trial = conditioned.trial.as_ref().unwrap_or(trial);

    let weighing = system_weight::resolve(trial, &request.weighing, &mut warnings)?;
    let epoch = weighing.epoch;
    let inherited_spread = (
        weighing.standard_deviation_convention,
        weighing.standard_deviation_convention_stated,
    );

    // A caller who states index zero placed the window, at the start. Reading zero as
    // nobody having placed it made one record answer the question twice: the rule records
    // `window_anchor` as `stated_index` from a stated source for any stated index, so a
    // stated zero said the caller placed it and said nobody did, in the same result. This
    // field is the one that reaches the fingerprint, so the disagreement travelled.
    let mut bound_methods = conditioned.bound_methods;
    bound_methods.push(bound_method(
        &request.weighing.method_id,
        weighing.bound,
        request.is_backed(&request.weighing.method_id),
        request.weighing.start_index.is_some(),
    ));

    let mut refusals: Vec<DeclinedRule> = Vec::new();

    // Takeoff settles first because one onset rule searches back from the countermovement
    // dip, which is the force minimum before the propulsive peak and so needs the jump's
    // end. Its warnings and refusal are held back and reported in slot order below, so the
    // record reads the same whichever rule needed the other.
    let mut takeoff_warnings = Vec::new();
    let mut takeoff = takeoff_slot::resolve(trial, &epoch, &request.takeoff, &mut takeoff_warnings);
    let takeoff_index = match request.takeoff.manual_index {
        Some(index) => Some(index.min(trial.len() - 1)),
        None => takeoff.index,
    };

    let (onset_index, onset_bound) = match request.onset.manual_index {
        // A dragged marker stands in for the rule, so no value the rule reads produced it.
        Some(index) => (
            Some(index.min(trial.len() - 1)),
            crate::resolution::BoundValues::default(),
        ),
        None => {
            let outcome = movement_onset::resolve(
                trial,
                &epoch,
                takeoff_index,
                &request.onset,
                inherited_spread,
                &mut warnings,
            );
            if let Some(rejected) = outcome.refusal {
                refusals.push(DeclinedRule {
                    construct: ONSET_CONSTRUCT,
                    method_id: crate::binding::records_under(&request.onset.method_id).to_string(),
                    refusal: rejected,
                });
            }
            (outcome.index, outcome.bound)
        }
    };
    let onset_methods = movement_onset::bound_methods(
        &request.onset.method_id,
        onset_bound,
        request,
        request.onset.manual_index.is_some(),
    );
    let onset_ids: Vec<String> = onset_methods
        .iter()
        .map(|bound| bound.method_id.clone())
        .collect();
    bound_methods.extend(onset_methods);

    // The takeoff rule runs even under a dragged marker, because the threshold it resolves
    // is what touchdown is found against.
    warnings.extend(takeoff_warnings);
    if let Some(rejected) = takeoff.refusal.take() {
        refusals.push(DeclinedRule {
            construct: TAKEOFF_CONSTRUCT,
            method_id: crate::binding::records_under(&request.takeoff.method_id).to_string(),
            refusal: rejected,
        });
    }
    let takeoff_methods = takeoff_slot::bound_methods(
        &request.takeoff.method_id,
        takeoff.bound,
        request,
        request.takeoff.manual_index.is_some(),
    );
    let takeoff_ids_bound: Vec<String> = takeoff_methods
        .iter()
        .map(|bound| bound.method_id.clone())
        .collect();
    bound_methods.extend(takeoff_methods);

    // Touchdown is the return above the threshold that defined takeoff, so it is not an
    // independent choice and it is not offered as one.
    let touchdown_was_stated = request.touchdown_index.is_some();
    let touchdown_index = request.touchdown_index.or_else(|| {
        takeoff_index.and_then(|from| {
            trial.force()[from..]
                .iter()
                .position(|&force| force > takeoff.threshold_newtons)
                .map(|offset| offset + from)
        })
    });

    if let (Some(onset), Some(takeoff_at)) = (onset_index, takeoff_index) {
        if onset >= takeoff_at {
            // Named to the numbers it reaches. Flight time and the height taken from it are
            // measured from takeoff to the return to the plate and are unaffected by where
            // onset landed, so a warning covering them would send a reader to discard a
            // number that is sound.
            warnings.push(
                "onset is at or after takeoff, so every number bounded by onset is meaningless"
                    .into(),
            );
        }
    }

    // The band drawn on the trace is k SD wide whichever onset rule ran, so it is not part
    // of any one rule's binding.
    let onset_band = request.onset.parameters.get("k").copied().unwrap_or(5.0)
        * epoch.standard_deviation_newtons;
    let landmarks = match (onset_index, takeoff_index) {
        (Some(onset), Some(takeoff_at)) if takeoff_at > onset => Some(Landmarks {
            onset_index: onset,
            takeoff_index: takeoff_at,
            touchdown_index: touchdown_index.unwrap_or(trial.len() - 1),
        }),
        _ => None,
    };

    // The spine's landmarks join the map derived rules already reach samples through, so a
    // rule reading one is recorded reading it and the chain behind its number is the closure
    // of what it asked for.
    //
    // Each node carries the entries that placed it and the names those entries read. Both
    // landmark rules hand their ids back as the threshold entry followed by every operator
    // entry it bound, and a node naming the threshold rule alone hides them: which crossing
    // each operator selected moves the sample its rule placed.
    //
    // What each rule read is the rule's own answer, from `landmarks_read` beside its dispatch,
    // rather than this function's guess. Nine of the ten landmark rules read the weighing
    // epoch and one does not, one of the five onset rules reads the takeoff and four do not,
    // so a single edge per construct stated here would name rules that did not contribute.
    //
    // A landmark a caller dragged rests on nothing, because no value any rule read produced
    // it. The rule's own record still names the marker, so the chain says a hand placed it.
    let mut placed: BTreeMap<&'static str, PlacedSample> = BTreeMap::new();
    placed.insert(
        crate::derived::WEIGHING_EPOCH,
        PlacedSample {
            index: Some(epoch.end_index),
            placed_by: vec![request.weighing.method_id.clone()],
            rests_on: Vec::new(),
            order: 0,
        },
    );
    placed.insert(
        crate::derived::MOVEMENT_ONSET,
        PlacedSample {
            index: onset_index,
            placed_by: onset_ids.clone(),
            rests_on: match request.onset.manual_index {
                Some(_) => Vec::new(),
                None => expect_landmarks_read(
                    movement_onset::landmarks_read(&request.onset.method_id),
                    &request.onset.method_id,
                ),
            },
            order: 1,
        },
    );
    placed.insert(
        crate::derived::TAKEOFF,
        PlacedSample {
            index: takeoff_index,
            placed_by: takeoff_ids_bound.clone(),
            rests_on: match request.takeoff.manual_index {
                Some(_) => Vec::new(),
                None => expect_landmarks_read(
                    takeoff_slot::landmarks_read(&request.takeoff.method_id),
                    &request.takeoff.method_id,
                ),
            },
            order: 2,
        },
    );
    placed.insert(
        crate::derived::TOUCHDOWN,
        PlacedSample {
            index: touchdown_index,
            // No entry of its own, because it is not a choice: it is the return above the
            // threshold the takeoff rule resolved, found by searching forward from the sample
            // that rule placed. A caller who states it has placed it by hand, and it then
            // rests on nothing either.
            placed_by: Vec::new(),
            rests_on: match touchdown_was_stated {
                true => Vec::new(),
                false => vec![crate::derived::TAKEOFF],
            },
            order: 3,
        },
    );

    // Every chain opens with what conditioned the signal, because every number below was
    // measured on the series those rules produced and none of them can be reproduced without
    // knowing which series that was. It is the one thing every rule reads, so it is stated
    // rather than asked for.
    let chain_over = |names: &[&'static str]| {
        let mut chain = conditioned.ids.clone();
        chain.extend(crate::derived::rules_behind(&placed, names));
        chain
    };

    // A quantity the request bound a rule for is reported by that rule, so the keys it bound
    // are settled before anything computes one. Read off the binding rows rather than off what
    // the rules produced, so a key is left out before it has a second value rather than
    // deduplicated afterwards: which of two values for one key survived a deduplication is an
    // ordering fact nobody stated.
    let bound_by_request = keys_the_request_bound(request);

    // Each of these is run rather than reproduced, so the arithmetic behind a number is the
    // arithmetic of the rule the number names.
    //
    // Nobody chose these rules, so each runs as the registry's default for its quantities and
    // says so through the record it leaves: the values it read are marked assumed unless the
    // request chose them. A default that reaches the record is a choice; one that does not is
    // an absence, which is the reason the conditioning phase runs its own default the same way.
    let spine_default = |method_id: &'static str,
                         bound_methods: &mut Vec<crate::resolution::BoundMethod>,
                         refusals: &mut Vec<DeclinedRule>,
                         warnings: &mut Vec<String>| {
        let binding = expect_row(method_id);
        if binding
            .quantities
            .iter()
            .any(|quantity| bound_by_request.contains(&quantity.key))
        {
            return Vec::new();
        }
        run_spine_default(
            binding,
            trial,
            request,
            &epoch,
            onset_index,
            takeoff_index,
            touchdown_index,
            landmarks.is_some(),
            &conditioned.ids,
            &placed,
            bound_methods,
            refusals,
            warnings,
        )
    };
    let interval_produced = spine_default(
        time_to_takeoff_rule::ID,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    );
    let flight_seconds_produced = spine_default(
        flight_time_seconds_rule::ID,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    );
    let impulse_produced = spine_default(
        net_impulse_rule::ID,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    );
    let flight_produced = spine_default(
        flight_time_rule::ID,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    );
    let takeoff_height_produced = spine_default(
        impulse_momentum_rule::ID,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    );
    let rsimod_produced = spine_default(
        rsimod_rule::ID,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    );

    let (interval_seconds, interval_chain) = number_and_chain(
        &interval_produced,
        crate::slots::time_to_takeoff::KEY,
        &conditioned.ids,
    );
    let (flight, flight_chain) = number_and_chain(
        &flight_seconds_produced,
        crate::slots::flight_time::KEY,
        &conditioned.ids,
    );
    let (net_impulse, net_impulse_chain) =
        number_and_chain(&impulse_produced, net_impulse::KEY, &conditioned.ids);
    let (takeoff_velocity, takeoff_velocity_chain) = number_and_chain(
        &impulse_produced,
        net_impulse::VELOCITY_KEY,
        &conditioned.ids,
    );
    let (flight_time_height, flight_time_height_chain) =
        number_and_chain(&flight_produced, FLIGHT_TIME_KEY, &conditioned.ids);
    let (takeoff_height, takeoff_height_chain) = number_and_chain(
        &takeoff_height_produced,
        jh_takeoff_frame::KEY,
        &conditioned.ids,
    );
    let (reactive_strength, reactive_strength_chain) = number_and_chain(
        &rsimod_produced,
        reactive_strength_index::KEY,
        &conditioned.ids,
    );

    // Every quantity's key, label, unit and computed-by come from the one declaration in
    // `response.rs`. What varies per analysis is the value, the chain behind it, and the
    // sentence beside it.
    let metrics = vec![
        Metric::declared(
            "system_weight_newtons",
            Some(epoch.system_weight_newtons),
            chain_over(&[crate::derived::WEIGHING_EPOCH]),
            Some("Includes any external load. System weight is not bodyweight.".into()),
        ),
        Metric::declared(
            "system_mass_kilograms",
            Some(epoch.system_mass_kilograms(gravity)),
            chain_over(&[crate::derived::WEIGHING_EPOCH]),
            Some("System weight over the gravity this analysis was bound to.".into()),
        ),
        Metric::declared(
            "onset_time_seconds",
            onset_index.map(|index| trial.time_at(index)),
            chain_over(&[crate::derived::MOVEMENT_ONSET]),
            None,
        ),
        Metric::declared(
            "takeoff_time_seconds",
            takeoff_index.map(|index| trial.time_at(index)),
            chain_over(&[crate::derived::TAKEOFF]),
            None,
        ),
        Metric::declared(
            "time_to_takeoff_seconds",
            interval_seconds,
            interval_chain,
            Some(
                "Bounded by two threshold crossings, which is why it is the least reproducible number here."
                    .into(),
            ),
        ),
        Metric::declared("flight_time_seconds", flight, flight_chain, None),
        Metric::declared(
            "takeoff_velocity_meters_per_second",
            takeoff_velocity,
            takeoff_velocity_chain,
            Some("Net impulse over system mass. An identity, not an estimate.".into()),
        ),
        Metric::declared(
            "net_impulse_newton_seconds",
            net_impulse,
            net_impulse_chain,
            None,
        ),
        Metric::declared(
            "jump_height_from_takeoff_meters",
            takeoff_height,
            takeoff_height_chain,
            Some(
                "Rise from the instant of takeoff. Not comparable with the standing frame without a declared correction."
                    .into(),
            ),
        ),
        Metric::declared(
            FLIGHT_TIME_KEY,
            flight_time_height,
            // The chain the rule's own phase built, so one id carries one record whichever way
            // a caller arrived at it.
            flight_time_height_chain,
            Some(
                "The projectile equation's estimate of the same takeoff-frame rise as the figure above. Higher by about 2.1 cm across nine unloaded studies, and lower under load."
                    .into(),
            ),
        ),
        Metric::declared(
            "reactive_strength_index_modified",
            reactive_strength,
            reactive_strength_chain,
            Some(
                "Impulse-momentum jump height over time to takeoff, so it inherits both choices. The registry carries a second numerator, rsimod.jh_ft_over_ttt, which uses flight-time height and is a different number."
                    .into(),
            ),
        ),
    ];

    let mut metrics = metrics;
    // Computed here as well as by the rule the request named, a key would carry two values, and
    // the surfaces that look a key up resolve that in opposite directions: one takes the first
    // match and one takes the last.
    metrics.retain(|metric| !bound_by_request.contains(&metric.key.as_str()));

    run_derived_phase(
        trial,
        request,
        &epoch,
        &conditioned.ids,
        placed,
        onset_index,
        takeoff_index,
        touchdown_index,
        &mut metrics,
        &mut bound_methods,
        &mut refusals,
        &mut warnings,
    )?;

    let mut response = AnalysisResponse {
        samples_carrying_no_number,
        weighing_start_index: epoch.start_index,
        weighing_end_index: epoch.end_index,
        onset_index,
        takeoff_index,
        touchdown_index,
        levels: Levels {
            system_weight_newtons: crate::response::drawable(epoch.system_weight_newtons),
            weighing_standard_deviation_newtons: crate::response::drawable(
                epoch.standard_deviation_newtons,
            ),
            onset_band_lower_newtons: crate::response::drawable(
                epoch.system_weight_newtons - onset_band,
            ),
            onset_band_upper_newtons: crate::response::drawable(
                epoch.system_weight_newtons + onset_band,
            ),
            takeoff_threshold_newtons: crate::response::drawable(takeoff.threshold_newtons),
        },
        bound_methods,
        bound_globals: request.bound_globals(),
        metrics,
        weighing_epoch_tied_window_count: epoch.tied_window_count,
        warnings,
        refusals,
        signals: Vec::new(),
    };
    // Last, because a signal reads the finished result: it compares two numbers this
    // analysis produced against each other.
    response.signals = crate::quality::signals(&response);
    Ok(response)
}

/// The row this build holds for an id, which is where a rule's construct, its quantities and
/// the function behind it are declared together.
///
/// Panics rather than falling back, because the caller is the spine reaching for a rule it
/// names in its own source: an id with no row is this file and `BINDINGS` disagreeing about
/// what this build runs, which no result should be produced under.
fn expect_row(method_id: &'static str) -> &'static crate::binding::Binding {
    crate::binding::BINDINGS
        .iter()
        .find(|binding| binding.id == method_id)
        .unwrap_or_else(|| panic!("{method_id} has no row in the binding table"))
}

/// What a landmark rule says it reads, or the end of the analysis where this build files no
/// rule under the id.
///
/// Panics rather than falling back to nothing, for the reason `expect_row` panics: an id whose
/// rule ran and whose reading is unanswered is this file and the slot disagreeing about what
/// the build contains. Falling back to an empty list would publish a chain claiming the number
/// rests on no landmark at all, which is the shape that reads as a finished answer and is not
/// one. `run` reaches here only for an id `expect_bound` has already accepted.
fn expect_landmarks_read(
    declared: Option<&'static [&'static str]>,
    method_id: &str,
) -> Vec<&'static str> {
    declared
        .unwrap_or_else(|| panic!("{method_id} ran without saying which landmarks it reads"))
        .to_vec()
}

/// The chain behind one number a rule computed: what conditioned the signal, the entries
/// behind every sample the rule asked for and behind everything those rest on, and the
/// entries it declared this number rests on.
///
/// One home for the whole shape, because the spine runs some of these rules itself and a
/// second copy would be free to answer the same question differently.
///
/// It opens with what conditioned the signal, because the number was measured on the series
/// those rules produced and cannot be reproduced without knowing which series that was. That
/// is the one thing stated rather than asked for, because it is the one thing every rule
/// reads.
fn chain_behind(
    context: &DerivedContext,
    quantity_key: &str,
    conditioning_ids: &[String],
) -> Vec<String> {
    let mut chain = conditioning_ids.to_vec();
    chain.extend(context.rules_read());
    chain.extend(context.entries_behind(quantity_key));
    chain
}

/// One number a rule the spine ran for itself produced, carrying the chain that rule's own
/// phase would have built for it, so one id leaves one record whichever way a caller arrived.
struct SpineQuantity {
    key: &'static str,
    value: Option<f64>,
    chain: Vec<String>,
}

/// One quantity among what a spine-run rule produced, and the chain behind it.
///
/// The fallback covers the case where the request named the rule itself: the spine does not
/// run it then, and the metric this fills is dropped before anything reads it. It carries the
/// conditioning ids and nothing else, because no rule ran here to ask for anything.
fn number_and_chain(
    produced: &[SpineQuantity],
    key: &str,
    conditioning_ids: &[String],
) -> (Option<f64>, Vec<String>) {
    produced
        .iter()
        .find(|quantity| quantity.key == key)
        .map(|quantity| (quantity.value, quantity.chain.clone()))
        .unwrap_or_else(|| (None, conditioning_ids.to_vec()))
}

/// A quantity the spine reports whose arithmetic is a registry entry, produced by running that
/// entry rather than by repeating it in the spine.
///
/// The entry publishes its own gravity and declares 9.81, so running it is what keeps one id
/// from returning two numbers on one trial.
///
/// Nobody named the rule here, so it runs on nothing the caller stated and the record says so
/// by itself: every value it read is marked assumed unless the request chose it elsewhere. A
/// default that reaches the record is a choice; one that does not is an absence.
#[allow(clippy::too_many_arguments)]
fn run_spine_default(
    binding: &'static crate::binding::Binding,
    trial: &Trial,
    request: &AnalysisRequest,
    epoch: &plateforce_core::WeighingEpoch,
    onset_index: Option<usize>,
    takeoff_index: Option<usize>,
    touchdown_index: Option<usize>,
    landmarks_were_placed: bool,
    conditioning_ids: &[String],
    placed: &BTreeMap<&'static str, PlacedSample>,
    bound_methods: &mut Vec<crate::resolution::BoundMethod>,
    refusals: &mut Vec<DeclinedRule>,
    warnings: &mut Vec<String>,
) -> Vec<SpineQuantity> {
    let Dispatch::Derived(rule) = binding.dispatch else {
        return Vec::new();
    };
    // The spine's landmarks and nothing else. No rule filed under a construct has run at this
    // point, so a rule the spine runs for itself reads the landmarks and its own parameters,
    // and reaches them through the same map by the same names a rule reached by construct
    // does. That is what makes one id leave one chain whichever way a caller arrived: both
    // routes ask, and both are answered from here.
    let context = DerivedContext::new(
        trial,
        epoch,
        onset_index,
        takeoff_index,
        touchdown_index,
        request.gravity_meters_per_second_squared,
        request.gravity_source,
        request.body_mass_kilograms,
        placed,
        &request.derived,
    );
    let choice = MethodChoice {
        method_id: binding.id.to_string(),
        ..Default::default()
    };
    let outcome = rule(&context, &choice, warnings);
    bound_methods.push(bound_method(
        binding.id,
        outcome.bound,
        request.is_backed(binding.id),
        false,
    ));
    // A rule the spine ran for itself declines out loud, on the same terms as one the caller
    // named. Five of the six trials in the conformance corpus report no flight time, because
    // the recording never returns to the plate after takeoff.
    //
    // The exception is a recording no landmark was placed on. There the rules that failed to
    // place them have already recorded why under their own names, or the spine has warned that
    // onset sits at or after takeoff, so a rule declining for want of what they did not produce
    // is a second record of one cause rather than a new fact.
    if let Some(rejected) = outcome.refusal {
        if landmarks_were_placed {
            warnings.push(rejected.to_string());
            refusals.push(DeclinedRule {
                construct: binding.construct,
                method_id: binding.id.to_string(),
                refusal: rejected,
            });
        }
    }
    // Every quantity the row declares, whether or not the rule produced it. A rule that
    // declined still consulted something, and the chain is what it consulted: a number's
    // absence has a cause, the cause is a rule on that chain, and a reader who cannot see the
    // chain cannot reach it. `spread.rs` reads exactly this to say why a swept variant came
    // back empty, and it will only name a rule the quantity itself says it rests on.
    //
    // Read off the row rather than off what ran, for the reason `keys_the_request_bound` is:
    // what a rule reports is settled by its declaration, before anything computes one.
    binding
        .quantities
        .iter()
        .map(|declared| SpineQuantity {
            key: declared.key,
            value: outcome
                .values
                .iter()
                .find(|(key, _)| *key == declared.key)
                .and_then(|(_, value)| *value),
            chain: chain_behind(&context, declared.key, conditioning_ids),
        })
        .collect()
}

/// Every quantity key a rule the request named will report.
///
/// Read off the binding rows rather than off what the rules produced, so a key is left out
/// before anything computes it rather than deduplicated afterwards. Which of two values for
/// one key survived a deduplication is an ordering fact nobody stated.
fn keys_the_request_bound(request: &AnalysisRequest) -> Vec<&'static str> {
    request
        .derived
        .iter()
        .flat_map(|(construct, choice)| {
            crate::binding::bindings_for_construct(construct)
                .filter(move |binding| binding.id == choice.method_id)
                .flat_map(|binding| binding.quantities.iter().map(|quantity| quantity.key))
        })
        .collect()
}

/// Every rule the request named for a construct computed from the landmarks, in the order
/// `BINDINGS` declares them.
///
/// Declaration order is the whole of the ordering rule: a rule that reads what another rule
/// placed is declared after it. That is checked rather than trusted, by
/// `a_rule_reading_a_placed_sample_is_declared_after_the_rule_that_places_it`.
///
/// A construct named with no rule behind it, or an id that is not the rule filed under the
/// construct it was named for, ends the analysis rather than being skipped. Skipping would
/// answer a request for peak force with a result carrying no peak force and nothing saying
/// why.
#[allow(clippy::too_many_arguments)]
fn run_derived_phase(
    trial: &Trial,
    request: &AnalysisRequest,
    epoch: &plateforce_core::WeighingEpoch,
    conditioning_ids: &[String],
    mut placed: BTreeMap<&'static str, PlacedSample>,
    onset_index: Option<usize>,
    takeoff_index: Option<usize>,
    touchdown_index: Option<usize>,
    metrics: &mut Vec<Metric>,
    bound_methods: &mut Vec<crate::resolution::BoundMethod>,
    refusals: &mut Vec<DeclinedRule>,
    warnings: &mut Vec<String>,
) -> Result<(), Box<plateforce_core::Refusal>> {
    if request.derived.is_empty() {
        return Ok(());
    }
    for (construct, choice) in &request.derived {
        expect_derived_choice(construct, &choice.method_id)?;
    }

    // The map arrives holding the spine's four landmarks, so a rule reaching one of them and a
    // rule reaching a sample an earlier construct placed are answered from one place and the
    // chain closes over both alike.
    let mut next_order = placed.len();
    for binding in crate::binding::derived_bindings() {
        let Some(choice) = request.derived.get(binding.construct) else {
            continue;
        };
        if choice.method_id != binding.id {
            continue;
        }
        let Dispatch::Derived(rule) = binding.dispatch else {
            continue;
        };

        let context = DerivedContext::new(
            trial,
            epoch,
            onset_index,
            takeoff_index,
            touchdown_index,
            request.gravity_meters_per_second_squared,
            request.gravity_source,
            request.body_mass_kilograms,
            &placed,
            &request.derived,
        );
        let outcome = rule(&context, choice, warnings);

        // A record naming only the last step understates what produced the number, and one
        // naming every earlier step cites rules it never used. Built per quantity, because two
        // numbers under one entry can rest on different things.
        for (key, value) in &outcome.values {
            let declared = binding
                .quantities
                .iter()
                .find(|quantity| quantity.key == *key)
                .unwrap_or_else(|| {
                    panic!(
                        "{} reported {key}, which its binding row does not declare",
                        binding.id
                    )
                });
            metrics.push(Metric::from_declaration(
                declared,
                *value,
                chain_behind(&context, key, conditioning_ids),
                None,
            ));
        }
        // What this rule read travels with what it placed, so a later rule reading the sample
        // reaches the rules behind it without naming any of them itself. A sample carrying
        // only the rule that placed it would stop the chain one construct short.
        let read_by_the_placing_rule = context.names_read();
        for (name, index) in outcome.placed {
            placed.insert(
                name,
                PlacedSample {
                    index: Some(index),
                    placed_by: vec![binding.id.to_string()],
                    rests_on: read_by_the_placing_rule.clone(),
                    order: next_order,
                },
            );
            next_order += 1;
        }
        bound_methods.push(bound_method(
            binding.id,
            outcome.bound,
            request.is_backed(binding.id),
            choice.manual_index.is_some(),
        ));
        if let Some(rejected) = outcome.refusal {
            warnings.push(rejected.to_string());
            refusals.push(DeclinedRule {
                construct: binding.construct,
                method_id: binding.id.to_string(),
                refusal: rejected,
            });
        }
    }
    Ok(())
}

/// What the conditioning phase settled: the signal everything below reads, the rules that
/// produced it, and their records.
struct Conditioned {
    /// `None` where every rule that ran was the identity, so the recording is used as it
    /// was digitised and no copy of it is made.
    trial: Option<Trial>,
    /// The ids, in the order they ran, to open every chain with.
    ids: Vec<String>,
    bound_methods: Vec<crate::resolution::BoundMethod>,
}

/// Every construct this build conditions with, run in declaration order.
///
/// A construct the request does not name still runs, under the rule declared as its default.
/// That is the whole point of the phase: before it existed the software applied no filter and
/// said nothing, so a reader could not tell an unfiltered signal from a filtered one whose
/// filter nobody wrote down. A default that reaches the record is a choice; one that does not
/// is an absence.
fn run_conditioning_phase(
    trial: &Trial,
    request: &AnalysisRequest,
    warnings: &mut Vec<String>,
) -> Result<Conditioned, Box<plateforce_core::Refusal>> {
    for (construct, choice) in &request.conditioning {
        expect_conditioning_choice(construct, &choice.method_id)?;
    }

    let mut settled = Conditioned {
        trial: None,
        ids: Vec::new(),
        bound_methods: Vec::new(),
    };
    for construct in crate::binding::conditioning_constructs() {
        let stated = request.conditioning.get(construct);
        let method_id = stated
            .map(|choice| choice.method_id.as_str())
            .unwrap_or(crate::slots::conditioned_force_signal::DECLARED_DEFAULT);
        let Some(binding) = crate::binding::conditioning_bindings().find(|b| b.id == method_id)
        else {
            continue;
        };
        let Dispatch::Conditioning(rule) = binding.dispatch else {
            continue;
        };

        // A construct the request did not name is run under the default, and the choice it
        // is run with says so, so the record distinguishes the software's pick from a
        // caller's without a second field to hold the distinction.
        let chosen = stated.cloned().unwrap_or_else(|| MethodChoice {
            method_id: method_id.to_string(),
            from_registry_default: [construct.to_string()].into_iter().collect(),
            ..Default::default()
        });
        let source = settled.trial.as_ref().unwrap_or(trial);
        let outcome = rule(source, &chosen, warnings);
        if let Some(force) = outcome.force_newtons {
            settled.trial = Some(
                Trial::new(force, trial.sample_rate_hz())
                    .map_err(|error| Box::new(plateforce_core::Refusal::from(error)))?,
            );
        }
        settled.ids.push(binding.id.to_string());
        settled.bound_methods.push(bound_method(
            binding.id,
            outcome.bound,
            request.is_backed(binding.id),
            false,
        ));
    }
    Ok(settled)
}

/// A construct and an id the request named together, checked against what this build
/// conditions with. Both halves, for the same reason the derived phase checks both: either
/// alone lets a request through that the loop would then skip in silence, and a skipped
/// conditioning choice is a filter the caller asked for and did not get.
fn expect_conditioning_choice(
    construct: &str,
    method_id: &str,
) -> Result<(), Box<plateforce_core::Refusal>> {
    let constructs = crate::binding::conditioning_constructs();
    if !constructs.contains(&construct) {
        return Err(Box::new(
            plateforce_core::Refusal::construct_not_on_the_path(
                construct,
                constructs.into_iter().map(str::to_string).collect(),
            ),
        ));
    }
    if crate::binding::conditioning_bindings().any(|binding| binding.id == method_id) {
        return Ok(());
    }
    Err(Box::new(plateforce_core::Refusal::method_not_implemented(
        method_id,
        construct,
        crate::binding::conditioning_bindings()
            .map(|binding| binding.id.to_string())
            .collect(),
    )))
}

/// A construct and an id the request named together, checked against what this build runs.
///
/// Both halves, because either one alone lets a request through that the loop would then
/// skip in silence: a construct with no rule matches no binding, and an id that is not the
/// rule filed under the construct it was named for matches no binding either. A skipped
/// request comes back as a result missing the number that was asked for, saying nothing.
fn expect_derived_choice(
    construct: &str,
    method_id: &str,
) -> Result<(), Box<plateforce_core::Refusal>> {
    let constructs = crate::binding::derived_constructs();
    if !constructs.contains(&construct) {
        return Err(Box::new(
            plateforce_core::Refusal::construct_not_on_the_path(
                construct,
                constructs.into_iter().map(str::to_string).collect(),
            ),
        ));
    }
    if crate::binding::bindings_for_construct(construct).any(|binding| binding.id == method_id) {
        return Ok(());
    }
    Err(Box::new(plateforce_core::Refusal::method_not_implemented(
        method_id,
        construct,
        crate::binding::bindings_for_construct(construct)
            .map(|binding| binding.id.to_string())
            .collect(),
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use plateforce_core::trial::CentralTendency;
    use plateforce_core::{
        DispersionEstimator, RefusalCode, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    };

    use super::*;
    use crate::binding::{BINDINGS, WEIGHING_CONSTRUCT};
    use crate::request::{MethodChoice, WeighingChoice};
    use crate::slots::system_weight::{weighing_epoch_at, window_length_parameter};

    fn synthetic() -> Trial {
        let mut force = vec![600.0; 1200];
        for (index, value) in force.iter_mut().enumerate() {
            *value += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
        force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
        force.extend(std::iter::repeat_n(0.0, 600));
        force.extend(std::iter::repeat_n(1400.0, 240));
        Trial::new(force, 1200.0).unwrap()
    }

    fn weighing(
        method_id: &str,
        start_index: Option<usize>,
        duration_seconds: f64,
    ) -> WeighingChoice {
        WeighingChoice {
            method_id: method_id.into(),
            start_index,
            parameters: BTreeMap::from([(
                window_length_parameter(method_id).to_string(),
                duration_seconds,
            )]),
            options: BTreeMap::new(),
            ..Default::default()
        }
    }

    fn request(onset_id: &str, takeoff_id: &str) -> AnalysisRequest {
        AnalysisRequest {
            weighing: weighing("bwepoch.fixed_window", None, 0.8),
            onset: MethodChoice {
                method_id: onset_id.into(),
                ..Default::default()
            },
            takeoff: MethodChoice {
                method_id: takeoff_id.into(),
                ..Default::default()
            },
            touchdown_index: None,
            gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            registry_backed_ids: vec!["onset.threshold.noise_relative".into()],
            ..Default::default()
        }
    }

    /// A request naming the rule under test, and the first rule of every construct declared
    /// before its own so anything it reads has been placed. Declaration order is the whole
    /// of the ordering rule, so building the request this way is also what exercises it.
    fn request_reaching(binding: &crate::binding::Binding) -> AnalysisRequest {
        let mut candidate = request(
            "onset.threshold.noise_relative",
            "takeoff.threshold.absolute_force",
        );
        // A conditioning rule is reached through its own map, because it runs before the
        // spine rather than over what the spine placed.
        if matches!(binding.dispatch, Dispatch::Conditioning(_)) {
            candidate.conditioning.insert(
                binding.construct.to_string(),
                MethodChoice {
                    method_id: binding.id.to_string(),
                    ..Default::default()
                },
            );
            return candidate;
        }
        match binding.slot {
            "onset" => candidate.onset.method_id = binding.id.to_string(),
            "takeoff" => candidate.takeoff.method_id = binding.id.to_string(),
            "weighing" => candidate.weighing.method_id = binding.id.to_string(),
            _ => {
                // Whatever an entry states required with no default is answered here. A rule
                // that cannot run unasked is the entry working, and leaving it unanswered
                // would make every rule downstream of it undemonstrable too.
                let choosing = |chosen: &crate::binding::Binding| MethodChoice {
                    method_id: chosen.id.to_string(),
                    options: crate::binding::required_options(chosen.id)
                        .iter()
                        .map(|(name, value)| (name.to_string(), value.to_string()))
                        .collect(),
                    ..Default::default()
                };
                for earlier in crate::binding::derived_bindings() {
                    if earlier.construct == binding.construct {
                        break;
                    }
                    candidate
                        .derived
                        .entry(earlier.construct.to_string())
                        .or_insert_with(|| choosing(earlier));
                }
                candidate
                    .derived
                    .insert(binding.construct.to_string(), choosing(binding));
            }
        }
        candidate
    }

    #[test]
    fn every_binding_this_build_advertises_actually_runs() {
        let trial = synthetic();
        let mut checked = 0usize;
        for binding in BINDINGS {
            let candidate = request_reaching(binding);
            let response = run(&trial, &candidate)
                .unwrap_or_else(|error| panic!("{} failed to run: {error}", binding.id));
            assert!(
                !response.metrics.is_empty(),
                "{} produced no metrics",
                binding.id
            );
            checked += 1;
        }
        println!("{checked} of {} bindings ran", BINDINGS.len());
        assert_eq!(checked, BINDINGS.len());
    }

    /// A rule that advertises a quantity produces it, or says why it did not. Silence is the
    /// third answer and it is the one this guard exists to forbid: a request naming a rule
    /// that comes back with neither the number nor a refusal reads as a rule that ran.
    #[test]
    fn every_rule_computed_from_the_landmarks_reports_what_it_declares() {
        let trial = synthetic();
        let mut reported = 0usize;
        for binding in crate::binding::derived_bindings() {
            let response =
                run(&trial, &request_reaching(binding)).expect("the request is well formed");
            // The "or says why it did not" half. A rule may decline, and only for a reason
            // that names something outside itself: a value only the caller supplies, an
            // answer another construct owes it, or a search this recording gave nothing to.
            // Any other code would be the rule failing rather than reporting.
            if let Some(declined) = response
                .refusals
                .iter()
                .find(|rule| rule.method_id == binding.id)
            {
                let crate::RuleRefusal::Refused(refusal) = &declined.refusal else {
                    panic!(
                        "{} declined with a trial error: {}",
                        binding.id, declined.refusal
                    )
                };
                assert!(
                    matches!(
                        refusal.code,
                        plateforce_core::RefusalCode::RequiredParameterUnstated
                            | plateforce_core::RefusalCode::DependencyUnresolved
                            | plateforce_core::RefusalCode::DecisionNotMade
                            | plateforce_core::RefusalCode::NoCrossing
                    ),
                    "{} declined under {:?}, which is the rule failing rather than reporting: {}",
                    binding.id,
                    refusal.code,
                    declined.refusal
                );
                continue;
            }
            for declared in binding.quantities {
                let metric = response
                    .metrics
                    .iter()
                    .find(|metric| metric.key == declared.key)
                    .unwrap_or_else(|| {
                        panic!(
                            "{} declares {} and reported no metric for it",
                            binding.id, declared.key
                        )
                    });
                assert_eq!(
                    metric.computed_by.as_deref(),
                    Some(binding.id),
                    "{} reported {} under another rule's name",
                    binding.id,
                    declared.key
                );
                // And it reported it once. Two values under one key reach the surfaces that
                // look a key up as two different answers, because one takes the first match
                // and one takes the last.
                assert_eq!(
                    response
                        .metrics
                        .iter()
                        .filter(|metric| metric.key == declared.key)
                        .count(),
                    1,
                    "{} reported {} beside another value under the same key",
                    binding.id,
                    declared.key
                );
                assert!(
                    metric.value.is_some(),
                    "{} reported {} with no value and no refusal",
                    binding.id,
                    declared.key
                );
                reported += 1;
            }
        }
        println!("{reported} declared quantities reported by the rules that declare them");
        // The population this guard was written against. A guard whose subject shrank below
        // it would pass by having less to read.
        assert!(reported >= 7, "only {reported} quantities were checked");
    }

    /// The ordering is declaration order and nothing else, so a rule reading a sample another
    /// rule places has to be declared after it. Held by running each rule with only the rules
    /// declared before it available: a rule that needs a later one declines here.
    ///
    /// A decline is read by what the rule said it wanted, not by its code alone. Two declines
    /// carry one code and mean opposite things: a rule wanting a construct this build runs a
    /// rule for was handed every earlier one, so it is declared too early, while a rule wanting
    /// a construct this build has no rule for at all is reporting coverage and says nothing
    /// about order. Excluding the code would exclude the ordering fault with it, so the
    /// constructs the refusal names are what decide.
    #[test]
    fn a_rule_reading_a_placed_sample_is_declared_after_the_rule_that_places_it() {
        let trial = synthetic();
        let runnable = crate::binding::executable_constructs();
        for binding in crate::binding::derived_bindings() {
            let response =
                run(&trial, &request_reaching(binding)).expect("the request is well formed");
            // A value the caller never stated is a different fault from a rule that ran too
            // early, and only the second is this guard's subject. Reading them as one would
            // make a rule whose entry publishes no default for a required name unbindable.
            let declined: Vec<&str> = response
                .refusals
                .iter()
                .filter(|rule| rule.method_id == binding.id)
                .filter(|rule| {
                    let refusal = crate::document::refusal_from_rule(rule);
                    match refusal.code {
                        // A value only the caller can supply, which `required_options`
                        // answers for every rule whose entry states one.
                        RefusalCode::RequiredParameterUnstated => false,
                        // A search this recording gave nothing to.
                        RefusalCode::NoCrossing => false,
                        RefusalCode::DependencyUnresolved | RefusalCode::DecisionNotMade => refusal
                            .available
                            .iter()
                            .any(|construct| runnable.contains(&construct.as_str())),
                        _ => true,
                    }
                })
                .map(|rule| rule.method_id.as_str())
                .collect();
            assert!(
                declined.is_empty(),
                "{} declined with only the rules declared before it available, so what it reads is declared after it: {}",
                binding.id,
                response
                    .refusals
                    .iter()
                    .map(|rule| rule.refusal.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
    }

    #[test]
    fn the_weighing_bindings_all_produce_a_window() {
        let trial = synthetic();
        for method_id in ["bwepoch.fixed_window", "bwepoch.adaptive_lowest_variance"] {
            let mut candidate = request(
                "onset.threshold.noise_relative",
                "takeoff.threshold.absolute_force",
            );
            candidate.weighing.method_id = method_id.to_string();
            let response = run(&trial, &candidate).unwrap();
            assert!(
                response.weighing_end_index > response.weighing_start_index,
                "{method_id}"
            );
        }
    }

    #[test]
    fn a_moved_weighing_window_keeps_the_weight_and_restates_the_indices() {
        let trial = synthetic();
        let anchored = weighing_epoch_at(
            &trial,
            0,
            0.5,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        let moved = weighing_epoch_at(
            &trial,
            240,
            0.5,
            CentralTendency::Mean,
            DispersionEstimator::Sample,
        )
        .unwrap();
        assert_eq!(moved.start_index, 240);
        assert_eq!(moved.end_index, 240 + 600);
        assert!((moved.system_weight_newtons - anchored.system_weight_newtons).abs() < 5.0);
    }

    #[test]
    fn every_metric_names_the_methods_that_produced_it() {
        let response = run(
            &synthetic(),
            &request(
                "onset.threshold.noise_relative",
                "takeoff.threshold.absolute_force",
            ),
        )
        .unwrap();
        for metric in &response.metrics {
            assert!(
                !metric.contributing_method_ids.is_empty(),
                "{} carries no provenance",
                metric.key
            );
        }
    }

    #[test]
    fn a_method_absent_from_the_registry_is_marked_unbacked_rather_than_hidden() {
        let response = run(
            &synthetic(),
            &request(
                "onset.threshold.noise_relative",
                "takeoff.threshold.absolute_force",
            ),
        )
        .unwrap();
        let takeoff = response
            .bound_methods
            .iter()
            .find(|m| m.method_id.starts_with("takeoff."))
            .unwrap();
        assert!(!takeoff.registry_backed);
    }

    #[test]
    fn dragging_a_marker_is_recorded_as_an_override() {
        let mut candidate = request(
            "onset.threshold.noise_relative",
            "takeoff.threshold.absolute_force",
        );
        candidate.onset.manual_index = Some(1100);
        let response = run(&synthetic(), &candidate).unwrap();
        assert_eq!(response.onset_index, Some(1100));
        assert!(response.bound_methods.iter().any(|m| m.manual_override));
    }

    /// The picker cannot reach an id with no rule behind it, and the module surface can. A
    /// rule run under somebody else's id would put that author's citation on the answer.
    ///
    /// The step is named as the registry names constructs. `weighing` and `onset` are the
    /// binding table's own words and resolve to nothing in the registry, so a caller reading
    /// one of those in a refusal holds a name it cannot look up.
    #[test]
    fn an_id_with_no_rule_behind_it_is_refused_rather_than_run_as_something_else() {
        let trial = synthetic();
        for (slot, construct, method_id) in [
            ("weighing", WEIGHING_CONSTRUCT, "bwepoch.robust_estimator"),
            ("onset", ONSET_CONSTRUCT, "onset.yank_inflection.sahrom2020"),
            (
                "takeoff",
                TAKEOFF_CONSTRUCT,
                "takeoff.system_weight_decrease.pinto2024",
            ),
        ] {
            let mut candidate = request(
                "onset.threshold.noise_relative",
                "takeoff.threshold.absolute_force",
            );
            match slot {
                "weighing" => candidate.weighing.method_id = method_id.to_string(),
                "onset" => candidate.onset.method_id = method_id.to_string(),
                _ => candidate.takeoff.method_id = method_id.to_string(),
            }
            let refused = run(&trial, &candidate)
                .expect_err(&format!("{method_id} ran under a rule that is not it"));
            // The code is a field rather than a word inside the sentence, so a caller that
            // reaches this branches on it without reading the prose.
            assert_eq!(
                refused.code,
                plateforce_core::RefusalCode::MethodNotImplemented
            );
            assert_eq!(refused.method_id, method_id);
            assert_eq!(refused.slot.as_deref(), Some(construct));
        }
    }

    /// A dragged marker still leaves the id to be honoured, so the refusal has to happen
    /// before the override is read.
    #[test]
    fn an_unbound_id_is_refused_even_when_a_marker_was_dragged() {
        let mut candidate = request(
            "onset.yank_inflection.sahrom2020",
            "takeoff.threshold.absolute_force",
        );
        candidate.onset.manual_index = Some(1100);
        assert!(run(&synthetic(), &candidate).is_err());
    }

    fn two_flight_phases() -> Trial {
        let mut force = vec![600.0; 1200];
        force.extend(std::iter::repeat_n(0.0, 400));
        force.extend(std::iter::repeat_n(600.0, 300));
        force.extend(std::iter::repeat_n(0.0, 1200));
        Trial::new(force, 1200.0).unwrap()
    }

    /// A weight shift above system weight before the countermovement, the only shape on
    /// which a band open on both sides and one open below disagree.
    fn pre_movement_bump() -> Trial {
        let mut force = vec![600.0; 1000];
        for (index, sample) in force.iter_mut().enumerate() {
            *sample += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend(std::iter::repeat_n(680.0, 120));
        force.extend(std::iter::repeat_n(600.0, 240));
        force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
        force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
        force.extend(std::iter::repeat_n(0.0, 600));
        force.extend(std::iter::repeat_n(1400.0, 240));
        Trial::new(force, 1200.0).unwrap()
    }

    /// The rule that skips past the first qualifying flight phase has to say so. Two open
    /// tools do this on 155 of 244 trials and report nothing.
    #[test]
    fn the_longest_run_rule_warns_when_it_skips_the_first_flight_phase() {
        let trial = two_flight_phases();

        let mut candidate = request(
            "onset.threshold.noise_relative",
            "takeoff.threshold.longest_run",
        );
        candidate.weighing = weighing(&candidate.weighing.method_id.clone(), None, 0.5);
        let response = run(&trial, &candidate).unwrap();
        assert!(
            response
                .warnings
                .iter()
                .any(|w| w.contains("qualifying flight phases")),
            "silent misplacement went unreported: {:?}",
            response.warnings
        );
    }

    type CaseParameters = &'static [(&'static str, f64)];
    type CaseOptions = &'static [(&'static str, &'static str)];
    /// Name, rule, where the window starts, how long it runs, and what else was stated.
    type WeighingCase = (
        &'static str,
        &'static str,
        Option<usize>,
        f64,
        CaseParameters,
        CaseOptions,
    );

    const ONSET_CASES: &[(&str, &str, CaseParameters, CaseOptions)] = &[
        (
            "noise_relative bare",
            "onset.threshold.noise_relative",
            &[],
            &[],
        ),
        (
            "noise_relative k",
            "onset.threshold.noise_relative",
            &[("k", 2.0)],
            &[],
        ),
        (
            "noise_relative direction below_only",
            "onset.threshold.noise_relative",
            &[],
            &[("direction", "below_only")],
        ),
        (
            "noise_relative direction two_sided",
            "onset.threshold.noise_relative",
            &[],
            &[("direction", "two_sided")],
        ),
        (
            "noise_relative selection last",
            "onset.threshold.noise_relative",
            &[],
            &[("selection", "last")],
        ),
        (
            "noise_relative selection first",
            "onset.threshold.noise_relative",
            &[],
            &[("selection", "first")],
        ),
        (
            "noise_relative persistence",
            "onset.threshold.noise_relative",
            &[("span_ms", 10.0)],
            &[],
        ),
        (
            "noise_relative search floor",
            "onset.threshold.noise_relative",
            &[("floor_seconds", 0.9)],
            &[],
        ),
        (
            "noise_relative back offset",
            "onset.threshold.noise_relative",
            &[("offset_ms", 50.0)],
            &[],
        ),
        (
            "noise_relative degenerate fraction",
            "onset.threshold.noise_relative",
            &[("degenerate_fraction", 0.2)],
            &[],
        ),
        (
            "noise_relative every value stated",
            "onset.threshold.noise_relative",
            &[
                ("k", 3.0),
                ("span_ms", 5.0),
                ("floor_seconds", 0.85),
                ("offset_ms", 20.0),
                ("degenerate_fraction", 0.1),
            ],
            &[("direction", "below_only"), ("selection", "last")],
        ),
        (
            "noise_relative names another rule carries",
            "onset.threshold.noise_relative",
            &[("threshold_n", 50.0), ("pct", 1.0)],
            &[],
        ),
        (
            "relative_to_system_weight bare",
            "onset.threshold.relative_to_system_weight",
            &[],
            &[],
        ),
        (
            "relative_to_system_weight pct",
            "onset.threshold.relative_to_system_weight",
            &[("pct", 5.0)],
            &[],
        ),
        (
            "relative_to_system_weight superseded spelling",
            "onset.threshold.relative_to_system_weight",
            &[("percent", 5.0)],
            &[],
        ),
        (
            "relative_to_system_weight every value stated",
            "onset.threshold.relative_to_system_weight",
            &[
                ("pct", 10.0),
                ("floor_seconds", 0.85),
                ("span_ms", 4.0),
                ("offset_ms", 10.0),
            ],
            &[("selection", "last")],
        ),
        (
            "absolute_force bare",
            "onset.threshold.absolute_force",
            &[],
            &[],
        ),
        (
            "absolute_force threshold",
            "onset.threshold.absolute_force",
            &[("threshold_n", 50.0)],
            &[],
        ),
        (
            "absolute_force superseded spelling",
            "onset.threshold.absolute_force",
            &[("threshold_newtons", 50.0)],
            &[],
        ),
        (
            "absolute_force direction two_sided",
            "onset.threshold.absolute_force",
            &[],
            &[("direction", "two_sided")],
        ),
        (
            "absolute_force direction below_only",
            "onset.threshold.absolute_force",
            &[],
            &[("direction", "below_only")],
        ),
        (
            "absolute_force every value stated",
            "onset.threshold.absolute_force",
            &[
                ("threshold_n", 40.0),
                ("floor_seconds", 0.82),
                ("span_ms", 6.0),
                ("offset_ms", 40.0),
            ],
            &[("direction", "two_sided"), ("selection", "last")],
        ),
        (
            "last_within_band bare",
            "onset.threshold.last_within_band",
            &[],
            &[],
        ),
        (
            "last_within_band k",
            "onset.threshold.last_within_band",
            &[("k", 3.0)],
            &[],
        ),
        (
            "last_within_band inverse lookback",
            "onset.threshold.last_within_band",
            &[("inverse_lookback_seconds", 0.25)],
            &[],
        ),
        (
            "last_within_band back offset",
            "onset.threshold.last_within_band",
            &[("offset_ms", 50.0)],
            &[],
        ),
        (
            "last_within_band every value stated",
            "onset.threshold.last_within_band",
            &[
                ("k", 2.0),
                ("inverse_lookback_seconds", 0.75),
                ("offset_ms", 10.0),
            ],
            &[("selection", "last")],
        ),
        (
            "adaptive_trailing_window bare",
            "onset.threshold.adaptive_trailing_window",
            &[],
            &[],
        ),
        (
            "adaptive_trailing_window window",
            "onset.threshold.adaptive_trailing_window",
            &[("window_seconds", 0.25)],
            &[],
        ),
        (
            "adaptive_trailing_window k",
            "onset.threshold.adaptive_trailing_window",
            &[("k", 3.0)],
            &[],
        ),
        (
            "adaptive_trailing_window population",
            "onset.threshold.adaptive_trailing_window",
            &[],
            &[("dispersion", "population")],
        ),
        (
            "adaptive_trailing_window sample",
            "onset.threshold.adaptive_trailing_window",
            &[],
            &[("dispersion", "sample")],
        ),
        (
            "adaptive_trailing_window every value stated",
            "onset.threshold.adaptive_trailing_window",
            &[("window_seconds", 0.75), ("k", 4.0), ("offset_ms", 50.0)],
            &[("dispersion", "population")],
        ),
    ];

    const BUMP_ONSET_CASES: &[(&str, &str, CaseParameters, CaseOptions)] = &[
        (
            "absolute_force bare",
            "onset.threshold.absolute_force",
            &[],
            &[],
        ),
        (
            "absolute_force direction below_only",
            "onset.threshold.absolute_force",
            &[],
            &[("direction", "below_only")],
        ),
        (
            "absolute_force direction two_sided",
            "onset.threshold.absolute_force",
            &[],
            &[("direction", "two_sided")],
        ),
        (
            "absolute_force both and last",
            "onset.threshold.absolute_force",
            &[],
            &[("direction", "two_sided"), ("selection", "last")],
        ),
        (
            "noise_relative bare",
            "onset.threshold.noise_relative",
            &[],
            &[],
        ),
        (
            "noise_relative direction below_only",
            "onset.threshold.noise_relative",
            &[],
            &[("direction", "below_only")],
        ),
        (
            "noise_relative direction two_sided",
            "onset.threshold.noise_relative",
            &[],
            &[("direction", "two_sided")],
        ),
        (
            "noise_relative persistence",
            "onset.threshold.noise_relative",
            &[("span_ms", 200.0)],
            &[("direction", "two_sided")],
        ),
        (
            "relative_to_system_weight bare",
            "onset.threshold.relative_to_system_weight",
            &[],
            &[],
        ),
        (
            "last_within_band bare",
            "onset.threshold.last_within_band",
            &[],
            &[],
        ),
        (
            "adaptive_trailing_window bare",
            "onset.threshold.adaptive_trailing_window",
            &[],
            &[],
        ),
    ];

    const TAKEOFF_CASES: &[(&str, &str, CaseParameters, CaseOptions)] = &[
        (
            "absolute_force bare",
            "takeoff.threshold.absolute_force",
            &[],
            &[],
        ),
        (
            "absolute_force threshold",
            "takeoff.threshold.absolute_force",
            &[("threshold_n", 25.0)],
            &[],
        ),
        (
            "absolute_force superseded spelling",
            "takeoff.threshold.absolute_force",
            &[("threshold_newtons", 30.0), ("minimum_flight", 0.03)],
            &[],
        ),
        (
            "absolute_force persistence",
            "takeoff.threshold.absolute_force",
            &[("persistence_ms", 50.0)],
            &[],
        ),
        (
            "absolute_force magnitude",
            "takeoff.threshold.absolute_force",
            &[],
            &[("comparison", "magnitude")],
        ),
        (
            "absolute_force signed",
            "takeoff.threshold.absolute_force",
            &[],
            &[("comparison", "signed")],
        ),
        (
            "absolute_force every value stated",
            "takeoff.threshold.absolute_force",
            &[("threshold_n", 30.0), ("persistence_ms", 200.0)],
            &[("comparison", "magnitude")],
        ),
        (
            "longest_run bare",
            "takeoff.threshold.longest_run",
            &[],
            &[],
        ),
        (
            "longest_run filter then rank",
            "takeoff.threshold.longest_run",
            &[],
            &[("short_run_handling", "filter_then_rank")],
        ),
        (
            "longest_run rank then filter",
            "takeoff.threshold.longest_run",
            &[],
            &[("short_run_handling", "rank_then_filter")],
        ),
        (
            "longest_run every value stated",
            "takeoff.threshold.longest_run",
            &[("threshold_n", 25.0), ("persistence_ms", 50.0)],
            &[
                ("comparison", "magnitude"),
                ("short_run_handling", "filter_then_rank"),
            ],
        ),
        (
            "descending_crossing bare",
            "takeoff.threshold.descending_crossing",
            &[],
            &[],
        ),
        (
            "descending_crossing confirmation",
            "takeoff.threshold.descending_crossing",
            &[("persistence_ms", 50.0)],
            &[],
        ),
        (
            "descending_crossing threshold",
            "takeoff.threshold.descending_crossing",
            &[("threshold_n", 25.0)],
            &[],
        ),
        (
            "flight_noise_k_sd bare",
            "takeoff.threshold.flight_noise_k_sd",
            &[],
            &[],
        ),
        (
            "flight_noise_k_sd trim",
            "takeoff.threshold.flight_noise_k_sd",
            &[("trim_fraction", 0.4)],
            &[],
        ),
        (
            "flight_noise_k_sd k",
            "takeoff.threshold.flight_noise_k_sd",
            &[("k", 3.0)],
            &[],
        ),
        (
            "flight_noise_k_sd population",
            "takeoff.threshold.flight_noise_k_sd",
            &[],
            &[("dispersion", "population")],
        ),
        (
            "flight_noise_k_sd every value stated",
            "takeoff.threshold.flight_noise_k_sd",
            &[
                ("trim_fraction", 0.1),
                ("k", 8.0),
                ("bounding_threshold_n", 20.0),
            ],
            &[("dispersion", "population")],
        ),
    ];

    const WEIGHING_CASES: &[WeighingCase] = &[
        (
            "fixed_window bare",
            "bwepoch.fixed_window",
            None,
            0.8,
            &[],
            &[],
        ),
        (
            "fixed_window half second",
            "bwepoch.fixed_window",
            None,
            0.5,
            &[],
            &[],
        ),
        (
            "fixed_window moved",
            "bwepoch.fixed_window",
            Some(240),
            0.8,
            &[],
            &[],
        ),
        (
            "fixed_window median",
            "bwepoch.fixed_window",
            None,
            0.8,
            &[],
            &[("centre", "median")],
        ),
        (
            "fixed_window mean",
            "bwepoch.fixed_window",
            None,
            0.8,
            &[],
            &[("centre", "mean")],
        ),
        (
            "fixed_window population",
            "bwepoch.fixed_window",
            None,
            0.8,
            &[],
            &[("dispersion", "population")],
        ),
        (
            "fixed_window sample",
            "bwepoch.fixed_window",
            None,
            0.8,
            &[],
            &[("dispersion", "sample")],
        ),
        (
            "fixed_window superseded spelling",
            "bwepoch.fixed_window",
            None,
            0.8,
            &[("duration_seconds", 0.2)],
            &[],
        ),
        (
            "fixed_window every value stated",
            "bwepoch.fixed_window",
            Some(120),
            0.6,
            &[],
            &[("centre", "median"), ("dispersion", "population")],
        ),
        (
            "manual_placement moved",
            "bwepoch.manual_placement",
            Some(240),
            0.5,
            &[],
            &[],
        ),
        (
            "manual_placement anchored",
            "bwepoch.manual_placement",
            None,
            0.5,
            &[],
            &[],
        ),
        (
            "adaptive_lowest_variance bare",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[],
            &[],
        ),
        (
            "adaptive_lowest_variance cumulative",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[],
            &[("accumulation", "cumulative_sum_of_squares")],
        ),
        (
            "adaptive_lowest_variance two pass",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[],
            &[("accumulation", "two_pass")],
        ),
        (
            "adaptive_lowest_variance population",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[],
            &[("dispersion", "population")],
        ),
        (
            "adaptive_lowest_variance moved",
            "bwepoch.adaptive_lowest_variance",
            Some(240),
            0.8,
            &[],
            &[("centre", "median")],
        ),
        (
            "adaptive_lowest_variance half second",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.5,
            &[],
            &[],
        ),
        (
            "adaptive_lowest_variance floor published",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[("variance_floor_pct_bodyweight", 0.5)],
            &[],
        ),
        (
            "adaptive_lowest_variance floor binding",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[("variance_floor_pct_bodyweight", 2.0)],
            &[],
        ),
    ];

    /// Both outcomes of the degenerate-band choice carry the policy under the registry's
    /// declared name. One rule used to answer the question with two vocabularies: a widened
    /// band recorded only the fraction, a refusal recorded only the policy.
    #[test]
    fn the_degenerate_band_policy_is_recorded_whichever_branch_ran() {
        let trial = synthetic();
        let recorded_policy = |request: &AnalysisRequest| {
            let response = run(&trial, request).expect("the synthetic trial analyses");
            let onset = response
                .bound_methods
                .iter()
                .find(|method| method.method_id == "onset.threshold.noise_relative")
                .expect("the onset rule is bound")
                .clone();
            onset
        };

        let bare = request(
            "onset.threshold.noise_relative",
            "takeoff.threshold.absolute_force",
        );
        let refused_band = recorded_policy(&bare);
        assert_eq!(
            refused_band
                .bound_parameters
                .iter()
                .find(|(name, _)| name == "degenerate_band")
                .map(|(_, value)| value.as_str()),
            Some("refuse"),
            "a run that would refuse names the policy"
        );

        let mut widened = bare.clone();
        widened
            .onset
            .parameters
            .insert("degenerate_fraction".into(), 0.2);
        let widened_band = recorded_policy(&widened);
        assert_eq!(
            widened_band
                .bound_parameters
                .iter()
                .find(|(name, _)| name == "degenerate_band")
                .map(|(_, value)| value.as_str()),
            Some("fraction_of_reference"),
            "a run that widened the band names the policy"
        );
        assert_eq!(
            widened_band.parameter_sources.get("degenerate_fraction"),
            Some(&plateforce_core::provenance::ParameterSource::Stated),
            "the stated fraction keeps the caller's signature"
        );
    }

    struct CharacterisationCase {
        name: String,
        trial: &'static str,
        request: AnalysisRequest,
    }

    fn case_parameters(pairs: CaseParameters) -> BTreeMap<String, f64> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), *value))
            .collect()
    }

    fn case_options(pairs: CaseOptions) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    fn characterisation_cases() -> Vec<CharacterisationCase> {
        let mut cases = Vec::new();
        let baseline = || {
            request(
                "onset.threshold.noise_relative",
                "takeoff.threshold.absolute_force",
            )
        };

        for (name, method_id, parameters, options) in ONSET_CASES {
            let mut candidate = baseline();
            candidate.onset = MethodChoice {
                method_id: (*method_id).into(),
                parameters: case_parameters(parameters),
                options: case_options(options),
                manual_index: None,
                ..Default::default()
            };
            cases.push(CharacterisationCase {
                name: format!("onset {name}"),
                trial: "synthetic",
                request: candidate,
            });
        }

        for (name, method_id, parameters, options) in TAKEOFF_CASES {
            let mut candidate = baseline();
            candidate.takeoff = MethodChoice {
                method_id: (*method_id).into(),
                parameters: case_parameters(parameters),
                options: case_options(options),
                manual_index: None,
                ..Default::default()
            };
            cases.push(CharacterisationCase {
                name: format!("takeoff {name}"),
                trial: "synthetic",
                request: candidate,
            });
        }

        for (name, method_id, start_index, duration_seconds, parameters, options) in WEIGHING_CASES
        {
            let mut candidate = baseline();
            let mut values = case_parameters(parameters);
            values.insert(
                window_length_parameter(method_id).to_string(),
                *duration_seconds,
            );
            candidate.weighing = WeighingChoice {
                method_id: (*method_id).into(),
                start_index: *start_index,
                parameters: values,
                options: case_options(options),
                ..Default::default()
            };
            cases.push(CharacterisationCase {
                name: format!("weighing {name}"),
                trial: "synthetic",
                request: candidate,
            });
        }

        let mut low_gravity = baseline();
        low_gravity.gravity_meters_per_second_squared = 9.8;
        cases.push(CharacterisationCase {
            name: "gravity 9.8".into(),
            trial: "synthetic",
            request: low_gravity,
        });

        let mut high_gravity = baseline();
        high_gravity.gravity_meters_per_second_squared = 9.81;
        cases.push(CharacterisationCase {
            name: "gravity 9.81".into(),
            trial: "synthetic",
            request: high_gravity,
        });

        let mut stated_touchdown = baseline();
        stated_touchdown.touchdown_index = Some(2300);
        cases.push(CharacterisationCase {
            name: "touchdown stated".into(),
            trial: "synthetic",
            request: stated_touchdown,
        });

        let mut dragged_onset = baseline();
        dragged_onset.onset.manual_index = Some(1100);
        dragged_onset.onset.parameters = case_parameters(&[("k", 3.0)]);
        cases.push(CharacterisationCase {
            name: "onset dragged".into(),
            trial: "synthetic",
            request: dragged_onset,
        });

        let mut dragged_takeoff = baseline();
        dragged_takeoff.takeoff.manual_index = Some(2100);
        dragged_takeoff.takeoff.parameters = case_parameters(&[("threshold_n", 25.0)]);
        cases.push(CharacterisationCase {
            name: "takeoff dragged".into(),
            trial: "synthetic",
            request: dragged_takeoff,
        });

        let mut both_dragged = baseline();
        both_dragged.onset.manual_index = Some(1150);
        both_dragged.takeoff.manual_index = Some(2050);
        cases.push(CharacterisationCase {
            name: "both dragged".into(),
            trial: "synthetic",
            request: both_dragged,
        });

        let mut inverted = baseline();
        inverted.onset.manual_index = Some(2200);
        inverted.takeoff.manual_index = Some(1300);
        cases.push(CharacterisationCase {
            name: "onset after takeoff".into(),
            trial: "synthetic",
            request: inverted,
        });

        let mut unbacked = baseline();
        unbacked.registry_backed_ids = Vec::new();
        cases.push(CharacterisationCase {
            name: "nothing registry backed".into(),
            trial: "synthetic",
            request: unbacked,
        });

        let mut everything_backed = baseline();
        everything_backed.registry_backed_ids = BINDINGS
            .iter()
            .map(|binding| binding.id.to_string())
            .collect();
        cases.push(CharacterisationCase {
            name: "everything registry backed".into(),
            trial: "synthetic",
            request: everything_backed,
        });

        for (name, method_id, parameters, options) in BUMP_ONSET_CASES {
            let mut candidate = baseline();
            candidate.onset = MethodChoice {
                method_id: (*method_id).into(),
                parameters: case_parameters(parameters),
                options: case_options(options),
                manual_index: None,
                ..Default::default()
            };
            cases.push(CharacterisationCase {
                name: format!("bump onset {name}"),
                trial: "pre_movement_bump",
                request: candidate,
            });
        }

        for (name, takeoff_id) in [
            ("first sustained run", "takeoff.threshold.absolute_force"),
            ("longest run", "takeoff.threshold.longest_run"),
            (
                "descending crossing",
                "takeoff.threshold.descending_crossing",
            ),
            ("flight noise", "takeoff.threshold.flight_noise_k_sd"),
        ] {
            let mut candidate = baseline();
            candidate.weighing = weighing(&candidate.weighing.method_id.clone(), None, 0.5);
            candidate.takeoff.method_id = takeoff_id.into();
            cases.push(CharacterisationCase {
                name: format!("two flight phases {name}"),
                trial: "two_flight_phases",
                request: candidate,
            });
        }

        cases
    }

    fn characterisation_report() -> String {
        let synthetic_trial = synthetic();
        let two_phase_trial = two_flight_phases();
        let bump_trial = pre_movement_bump();
        let mut report = String::new();

        for case in characterisation_cases() {
            let trial = match case.trial {
                "two_flight_phases" => &two_phase_trial,
                "pre_movement_bump" => &bump_trial,
                _ => &synthetic_trial,
            };
            report.push_str(&format!("case {} on {}\n", case.name, case.trial));
            match run(trial, &case.request) {
                Ok(response) => {
                    report.push_str(&format!(
                        "  window {}..{}\n  onset {:?}\n  takeoff {:?}\n  touchdown {:?}\n",
                        response.weighing_start_index,
                        response.weighing_end_index,
                        response.onset_index,
                        response.takeoff_index,
                        response.touchdown_index
                    ));
                    report.push_str(&format!(
                        "  levels {:?} {:?} {:?} {:?} {:?}\n",
                        response.levels.system_weight_newtons,
                        response.levels.weighing_standard_deviation_newtons,
                        response.levels.onset_band_lower_newtons,
                        response.levels.onset_band_upper_newtons,
                        response.levels.takeoff_threshold_newtons
                    ));
                    for metric in &response.metrics {
                        report.push_str(&format!("  metric {} {:?}\n", metric.key, metric.value));
                    }
                    for method in &response.bound_methods {
                        report.push_str(&format!(
                            "  method {} backed {} override {}\n",
                            method.method_id, method.registry_backed, method.manual_override
                        ));
                    }
                    for warning in &response.warnings {
                        report.push_str(&format!("  warning {warning}\n"));
                    }
                }
                Err(error) => report.push_str(&format!("  error {error}\n")),
            }
        }
        report
    }

    /// Records the report over the baseline when asked, and does nothing otherwise. The
    /// file below is a record of what this build does, not a statement of what it should
    /// do, so every number that moved is accounted for before this is run.
    #[test]
    fn the_baseline_is_rewritten_only_when_asked() {
        if std::env::var("PLATEFORCE_REGENERATE").is_ok() {
            std::fs::write(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/resolved-values-baseline.txt"
                ),
                characterisation_report(),
            )
            .unwrap();
        }
    }

    /// The file this compares against was recorded before the fingerprint was built from
    /// the resolution, so a number that moved shows up as a changed line.
    #[test]
    fn recording_what_each_rule_resolved_moved_no_number() {
        let expected = include_str!("../tests/resolved-values-baseline.txt");
        let found = characterisation_report();
        for (offset, (want, got)) in expected.lines().zip(found.lines()).enumerate() {
            assert_eq!(want, got, "baseline line {} moved", offset + 1);
        }
        assert_eq!(
            expected.lines().count(),
            found.lines().count(),
            "the case list is no longer the one the baseline was recorded from"
        );
    }
}
