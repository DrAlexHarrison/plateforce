//! The rate, impulse and power-rate family, on the committed subject-01 trials.
//!
//! Rate of force development is among the most reported quantities in the field and among the
//! most method-sensitive, which is this product's whole case. What is read here is the size of
//! the disagreement between the rules on one recording, the two decisions the family forces a
//! caller to make before it will produce a number at all, and the one identity that ties two
//! of the rules together exactly.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice};

use crate::common::{committed_trial, default_request, COMMITTED_TRIALS};

/// The construct and key every rate rule answers under.
const RATE_CONSTRUCT: &str = "rate_of_force_development";
const RATE_KEY: &str = "rate_of_force_development_newtons_per_second";
const IMPULSE_CONSTRUCT: &str = "epoch_impulse";
const IMPULSE_KEY: &str = "epoch_impulse_newton_seconds";
const POWER_RATE_CONSTRUCT: &str = "rate_of_power_development";
const POWER_RATE_KEY: &str = "rate_of_power_development_watts_per_second";

/// The window every rule in this family reads, and the phase boundaries two of them read.
const WINDOW_CONSTRUCT: &str = "analysis_window";
const WINDOW_RULE: &str = "window_end.takeoff.detected";
const PROPULSION_START_CONSTRUCT: &str = "propulsion_phase_start";
const PROPULSION_START_RULE: &str = "phase.propulsion_start.zero_velocity";
const PROPULSION_END_CONSTRUCT: &str = "propulsion_phase_end";
const PROPULSION_END_RULE: &str = "phase.propulsion_end.peak_com_velocity";

/// The six rate rules this build runs, and the values each needs before it will answer.
///
/// The two force levels are stated here and nowhere in the registry, which publishes no pair
/// and says why: no published pair was located. They are the values this reading was taken
/// under, not a recommendation.
fn rate_rules() -> Vec<(
    &'static str,
    BTreeMap<String, f64>,
    BTreeMap<String, String>,
)> {
    vec![
        (
            "rfd.epoch_from_onset.overlapping",
            BTreeMap::from([("epoch_ms".to_string(), 200.0)]),
            BTreeMap::new(),
        ),
        (
            "rfd.peak_sliding_window",
            BTreeMap::from([("window_width_ms".to_string(), 20.0)]),
            BTreeMap::new(),
        ),
        (
            "rfd.at_fraction_of_peak_force",
            BTreeMap::from([("fraction_pct".to_string(), 50.0)]),
            BTreeMap::new(),
        ),
        (
            "rfd.average_to_peak_force",
            BTreeMap::new(),
            BTreeMap::new(),
        ),
        (
            "rfd.between_force_levels",
            BTreeMap::from([
                ("lower_level".to_string(), 700.0),
                ("upper_level".to_string(), 900.0),
            ]),
            BTreeMap::from([("reference_basis".to_string(), "absolute".to_string())]),
        ),
        (
            "rfd.phase_endpoint_secant.harry",
            BTreeMap::new(),
            BTreeMap::new(),
        ),
        (
            "rfd.mean_force_over_duration.lapuente",
            BTreeMap::new(),
            BTreeMap::new(),
        ),
    ]
}

/// A request carrying the analysis window, the propulsion boundaries, and one rule of this
/// family with the values it was asked for.
fn asking(
    construct: &str,
    method_id: &str,
    parameters: BTreeMap<String, f64>,
    options: BTreeMap<String, String>,
) -> AnalysisRequest {
    let mut request = default_request();
    request.derived.insert(
        WINDOW_CONSTRUCT.to_string(),
        MethodChoice {
            method_id: WINDOW_RULE.to_string(),
            ..Default::default()
        },
    );
    request.derived.insert(
        PROPULSION_START_CONSTRUCT.to_string(),
        MethodChoice {
            method_id: PROPULSION_START_RULE.to_string(),
            ..Default::default()
        },
    );
    request.derived.insert(
        PROPULSION_END_CONSTRUCT.to_string(),
        MethodChoice {
            method_id: PROPULSION_END_RULE.to_string(),
            options: BTreeMap::from([("search_signal".to_string(), "velocity_argmax".to_string())]),
            ..Default::default()
        },
    );
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            parameters,
            options,
            ..Default::default()
        },
    );
    request
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response.metric(key).and_then(|metric| metric.value)
}

/// The id the result says produced a number, which is the whole record for a construct whose
/// rules all report one key.
fn computed_by(response: &AnalysisResponse, key: &str) -> Option<String> {
    response
        .metric(key)
        .and_then(|metric| metric.computed_by.clone())
}

/// The entries besides its own whose values a rule of this family reads.
///
/// Two of the family's choices are published on an entry other than the rule that runs under
/// them: whether system weight comes out before integrating, and what power is. A rule reads
/// them, records them, and names the owning entry among the entries its number rests on, on
/// the model of the four integration entries every jump-height rule inherits. So the names a
/// rule may legitimately read are its own entry's plus those, and the pair is written out
/// here rather than being inferred from whatever the rule happened to consult.
fn entries_read_through(method_id: &str) -> &'static [&'static str] {
    match method_id {
        "impulse.epoch_from_onset" | "impulse.to_fraction_of_peak_force" => &["impulse.convention"],
        "rpd.phase_anchored" | "rpd.peak_to_peak_anchored.amti" => {
            &["power.instantaneous.force_x_velocity"]
        }
        _ => &[],
    }
}

/// Every value a caller states reaches the rule it was stated to, and every id the record
/// carries resolves in the shipped registry.
///
/// Two directions, and the second is the one that bites. A name the request carried and the
/// rule never consulted lands in `unread_parameters`, so the caller's number moved nothing
/// while the record showed a reader who had chosen. And a name the rule was given has to come
/// back in what it read, or the value reached no arithmetic.
///
/// An id that resolves nowhere leaves a reader unable to look up what produced the number,
/// which is the same as not naming it.
#[test]
fn every_rule_in_this_family_reads_what_it_is_given_and_records_a_name_a_reader_can_look_up() {
    let registry = crate::common::registry();
    let trial = committed_trial("subject01_trial1");

    let mut asked: Vec<(&str, &str, BTreeMap<String, f64>, BTreeMap<String, String>)> =
        rate_rules()
            .into_iter()
            .map(|(id, parameters, options)| (RATE_CONSTRUCT, id, parameters, options))
            .collect();
    for method_id in [
        "impulse.epoch_from_onset",
        "impulse.to_fraction_of_peak_force",
    ] {
        asked.push((
            IMPULSE_CONSTRUCT,
            method_id,
            BTreeMap::from([
                ("epoch_ms".to_string(), 200.0),
                ("fraction_pct".to_string(), 50.0),
            ]),
            BTreeMap::from([("convention".to_string(), "net".to_string())]),
        ));
    }
    for method_id in ["rpd.phase_anchored", "rpd.peak_to_peak_anchored.amti"] {
        asked.push((
            POWER_RATE_CONSTRUCT,
            method_id,
            BTreeMap::new(),
            BTreeMap::from([
                ("force_term".to_string(), "total".to_string()),
                ("sign_convention".to_string(), "upward_positive".to_string()),
            ]),
        ));
    }

    let mut unread = Vec::new();
    let mut never_arrived = Vec::new();
    let mut unresolved = Vec::new();
    let mut checked = 0usize;
    let mut names_checked = 0usize;
    for (construct, method_id, parameters, options) in &asked {
        // The names this rule may read: its own entry's, and those of the entries it reads
        // through. An epoch stated at a rule that reads a fraction is dropped rather than
        // counted against it, which is a fact about this list rather than about the rule.
        let mut published: Vec<String> = Vec::new();
        for id in std::iter::once(method_id.to_string()).chain(
            entries_read_through(method_id)
                .iter()
                .map(|id| id.to_string()),
        ) {
            let entry = registry
                .methods
                .get(id.as_str())
                .unwrap_or_else(|| panic!("{id} is not in the shipped registry"));
            published.extend(
                entry
                    .parameters
                    .iter()
                    .map(|parameter| parameter.name.clone()),
            );
        }

        let mut request = asking(construct, method_id, parameters.clone(), options.clone());
        let choice = request
            .derived
            .get_mut(*construct)
            .expect("the rule is named");
        choice.parameters.retain(|name, _| published.contains(name));
        choice.options.retain(|name, _| published.contains(name));
        let stated: Vec<String> = choice
            .parameters
            .keys()
            .chain(choice.options.keys())
            .cloned()
            .collect();

        let response = run(&trial, &request).unwrap_or_else(|error| panic!("{method_id}: {error}"));
        let bound = response
            .bound_methods
            .iter()
            .find(|method| method.method_id == *method_id)
            .unwrap_or_else(|| panic!("{method_id} left no record of what it read"));
        if !bound.unread_parameters.is_empty() {
            unread.push(format!("{method_id}: {:?}", bound.unread_parameters));
        }
        for name in &stated {
            names_checked += 1;
            if !bound
                .bound_parameters
                .iter()
                .any(|(recorded, _)| recorded == name)
            {
                never_arrived.push(format!(
                    "{method_id} was given {name} and recorded no value"
                ));
            }
        }
        for method in &response.bound_methods {
            if !registry.methods.contains_key(method.method_id.as_str()) {
                unresolved.push(format!("{method_id} recorded {}", method.method_id));
            }
        }
        checked += 1;
    }

    assert!(
        unread.is_empty(),
        "{} of {checked} rules carried a value they never read:\n  {}",
        unread.len(),
        unread.join("\n  ")
    );
    assert!(
        never_arrived.is_empty(),
        "{} of {names_checked} stated values reached no rule:\n  {}",
        never_arrived.len(),
        never_arrived.join("\n  ")
    );
    assert!(
        unresolved.is_empty(),
        "{} records name an id that resolves in no registry entry:\n  {}",
        unresolved.len(),
        unresolved.join("\n  ")
    );
    assert_eq!(
        checked,
        asked.len(),
        "a rule was skipped without being named"
    );
    println!(
        "{checked} rules read all {names_checked} values stated to them, and every id their \
         records carry resolves in the registry's {} entries",
        registry.methods.len()
    );
    assert!(
        names_checked >= 12,
        "only {names_checked} values were stated, so this read almost nothing"
    );
}

/// Every rate rule answers on subject 01 trial 1, each under its own id, and the spread
/// between them is what the choice of rule costs.
///
/// Not an assertion that the numbers are right. It is the reading this product exists to
/// make visible: seven published rules, one recording, and a ratio between the largest and
/// the smallest that no training intervention would produce.
#[test]
fn every_rate_rule_answers_on_one_trial_and_the_spread_between_them_is_the_method() {
    let trial = committed_trial("subject01_trial1");
    let mut reported: Vec<(&str, f64)> = Vec::new();
    let mut declined: Vec<String> = Vec::new();

    for (method_id, parameters, options) in rate_rules() {
        let request = asking(RATE_CONSTRUCT, method_id, parameters, options);
        let response = run(&trial, &request).unwrap_or_else(|error| panic!("{method_id}: {error}"));
        match value(&response, RATE_KEY) {
            Some(rate) => {
                assert_eq!(
                    computed_by(&response, RATE_KEY).as_deref(),
                    Some(method_id),
                    "{method_id} reported a rate under another rule's name"
                );
                println!("{method_id}: {rate:.0} N/s");
                reported.push((method_id, rate));
            }
            None => {
                let refusal = response
                    .refusals
                    .iter()
                    .find(|declined| declined.method_id == method_id)
                    .unwrap_or_else(|| panic!("{method_id} produced no rate and no refusal"));
                declined.push(format!("{method_id}: {}", refusal.refusal));
            }
        }
    }

    assert!(
        declined.is_empty(),
        "{} of {} rate rules produced nothing on subject 01 trial 1:\n  {}",
        declined.len(),
        rate_rules().len(),
        declined.join("\n  ")
    );

    let lowest = reported
        .iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .expect("a rule answered");
    let highest = reported
        .iter()
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .expect("a rule answered");
    println!(
        "{} of {} rules answered on subject 01 trial 1, from {} at {:.0} N/s to {} at {:.0} N/s, a ratio of {:.1}",
        reported.len(),
        rate_rules().len(),
        lowest.0,
        lowest.1,
        highest.0,
        highest.1,
        highest.1 / lowest.1,
    );
    assert_eq!(
        reported.len(),
        rate_rules().len(),
        "a rule answered without being counted"
    );

    // Every rule reports one construct on one trace, so two of them landing on the same
    // number means two entries reached one arithmetic and the registry files a distinction
    // this build does not make. Pairwise rather than on the spread, because a spread stays
    // wide while two rules in the middle of it collapse onto each other.
    let mut collapsed = Vec::new();
    for (index, (left_id, left)) in reported.iter().enumerate() {
        for (right_id, right) in reported.iter().skip(index + 1) {
            if (left - right).abs() < 1.0 {
                collapsed.push(format!(
                    "{left_id} and {right_id} both report {left:.4} N/s"
                ));
            }
        }
    }
    assert!(
        collapsed.is_empty(),
        "{} of {} pairs of rules report one number, so the registry files a distinction this \
         build does not make:\n  {}",
        collapsed.len(),
        reported.len() * (reported.len() - 1) / 2,
        collapsed.join("\n  ")
    );
    println!(
        "{} pairs of rules compared, none within 1 N/s of another",
        reported.len() * (reported.len() - 1) / 2
    );
}

/// The steepest chord of a width is at least as steep as the chord of that width from onset,
/// on every committed trial.
///
/// An identity rather than a tolerance: the steepest-chord rule maximises over position and
/// the epoch rule takes one position, so one is the maximum of a set the other is a member
/// of. A build where the epoch rule read a different series, a different width or a different
/// window would break it.
#[test]
fn the_steepest_chord_is_never_shallower_than_the_chord_from_onset_at_the_same_width() {
    const WIDTH_MILLISECONDS: f64 = 20.0;
    let mut checked = 0usize;

    for name in COMMITTED_TRIALS {
        let trial = committed_trial(name);
        let from_onset = run(
            &trial,
            &asking(
                RATE_CONSTRUCT,
                "rfd.epoch_from_onset.overlapping",
                BTreeMap::from([("epoch_ms".to_string(), WIDTH_MILLISECONDS)]),
                BTreeMap::new(),
            ),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        let anywhere = run(
            &trial,
            &asking(
                RATE_CONSTRUCT,
                "rfd.peak_sliding_window",
                BTreeMap::from([("window_width_ms".to_string(), WIDTH_MILLISECONDS)]),
                BTreeMap::new(),
            ),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));

        let (Some(at_onset), Some(steepest)) =
            (value(&from_onset, RATE_KEY), value(&anywhere, RATE_KEY))
        else {
            panic!("{name} did not produce both rates");
        };
        println!("{name}: from onset {at_onset:.0} N/s, steepest anywhere {steepest:.0} N/s");
        assert!(
            steepest >= at_onset,
            "{name}: the steepest {WIDTH_MILLISECONDS} ms chord reads {steepest:.1} N/s and the \
             chord of the same width from onset reads {at_onset:.1} N/s, so one of them is not \
             reading the width, the series or the window the other is"
        );
        // The second identity, and the reason the registry warns that a 2 ms window on a
        // noisy trace measures the filter: a chord over a doubled width is the mean of the
        // two halves it spans, so it can never exceed the steepest chord of the half width.
        // A build that stopped reading the width would report one number for both and pass
        // the line above while failing this one.
        let doubled = run(
            &trial,
            &asking(
                RATE_CONSTRUCT,
                "rfd.peak_sliding_window",
                BTreeMap::from([("window_width_ms".to_string(), WIDTH_MILLISECONDS * 2.0)]),
                BTreeMap::new(),
            ),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        let wider = value(&doubled, RATE_KEY).expect("the doubled width produced a rate");
        println!("{name}: at twice the width {wider:.0} N/s");
        assert!(
            steepest >= wider,
            "{name}: the steepest chord reads {steepest:.1} N/s at {WIDTH_MILLISECONDS} ms and \
             {wider:.1} N/s at twice that, so the width is not reaching the search"
        );
        assert!(
            steepest > wider,
            "{name}: the steepest chord reads {steepest:.1} N/s at both {WIDTH_MILLISECONDS} ms \
             and twice that, so the stated width moved nothing"
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        COMMITTED_TRIALS.len(),
        "the identity was checked on fewer trials than are committed"
    );
    println!(
        "{checked} of {} committed trials checked",
        COMMITTED_TRIALS.len()
    );
}

/// Neither impulse rule produces a number until somebody states whether system weight comes
/// out, and the gap between the two answers is exactly the weight the epoch spans.
///
/// The algebra is the check: over a stated epoch the two integrals differ by system weight
/// times the epoch and by nothing else, so a build that subtracted the weight outside the
/// integral, or over the wrong number of intervals, misses by a whole sample of weight.
#[test]
fn an_epoch_impulse_states_its_convention_and_the_two_differ_by_the_weight_it_spans() {
    const EPOCH_MILLISECONDS: f64 = 200.0;
    let trial = committed_trial("subject01_trial1");

    let unstated = run(
        &trial,
        &asking(
            IMPULSE_CONSTRUCT,
            "impulse.epoch_from_onset",
            BTreeMap::from([("epoch_ms".to_string(), EPOCH_MILLISECONDS)]),
            BTreeMap::new(),
        ),
    )
    .expect("the analysis ran");
    assert_eq!(
        value(&unstated, IMPULSE_KEY),
        None,
        "an impulse was produced under a convention nobody stated"
    );
    let refusal = unstated
        .refusals
        .iter()
        .find(|declined| declined.method_id == "impulse.epoch_from_onset")
        .expect("the rule declined by name");
    let sentence = refusal.refusal.to_string();
    println!("unstated: {sentence}");
    assert!(
        sentence.contains("convention"),
        "the refusal does not name the parameter a reader would state: {sentence}"
    );

    let under = |convention: &str| {
        run(
            &trial,
            &asking(
                IMPULSE_CONSTRUCT,
                "impulse.epoch_from_onset",
                BTreeMap::from([("epoch_ms".to_string(), EPOCH_MILLISECONDS)]),
                BTreeMap::from([("convention".to_string(), convention.to_string())]),
            ),
        )
        .expect("the analysis ran")
    };
    let net = under("net");
    let gross = under("gross");
    let (Some(net_impulse), Some(gross_impulse)) =
        (value(&net, IMPULSE_KEY), value(&gross, IMPULSE_KEY))
    else {
        panic!("a stated convention did not produce an impulse");
    };
    let weight = net
        .levels
        .system_weight_newtons
        .expect("the weighing rule answered");
    let spanned = weight * EPOCH_MILLISECONDS / 1000.0;
    println!(
        "net {net_impulse:.4} N.s, gross {gross_impulse:.4} N.s, difference {:.4} N.s against \
         {weight:.2} N over {EPOCH_MILLISECONDS} ms, which is {spanned:.4} N.s",
        gross_impulse - net_impulse
    );
    assert!(
        (gross_impulse - net_impulse - spanned).abs() < 1e-6,
        "the two conventions differ by {:.6} N.s where the weight over the epoch is {spanned:.6} N.s",
        gross_impulse - net_impulse
    );
}

/// A power rate needs both of the choices that decide what power is, and the two force terms
/// give different numbers on the same recording.
#[test]
fn a_power_rate_states_what_power_is_before_it_reports_one() {
    let trial = committed_trial("subject01_trial1");

    let unstated = run(
        &trial,
        &asking(
            POWER_RATE_CONSTRUCT,
            "rpd.peak_to_peak_anchored.amti",
            BTreeMap::new(),
            BTreeMap::new(),
        ),
    )
    .expect("the analysis ran");
    assert_eq!(
        value(&unstated, POWER_RATE_KEY),
        None,
        "a power rate was produced under a force term nobody stated"
    );
    let sentence = unstated
        .refusals
        .iter()
        .find(|declined| declined.method_id == "rpd.peak_to_peak_anchored.amti")
        .expect("the rule declined by name")
        .refusal
        .to_string();
    println!("unstated: {sentence}");
    assert!(
        sentence.contains("force_term"),
        "the refusal does not name a parameter a reader would state: {sentence}"
    );

    let under = |method_id: &str, force_term: &str| {
        run(
            &trial,
            &asking(
                POWER_RATE_CONSTRUCT,
                method_id,
                BTreeMap::new(),
                BTreeMap::from([
                    ("force_term".to_string(), force_term.to_string()),
                    ("sign_convention".to_string(), "upward_positive".to_string()),
                ]),
            ),
        )
        .expect("the analysis ran")
    };

    // Under one force term a rule can answer where under the other it declines, and that is
    // an answer about the pair rather than a gap: the two terms differ by system weight times
    // velocity at every instant, which is 2000 W at 2.5 m/s and 800 N.
    let mut answered_under_both = 0usize;
    let mut answered_under_one = Vec::new();
    for method_id in ["rpd.phase_anchored", "rpd.peak_to_peak_anchored.amti"] {
        let mut rates = Vec::new();
        for force_term in ["total", "net"] {
            let response = under(method_id, force_term);
            match value(&response, POWER_RATE_KEY) {
                Some(rate) => {
                    assert_eq!(
                        computed_by(&response, POWER_RATE_KEY).as_deref(),
                        Some(method_id),
                        "{method_id} reported under another rule's name"
                    );
                    println!("{method_id}, {force_term} force: {rate:.0} W/s");
                    rates.push(rate);
                }
                None => {
                    let sentence = response
                        .refusals
                        .iter()
                        .find(|declined| declined.method_id == method_id)
                        .map(|declined| declined.refusal.to_string())
                        .unwrap_or_else(|| {
                            panic!("{method_id} produced neither a rate nor a refusal")
                        });
                    println!("{method_id}, {force_term} force: {sentence}");
                    answered_under_one.push(format!("{method_id} under {force_term}: {sentence}"));
                }
            }
        }
        assert!(
            !rates.is_empty(),
            "{method_id} produced no rate under either force term"
        );
        if rates.len() == 2 {
            assert_ne!(
                rates[0], rates[1],
                "{method_id} reports one number under both force terms, so the choice reached no \
                 arithmetic"
            );
            answered_under_both += 1;
        }
    }
    println!(
        "2 power rules, {answered_under_both} answering under both force terms, {} declining \
         under one:\n  {}",
        answered_under_one.len(),
        answered_under_one.join("\n  ")
    );
    // At least one rule has to answer both ways, or the assertion that the choice reaches the
    // arithmetic was never made against anything.
    assert!(
        answered_under_both >= 1,
        "no power rule answered under both force terms, so nothing here compared them"
    );
}
