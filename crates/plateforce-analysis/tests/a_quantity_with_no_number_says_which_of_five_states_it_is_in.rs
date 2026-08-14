//! A quantity that came back without a number is in one of five states, and a reader can tell
//! which from fields that already ship.
//!
//! Five, not two. A number is a number. A rule on the number's chain declined, and the refusal
//! is why. The arithmetic ran and produced something that is not a number, which
//! `carried_no_number` says. A condition qualified the trial and suppressed the quantity, which
//! a signal's `qualifies` names. Or a rule ran, returned nothing and declined nothing, which is
//! silent and is the state this file exists to make visible.
//!
//! Two of the five have no producer and their account is empty on purpose. **The count of empty
//! accounts is asserted equal to the population of those two states, never asserted to be
//! zero**: an implementation that wrote nothing for any absence would pass a guard shaped the
//! second way, and that is the guard this whole change replaces.
//!
//! Every account here is compared against its producer's own string in full rather than by
//! `contains`. A sentence with anything added to it is a sentence this crate composed, and one
//! quantity accounted for by two producers is the defect the product exists against.
//!
//! Each state carries a fixture measured on this tree rather than argued for:
//!
//! | state | recording | count |
//! |---|---|---|
//! | a number | `subject01_trial1` | 11 of 11 |
//! | a rule on the chain declined | `subject01_trial1_interrupted` | 6 of 11 |
//! | the arithmetic produced no number | `subject01_trial1_interrupted` | 2 of 11 |
//! | a condition suppressed it | `synthetic_untrimmed_step_off_after_jump` | 5 of 11 |
//! | a rule returned nothing, silently | `synthetic_untrimmed_step_off` | 1 of 12 |

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::{
    accounts_of, chain_names, metrics_resting_on, recorded_number_text, run, AnalysisRequest,
    AnalysisResponse, MethodChoice, Metric, WeighingChoice,
};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::{read_trial_from_path, Trial};

mod common;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures/"
);

/// The recording that stops mid-jump, which is where three of the five states are reachable at
/// once. Held apart from `fixtures/` because a damaged trace is not a trial anybody would
/// analyse on purpose.
const INTERRUPTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/damaged/subject01_trial1_interrupted.force.txt"
);

const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

fn trial_at(path: &str) -> Trial {
    let (trial, _) = read_trial_from_path(path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

fn fixture(name: &str) -> Trial {
    trial_at(&format!("{FIXTURES}{name}.force.txt"))
}

/// The rules the shipped spine binds, which is what the committed parity requests ask.
fn spine_request() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        ..Default::default()
    })
}

/// The trio that reads the step-off as the start of the jump, so the two landmarks come back in
/// the order the recording did not happen in.
fn inverting_request() -> AnalysisRequest {
    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.adaptive_lowest_variance".into(),
            parameters: BTreeMap::from([("window_seconds".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.adaptive_trailing_window".into(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.flight_noise_k_sd".into(),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn binding(construct: &str, method_id: &str) -> AnalysisRequest {
    let mut request = spine_request();
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            ..Default::default()
        },
    );
    common::prepared(request)
}

fn analysed(trial: &Trial, request: &AnalysisRequest) -> AnalysisResponse {
    run(trial, request).unwrap_or_else(|refusal| panic!("the request ran: {refusal}"))
}

/// The stamp every surface carries. Its contents do not reach an account of an absence, and a
/// guard here that varied it would be varying a knob the property does not read.
fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

fn accounts(response: &AnalysisResponse) -> BTreeMap<String, String> {
    accounts_of(response, &stamp(), false)
}

/// The account written for one quantity, refused rather than defaulted where the block holds no
/// key for it.
///
/// A `unwrap_or_default` here would read a missing key as an empty account, which is one of the
/// five states, so the state this whole file is about would be indistinguishable from the row
/// that is not there.
fn account_of<'a>(block: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    block
        .get(key)
        .unwrap_or_else(|| panic!("the block holds no entry for {key}: {:?}", block.keys()))
}

fn valueless(response: &AnalysisResponse) -> Vec<&Metric> {
    response
        .metrics
        .iter()
        .filter(|metric| metric.value.is_none())
        .collect()
}

/// Whether an account opens with a number, which is what `describe` writes and what an account
/// of an absence must never do.
///
/// Read off the first whitespace-delimited token rather than by comparing against a rebuilt
/// sentence: an account beginning "0 meters" is the shape a substituted value produces, and the
/// number is what makes it a claim about a measurement.
fn opens_with_a_value(account: &str) -> bool {
    account
        .split_whitespace()
        .next()
        .is_some_and(|token| token.parse::<f64>().is_ok())
}

/// Which refusals on this response account for a quantity, as this test reaches them: through
/// the wire shape rather than through the call `descriptions_of` makes.
///
/// `DeclinedRule` serialises as the refusal a browser, a notebook and an R session are handed,
/// so the string compared below is read out of the record every surface but this one meets.
fn wire_sentences(response: &AnalysisResponse) -> Vec<(String, String)> {
    let wire = serde_json::to_value(&response.refusals).expect("the refusals serialise");
    wire.as_array()
        .expect("the refusals are a list")
        .iter()
        .map(|refusal| {
            (
                refusal["method_id"]
                    .as_str()
                    .expect("a refusal names its rule")
                    .to_string(),
                refusal["message"]
                    .as_str()
                    .expect("a refusal carries its sentence")
                    .to_string(),
            )
        })
        .collect()
}

/// Every number gives the account of its own value, and the account opens with that value.
///
/// The control for `no_account_of_a_quantity_without_a_number_opens_with_a_value`. A predicate
/// that could not see a number would report every account as clean, and clean is what that
/// guard is looking for, so it is proven here against the trial where all eleven answer.
#[test]
fn a_number_gives_the_account_of_its_own_value() {
    let response = analysed(&fixture("subject01_trial1"), &spine_request());
    let block = accounts(&response);
    assert_eq!(block.len(), response.metrics.len());

    let mut opening = 0usize;
    for metric in &response.metrics {
        let value = metric
            .value
            .unwrap_or_else(|| panic!("{} answers on this trial", metric.key));
        let account = account_of(&block, &metric.key);
        assert!(
            account.starts_with(&recorded_number_text(value)),
            "{} opens with something other than its own value: {account}",
            metric.key
        );
        assert!(opens_with_a_value(account), "{}: {account}", metric.key);
        opening += 1;
    }
    println!(
        "{opening} of {} accounts open with the value they are about",
        response.metrics.len()
    );
    assert_eq!(opening, 11, "the trial no longer answers eleven quantities");
}

/// A rule on the number's chain declined, so the number's account is that rule's own refusal.
///
/// Six of the eight absences on this recording, and the sentence is compared in full rather
/// than searched for inside a longer one: an account carrying the refusal plus anything else is
/// a sentence composed here, which is the second producer this change exists to end.
#[test]
fn a_quantity_whose_chain_declined_carries_that_rules_own_refusal() {
    let response = analysed(&trial_at(INTERRUPTED), &spine_request());
    let block = accounts(&response);
    let sentences = wire_sentences(&response);
    assert_eq!(sentences.len(), 1, "{sentences:#?}");

    let mut accounted: Vec<&str> = Vec::new();
    for metric in valueless(&response) {
        let Some((_, sentence)) = sentences
            .iter()
            .find(|(method_id, _)| chain_names(metric, method_id))
        else {
            continue;
        };
        let account = account_of(&block, &metric.key);
        assert_eq!(
            account, sentence,
            "the account of {} is not the refusal's own sentence",
            metric.key
        );
        accounted.push(&metric.key);
    }

    println!(
        "{} of {} quantities carry the sentence of a rule on their own chain",
        accounted.len(),
        response.metrics.len()
    );
    assert_eq!(
        accounted.len(),
        6,
        "the recording no longer attributes six absences: {accounted:?}"
    );
    // The rule declined once and every account of it is the same string, so a build writing one
    // sentence per quantity would still pass the comparison above. The refusal it came from is
    // named here, so an account borrowed from some other rule's decline reddens.
    assert_eq!(
        refusal_from_rule(&response.refusals[0]).method_id,
        "onset.threshold.noise_relative"
    );
}

/// The refusal that is reachable only through the arithmetic, which is the half a second
/// implementation of this question had dropped.
///
/// `jumpheight.dj.mcmahon_correction_factor` is the rule that computed the quantity, not a rule
/// that fed it, so it is named by `computed_by` and by nothing in `contributing_method_ids`. A
/// reader of the landmark chain alone finds no cause and reports none, and the swept panel did
/// exactly that on 60 of 75 variants.
#[test]
fn a_rule_that_declined_off_the_landmark_chain_still_accounts_for_the_number_it_computed() {
    let request = binding(
        "jump_height.takeoff_frame",
        "jumpheight.dj.mcmahon_correction_factor",
    );
    let response = analysed(&fixture("subject01_trial1"), &request);
    let key = "jump_height_from_takeoff_meters";
    let metric = response.metric(key).expect("the quantity is reported");
    assert!(metric.value.is_none(), "{:?}", metric.value);

    // The discriminating fact: the declining rule is the arithmetic and is absent from the
    // landmark chain, so an attribution reading only that list cannot reach it.
    assert_eq!(
        metric.computed_by.as_deref(),
        Some("jumpheight.dj.mcmahon_correction_factor")
    );
    assert!(
        !metric
            .contributing_method_ids
            .iter()
            .any(|id| id == "jumpheight.dj.mcmahon_correction_factor"),
        "{:?}",
        metric.contributing_method_ids
    );

    let sentences = wire_sentences(&response);
    let (_, sentence) = sentences
        .iter()
        .find(|(method_id, _)| method_id == "jumpheight.dj.mcmahon_correction_factor")
        .expect("the rule declined");
    assert_eq!(account_of(&accounts(&response), key), sentence);
    println!("{sentence}");
}

/// The arithmetic ran and produced something that is not a number, and nothing is said about
/// it.
///
/// Two of the eleven on this recording, and they are the two computed over the weighing window
/// that holds its three unreadable samples. The count of empty accounts is compared against the
/// count of metrics in this state rather than against zero, because a build that described no
/// absence at all would satisfy any assertion written the other way round.
#[test]
fn an_arithmetic_that_produced_no_number_says_nothing_and_the_two_counts_agree() {
    let response = analysed(&trial_at(INTERRUPTED), &spine_request());
    let block = accounts(&response);

    let carried_no_number: BTreeSet<&str> = response
        .metrics
        .iter()
        .filter(|metric| metric.carried_no_number)
        .map(|metric| metric.key.as_str())
        .collect();
    let empty: BTreeSet<&str> = response
        .metrics
        .iter()
        .filter(|metric| account_of(&block, &metric.key).is_empty())
        .map(|metric| metric.key.as_str())
        .collect();

    println!(
        "{} of {} carried no number, {} of {} carry an empty account",
        carried_no_number.len(),
        response.metrics.len(),
        empty.len(),
        block.len()
    );
    assert_eq!(
        carried_no_number.len(),
        2,
        "the recording no longer produces two non-numbers: {carried_no_number:?}"
    );
    assert_eq!(
        empty, carried_no_number,
        "an account is empty for a quantity that is not in this state, or a quantity in this \
         state carries a sentence somebody wrote for it"
    );
    // Every other absence on this recording is accounted for, so the empty count above is the
    // state rather than the whole absent population.
    assert_eq!(valueless(&response).len(), 8);
}

/// A condition suppressed the quantity, so its account is that signal's own remedy.
///
/// Five of eleven on this recording, with zero refusals: a build that reached only refusals
/// would leave every one of these blank in both columns and pass any guard whose population is
/// the eight fixtures on which every landmark places.
#[test]
fn a_quantity_a_condition_suppressed_carries_the_signals_own_remedy() {
    let response = analysed(
        &fixture("synthetic_untrimmed_step_off_after_jump"),
        &inverting_request(),
    );
    assert!(response.refusals.is_empty(), "{:#?}", response.refusals);
    assert_eq!(response.signals.len(), 1, "{:#?}", response.signals);
    let signal = &response.signals[0];

    // The remedy read back off the wire, which is the string every surface but this one is
    // handed, rather than the field this test is holding.
    let wire = serde_json::to_value(&response.signals).expect("the signals serialise");
    let remedy = wire[0]["remedy"]
        .as_str()
        .expect("the signal carries a remedy")
        .to_string();

    let block = accounts(&response);
    let absent: BTreeSet<&str> = valueless(&response)
        .iter()
        .map(|metric| metric.key.as_str())
        .collect();
    let named: BTreeSet<&str> = signal.qualifies.iter().map(String::as_str).collect();
    println!("absent {absent:?}");
    assert_eq!(
        named, absent,
        "a column came back empty the signal does not account for, or the signal names a column \
         carrying a number"
    );
    assert_eq!(absent.len(), 5, "the recording no longer suppresses five");

    for key in &absent {
        assert_eq!(
            account_of(&block, key),
            remedy,
            "the account of {key} is not the signal's own remedy"
        );
    }
    println!(
        "{} of {} accounts are the remedy the signal wrote",
        absent.len(),
        block.len()
    );
}

/// A rule ran, returned nothing and declined nothing, so nothing is said and nothing is
/// invented.
///
/// `phase.propulsion_start.zero_velocity` on the recording where the athlete steps off the
/// plate: the velocity never crosses zero inside the interval, and the rule reports the key
/// with no value rather than a refusal. One of twelve. Before this change the row was absent
/// from every long-form table, so the silence was invisible; it is now a row with both columns
/// empty, which is the state CONVENTIONS section 3 forbids and which the plan raises separately
/// rather than folding in.
#[test]
fn a_rule_that_returned_nothing_without_declining_says_nothing_and_is_visible_as_a_row() {
    let request = binding(
        "propulsion_phase_start",
        "phase.propulsion_start.zero_velocity",
    );
    let response = analysed(&fixture("synthetic_untrimmed_step_off"), &request);
    let key = "propulsion_phase_start_seconds";
    let metric = response.metric(key).expect("the quantity is reported");

    assert!(metric.value.is_none());
    assert!(!metric.carried_no_number, "this is not the fourth state");
    assert!(
        !response
            .refusals
            .iter()
            .any(|declined| chain_names(metric, &declined.method_id)),
        "a rule on this quantity's chain declined, so this is not the silent state: {:#?}",
        response.refusals
    );
    assert!(
        !response
            .signals
            .iter()
            .any(|signal| signal.qualifies.iter().any(|named| named == key)),
        "a signal accounts for this quantity, so this is not the silent state"
    );

    let block = accounts(&response);
    assert!(
        block.contains_key(key),
        "the row is absent again, which is the shape a reader cannot filter for"
    );
    assert_eq!(account_of(&block, key), "");
    println!(
        "1 of {} quantities ran and returned nothing, and the row says so by being there",
        response.metrics.len()
    );
    // The other eleven are not in this state, so the fixture is not one where everything is
    // silent and this assertion is about one rule.
    assert_eq!(
        response
            .metrics
            .iter()
            .filter(|other| other.value.is_none())
            .count(),
        1
    );
}

/// No account of a quantity without a number asserts a measurement.
///
/// The property `a_quantity_with_no_value_gives_no_account_rather_than_an_invented_one` held,
/// carried over to a block that now holds a key for every quantity. Taken over the union of the
/// four absent states rather than over one, and the denominator is printed, because zero
/// offences out of zero quantities is what a guard looking at nothing reports.
#[test]
fn no_account_of_a_quantity_without_a_number_opens_with_a_value() {
    let populations: Vec<(&str, AnalysisResponse)> = vec![
        (
            "interrupted",
            analysed(&trial_at(INTERRUPTED), &spine_request()),
        ),
        (
            "inverted",
            analysed(
                &fixture("synthetic_untrimmed_step_off_after_jump"),
                &inverting_request(),
            ),
        ),
        (
            "mcmahon on trial1",
            analysed(
                &fixture("subject01_trial1"),
                &binding(
                    "jump_height.takeoff_frame",
                    "jumpheight.dj.mcmahon_correction_factor",
                ),
            ),
        ),
        (
            "silent propulsion start",
            analysed(
                &fixture("synthetic_untrimmed_step_off"),
                &binding(
                    "propulsion_phase_start",
                    "phase.propulsion_start.zero_velocity",
                ),
            ),
        ),
    ];

    let mut examined = 0usize;
    let mut offences: Vec<String> = Vec::new();
    for (name, response) in &populations {
        let block = accounts(response);
        for metric in valueless(response) {
            examined += 1;
            let account = account_of(&block, &metric.key);
            if opens_with_a_value(account) {
                offences.push(format!("{name}: {} reads {account}", metric.key));
            }
        }
    }
    println!(
        "{examined} quantities with no number, across {} recordings",
        populations.len()
    );
    assert_eq!(examined, 15, "the four populations changed shape");
    assert!(
        offences.is_empty(),
        "an account of a number nobody computed asserts a measurement: {offences:?}"
    );
}

/// A refusal is never written against a quantity that carries a number.
///
/// The population is real: the interrupted recording answers three of eleven while a rule on
/// six other chains declined, and that rule is on the chain of quantities that answered as well
/// as of quantities that did not. Writing its sentence under a number in front of a reader
/// would tell them the number is absent.
///
/// Read off the accounts a reader meets rather than off the predicate that fills them, so a
/// second implementation of the attribution question could not agree with itself here.
#[test]
fn a_refusal_accounts_for_no_quantity_that_carries_a_number() {
    let response = analysed(&trial_at(INTERRUPTED), &spine_request());
    let declining = "onset.threshold.noise_relative";
    let block = accounts(&response);
    let sentence = wire_sentences(&response)
        .into_iter()
        .find(|(method_id, _)| method_id == declining)
        .map(|(_, sentence)| sentence)
        .expect("the onset rule declined");

    let mut answered: Vec<&str> = Vec::new();
    let mut claimed: Vec<&str> = Vec::new();
    let mut silent: Vec<&str> = Vec::new();
    for metric in &response.metrics {
        let account = account_of(&block, &metric.key);
        match (metric.value, account == sentence) {
            (Some(value), false) => {
                assert!(account.starts_with(&recorded_number_text(value)));
                answered.push(&metric.key);
            }
            (Some(_), true) => panic!(
                "{} carries a number and the refusal's sentence with it",
                metric.key
            ),
            (None, true) => claimed.push(&metric.key),
            (None, false) => silent.push(&metric.key),
        }
    }
    println!("{answered:?} answered, {claimed:?} claimed, {silent:?} silent");
    assert_eq!(answered.len(), 3, "{answered:?}");
    assert_eq!(claimed.len(), 6, "{claimed:?}");
    assert_eq!(silent.len(), 2, "{silent:?}");
}

/// The value is half the attribution, and it is the half that has to be shown working.
///
/// No request this build can be given puts a declining rule on the chain of a quantity that
/// answered: 38,475 analyses, being every one of the nine committed recordings under each of
/// the 75 combinations of the three landmark rule sets, alone and with each of the derived rules
/// bound in turn, produce zero such pairs. So the guard above is a boundary rather than a
/// discriminator, and on its own it would pass on an attribution that never read the value at
/// all.
///
/// This is the pair that discriminates, and it is taken off a real response rather than a
/// metric built here. `flight_time_seconds` answers on the damaged recording and its chain
/// names the takeoff rule, so one predicate says the rule is on the chain and the other has to
/// say the rule's decline would not account for the number. A build that dropped the value from
/// the question makes the two agree.
#[test]
fn a_rule_on_the_chain_of_a_quantity_that_answered_accounts_for_none_of_it() {
    let response = analysed(&trial_at(INTERRUPTED), &spine_request());
    let answered = response
        .metric("flight_time_seconds")
        .expect("the quantity is reported");
    assert!(
        answered.value.is_some(),
        "the recording no longer answers the flight time, so this pair compares nothing"
    );

    let on_its_chain = answered
        .contributing_method_ids
        .last()
        .expect("the number rests on rules")
        .clone();
    println!("{} rests on {on_its_chain}", answered.key);
    assert!(chain_names(answered, &on_its_chain));
    assert!(
        !plateforce_analysis::chain::accounts_for(answered, &on_its_chain),
        "a decline by {on_its_chain} would be written under a number that is sitting in front of \
         the reader"
    );

    // And the same rule does account for a quantity of the same response that came back empty,
    // so the predicate is not simply answering no to everything.
    let empty = response
        .metrics
        .iter()
        .find(|metric| metric.value.is_none() && chain_names(metric, &on_its_chain))
        .unwrap_or_else(|| {
            panic!("no quantity resting on {on_its_chain} came back empty on this recording")
        });
    assert!(plateforce_analysis::chain::accounts_for(
        empty,
        &on_its_chain
    ));
    println!(
        "{} rests on the same rule, came back empty, and is accounted for by it",
        empty.key
    );
}

/// A name that is the front of two rules claims nothing.
///
/// The property `derive::quantities_of_rule`'s second test held, asserted where attribution now
/// happens. `jumpheight.takeoff` is the front of two rules reporting two different heights, so
/// a lookup matching on it would point a reader at a blank cell the refusal has nothing to do
/// with. The control is the full id, which does claim its quantity, so a comparison that
/// matched nothing at all could not read as clean.
#[test]
fn a_name_that_is_the_front_of_two_rules_claims_no_quantity() {
    let response = analysed(&fixture("subject01_trial1"), &spine_request());
    assert!(
        metrics_resting_on(&response, "jumpheight.takeoff").is_empty(),
        "{:?}",
        metrics_resting_on(&response, "jumpheight.takeoff")
    );
    assert!(metrics_resting_on(&response, "flight_time").is_empty());
    assert!(metrics_resting_on(&response, "").is_empty());

    let whole = metrics_resting_on(&response, "jumpheight.takeoff.impulse_momentum");
    assert_eq!(
        whole,
        vec!["jump_height_from_takeoff_meters".to_string()],
        "the whole name claims nothing either, so the three assertions above prove nothing"
    );
}
