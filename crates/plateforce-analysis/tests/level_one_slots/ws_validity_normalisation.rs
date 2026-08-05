//! The trial-validity and normalisation families, on the committed subject-01 trials.
//!
//! These two families are the first whose inputs are not the force trace alone. A validity
//! gate scores a trial against boundaries other rules placed, and a normalised number divides
//! a quantity another construct's chosen rule reported. So what is read here is the size of
//! the disagreements the registry records, and, in every case, whether the record names both
//! of the things the number rests on.
//!
//! What is measured below: the two pre-tension criteria against each other on a real
//! recording, what the two jump-type thresholds do across athlete mass, what each gate
//! reports over all six committed trials, the exponent gap between ratio and allometric
//! scaling, and the chain behind a per-kilogram number.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice};

use crate::common::{committed_trial, default_request, prepared, COMMITTED_TRIALS};

const VALIDITY_CONSTRUCT: &str = "trial_validity";
const PRETENSION_RULE: &str = "trial.gate.pretension_ceiling";
const CONTAMINATION_RULE: &str = "qc.countermovement_contamination.chavda2020";
const TRANSIENT_RULE: &str = "qc.transient_peak_count.pedley2023";
const FLIGHT_WINDOW_RULE: &str = "qc.flight_time_acceptance_window";

const JUMP_TYPE_CONSTRUCT: &str = "jump_type";
const FIXED_RULE: &str = "qc.jump_type_autodetection.sams";
const SCALED_RULE: &str = "qc.jump_type_autodetection.mass_scaled";
const CLASSIFICATION_KEY: &str = "jump_type_is_countermovement";
const UNWEIGHTING_KEY: &str = "jump_type_unweighting_newtons";
const THRESHOLD_KEY: &str = "jump_type_threshold_newtons";

const NORMALISATION_CONSTRUCT: &str = "normalisation_basis";
const RATIO_RULE: &str = "norm.ratio_bodymass";
const ALLOMETRIC_RULE: &str = "norm.allometric";
const PERCENT_OF_PEAK_RULE: &str = "norm.pct_peak_force";
const RATIO_KEY: &str = "peak_force_per_body_mass_newtons_per_kilogram";
const ALLOMETRIC_KEY: &str = "peak_force_allometric_newtons_per_kilogram_to_the_exponent";
const ALLOMETRIC_DIVISOR_KEY: &str = "allometric_divisor_kilograms_to_the_exponent";

const PEAK_FORCE_CONSTRUCT: &str = "peak_force";
const PEAK_FORCE_RULE: &str = "force.peak.gross";
const PEAK_FORCE_KEY: &str = "peak_force_newtons";
const NET_PEAK_CONSTRUCT: &str = "net_peak_force";
const NET_PEAK_RULE: &str = "force.peak.net";

/// The window every extremum here is taken over. A peak taken across the whole recording is
/// the landing on a countermovement jump, so the normalised numbers below rest on this choice
/// as much as on the mass they are divided by.
const WINDOW_CONSTRUCT: &str = "analysis_window";
const WINDOW_RULE: &str = "window_end.takeoff.detected";

/// The boundary rules the braking period resolves through, which the transient-peak gate
/// reads and nothing else here does.
const BRAKING_CONSTRUCT: &str = "braking_phase_start";
const BRAKING_RULE: &str = "phase.braking_start.zero_net_force";
const PROPULSION_START_CONSTRUCT: &str = "propulsion_phase_start";
const PROPULSION_START_RULE: &str = "phase.propulsion_start.zero_velocity";
const LANDING_CONSTRUCT: &str = "landing";
const LANDING_RULE: &str = "landing.threshold.tied_to_takeoff";

/// The athlete this corpus records, whose mass the fixture files do not carry.
const SUBJECT_MASS_KILOGRAMS: f64 = 52.0;

struct Asked {
    request: AnalysisRequest,
}

impl Asked {
    /// A request carrying the boundary rules these two families read, and nothing else stated.
    fn new() -> Self {
        let mut request = default_request();
        for (construct, rule) in [
            (WINDOW_CONSTRUCT, WINDOW_RULE),
            (PEAK_FORCE_CONSTRUCT, PEAK_FORCE_RULE),
            (NET_PEAK_CONSTRUCT, NET_PEAK_RULE),
            (LANDING_CONSTRUCT, LANDING_RULE),
            (BRAKING_CONSTRUCT, BRAKING_RULE),
            (PROPULSION_START_CONSTRUCT, PROPULSION_START_RULE),
        ] {
            request.derived.insert(
                construct.to_string(),
                MethodChoice {
                    method_id: rule.to_string(),
                    ..Default::default()
                },
            );
        }
        Self { request }
    }

    fn mass(mut self, kilograms: f64) -> Self {
        self.request.body_mass_kilograms = Some(kilograms);
        self
    }

    fn rule(mut self, construct: &str, method_id: &str) -> Self {
        self.request.derived.insert(
            construct.to_string(),
            MethodChoice {
                method_id: method_id.to_string(),
                ..Default::default()
            },
        );
        self
    }

    fn stating(mut self, construct: &str, method_id: &str, values: &[(&str, &str)]) -> Self {
        let mut options = BTreeMap::new();
        let mut parameters = BTreeMap::new();
        for (name, value) in values {
            match value.parse::<f64>() {
                Ok(number) => {
                    parameters.insert((*name).to_string(), number);
                }
                Err(_) => {
                    options.insert((*name).to_string(), (*value).to_string());
                }
            }
        }
        self.request.derived.insert(
            construct.to_string(),
            MethodChoice {
                method_id: method_id.to_string(),
                options,
                parameters,
                ..Default::default()
            },
        );
        self
    }

    /// Filled here rather than in `new`, because every builder above this one names another
    /// slot, and a choice inserted into a filled request carries its own empty declared table.
    fn on(&self, trial: &plateforce_core::Trial) -> AnalysisResponse {
        run(trial, &prepared(self.request.clone())).expect("the request is well formed")
    }
}

fn number(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response.metric(key).and_then(|metric| metric.value)
}

fn declined(response: &AnalysisResponse, method_id: &str) -> Option<String> {
    response
        .refusals
        .iter()
        .find(|rule| rule.method_id == method_id)
        .map(|rule| rule.refusal.to_string())
}

/// What the two pre-tension criteria say about one recording, and which of them decides.
///
/// The entry's recorded finding is that a fixed newton ceiling and a percentage of bodyweight
/// are not two strictnesses of one rule, and that the gap between them follows athlete mass.
/// Held here on a real trial rather than argued: the two verdicts are reported side by side,
/// and moving the stated criterion moves which of them the trial is admitted by.
#[test]
fn the_criterion_a_caller_states_is_the_one_that_admits_the_trial() {
    let trial = committed_trial("subject01_trial1");
    // A band tight enough to reject this trial and a ceiling loose enough to admit it, so the
    // two criteria genuinely disagree here and the stated one has something to decide.
    let values: &[(&str, &str)] = &[("ceiling", "1000"), ("band_pct_bodyweight", "0.5")];

    let mut admitted_under = Vec::new();
    for criterion in ["absolute_newtons_above_bodyweight", "percent_of_bodyweight"] {
        let mut stated = values.to_vec();
        stated.push(("criterion", criterion));
        let response = Asked::new()
            .stating(VALIDITY_CONSTRUCT, PRETENSION_RULE, &stated)
            .on(&trial);
        let excursion = number(&response, "pretension_excursion_newtons")
            .expect("the gate read the stretch before the effort");
        let by_ceiling = number(
            &response,
            "trial_validity_pretension_admitted_at_the_absolute_ceiling",
        );
        let inside_band = number(
            &response,
            "trial_validity_pretension_admitted_inside_the_percentage_band",
        );
        let verdict = number(&response, "trial_validity_pretension_admitted");
        println!(
            "{criterion}: excursion {excursion:.4} N, by ceiling {by_ceiling:?}, inside band \
             {inside_band:?}, admitted {verdict:?}"
        );
        assert_eq!(
            by_ceiling,
            Some(1.0),
            "a 1000 N ceiling admits this trial whichever criterion decides"
        );
        assert_eq!(
            inside_band,
            Some(0.0),
            "a 0.5 percent band rejects this trial whichever criterion decides"
        );
        admitted_under.push(verdict);
    }
    assert_eq!(
        admitted_under,
        vec![Some(1.0), Some(0.0)],
        "the stated criterion decided nothing: both readings admitted alike"
    );
}

/// The two jump-type rules answer one question and the choice moves the threshold.
///
/// A constant newton value is a wildly varying fraction of bodyweight, which is the entry's
/// own recorded bias, so the scaled rule has to hold a lighter athlete to a lower threshold
/// and a heavier one to a higher. At the anchor mass the two meet exactly, which is what the
/// anchor is for and is the reading that would go red if the scaling were dropped.
#[test]
fn the_two_jump_type_rules_report_one_answer_and_the_threshold_follows_the_athlete() {
    let trial = committed_trial("subject01_trial1");
    let fixed = Asked::new()
        .mass(SUBJECT_MASS_KILOGRAMS)
        .rule(JUMP_TYPE_CONSTRUCT, FIXED_RULE)
        .on(&trial);
    let fixed_threshold = number(&fixed, THRESHOLD_KEY).expect("the fixed rule ran");
    let unweighting = number(&fixed, UNWEIGHTING_KEY).expect("the fixed rule ran");

    let mut scaled_thresholds = Vec::new();
    for kilograms in [45.0, 87.5, 150.0] {
        let response = Asked::new()
            .mass(kilograms)
            .rule(JUMP_TYPE_CONSTRUCT, SCALED_RULE)
            .on(&trial);
        let threshold = number(&response, THRESHOLD_KEY).expect("the scaled rule ran");
        // Both rules report all three keys, which is what one question looks like: the choice
        // moves values rather than settling which quantities exist.
        for key in [CLASSIFICATION_KEY, UNWEIGHTING_KEY, THRESHOLD_KEY] {
            assert!(
                number(&response, key).is_some(),
                "{SCALED_RULE} reported nothing for {key}"
            );
            assert!(
                number(&fixed, key).is_some(),
                "{FIXED_RULE} reported nothing for {key}"
            );
        }
        println!(
            "{kilograms:>6.1} kg: threshold {threshold:8.4} N, {:.1} percent of the {unweighting:.1} N \
             this trial unloaded",
            threshold / unweighting * 100.0
        );
        scaled_thresholds.push((kilograms, threshold));
    }
    assert!(
        scaled_thresholds[0].1 < fixed_threshold,
        "a 45 kg athlete was held to {:.4} N against the incumbent {fixed_threshold:.4} N",
        scaled_thresholds[0].1
    );
    assert!(
        (scaled_thresholds[1].1 - fixed_threshold).abs() < 1e-9,
        "the two rules disagree at the anchor mass: {:.6} against {fixed_threshold:.6}",
        scaled_thresholds[1].1
    );
    assert!(
        scaled_thresholds[2].1 > fixed_threshold,
        "a 150 kg athlete was held to {:.4} N against the incumbent {fixed_threshold:.4} N",
        scaled_thresholds[2].1
    );
}

/// A gate that cannot reach what it scores declines by name, and reports no verdict.
///
/// The whole of this construct's value is that a rejection is visible. A gate that answered
/// "admitted" when it never ran would be the silent default the registry exists to prevent,
/// and a boolean has no third value to say which of the two it is, so the refusal is where it
/// has to go. Read on the request that withholds the braking boundaries the gate reads.
#[test]
fn a_gate_that_cannot_reach_its_input_declines_rather_than_admitting() {
    let trial = committed_trial("subject01_trial1");
    let mut starved = Asked::new().rule(VALIDITY_CONSTRUCT, TRANSIENT_RULE);
    starved.request.derived.remove(BRAKING_CONSTRUCT);
    starved.request.derived.remove(PROPULSION_START_CONSTRUCT);
    let response = starved.on(&trial);

    let refusal = declined(&response, TRANSIENT_RULE)
        .unwrap_or_else(|| panic!("{TRANSIENT_RULE} reported no refusal"));
    println!("starved of its braking period: {refusal}");
    assert!(
        refusal.contains(BRAKING_CONSTRUCT) && refusal.contains(PROPULSION_START_CONSTRUCT),
        "the refusal did not name what it could not reach: {refusal}"
    );
    for key in [
        "trial_validity_transient_peaks_admitted",
        "braking_transient_peak_count",
    ] {
        assert_eq!(
            number(&response, key),
            None,
            "{TRANSIENT_RULE} reported {key} on a recording it could not read"
        );
    }

    // And the same gate, handed its boundaries, produces. Without this the assertion above
    // would pass for a rule that declines on every recording.
    let reached = Asked::new()
        .rule(VALIDITY_CONSTRUCT, TRANSIENT_RULE)
        .on(&trial);
    assert_eq!(declined(&reached, TRANSIENT_RULE), None);
    assert!(number(&reached, "braking_transient_peak_count").is_some());
}

/// A number expressed per kilogram names the rule that produced it and the mass it was
/// divided by.
///
/// Two dependencies, and the record has to carry both. The mass is a value the caller stated,
/// so it lands among the globals; the peak is another construct's answer, so the entry that
/// produced it lands in this number's own chain. Before the chain carried it, a per-kilogram
/// figure named the landmark rules and said nothing about which of the two peak-force rules
/// its numerator came from.
#[test]
fn a_per_kilogram_number_names_the_peak_it_divided_and_the_mass_it_divided_by() {
    let trial = committed_trial("subject01_trial1");
    let response = Asked::new()
        .mass(SUBJECT_MASS_KILOGRAMS)
        .rule(NORMALISATION_CONSTRUCT, RATIO_RULE)
        .on(&trial);

    let peak = number(&response, PEAK_FORCE_KEY).expect("the peak-force rule ran");
    let per_kilogram = number(&response, RATIO_KEY).expect("the ratio rule ran");
    println!("{peak:.4} N over {SUBJECT_MASS_KILOGRAMS} kg is {per_kilogram:.4} N/kg");
    assert!(
        (per_kilogram - peak / SUBJECT_MASS_KILOGRAMS).abs() < 1e-9,
        "the reported ratio is not the reported peak over the stated mass"
    );

    let chain = response
        .metric(RATIO_KEY)
        .expect("the ratio metric is present")
        .contributing_method_ids
        .clone();
    assert!(
        chain.iter().any(|id| id == PEAK_FORCE_RULE),
        "the chain behind {RATIO_KEY} does not name the rule that produced its numerator: {chain:?}"
    );
    assert!(
        response
            .bound_globals
            .iter()
            .any(|global| global.name == "body_mass_kilograms"),
        "the record does not carry the mass the number was divided by"
    );
}

/// The exponent is the whole of the argument between ratio and allometric scaling, so it has
/// to move the number, and the divisor has to be what the scaling divided by.
///
/// Reported together because their units differ with the exponent: a reader holding the scaled
/// value alone cannot say what it is per, which is the failure the entry's own gui note names.
#[test]
fn the_exponent_moves_the_scaled_number_and_the_divisor_says_what_it_is_per() {
    let trial = committed_trial("subject01_trial1");
    let mut scaled = Vec::new();
    for exponent in ["0.67", "1.0"] {
        let response = Asked::new()
            .mass(SUBJECT_MASS_KILOGRAMS)
            .stating(
                NORMALISATION_CONSTRUCT,
                ALLOMETRIC_RULE,
                &[("provenance", "assumed"), ("exponent", exponent)],
            )
            .on(&trial);
        let peak = number(&response, PEAK_FORCE_KEY).expect("the peak-force rule ran");
        let divisor = number(&response, ALLOMETRIC_DIVISOR_KEY).expect("the rule ran");
        let value = number(&response, ALLOMETRIC_KEY).expect("the rule ran");
        println!("exponent {exponent}: divisor {divisor:.4}, {value:.4} from a {peak:.1} N peak");
        assert!(
            (peak / divisor - value).abs() < 1e-9,
            "the divisor is not what the scaling divided by at exponent {exponent}"
        );
        scaled.push(value);
    }
    assert!(
        scaled[0] / scaled[1] > 3.0,
        "the two published exponents moved the number by {:.4}, which a reader could take for \
         rounding",
        scaled[0] / scaled[1]
    );

    // A fitted exponent is estimated from the sample at hand, and this analysis holds one
    // trial, so the rule takes the caller's number rather than the assumed one and says so.
    let fitted = Asked::new()
        .mass(SUBJECT_MASS_KILOGRAMS)
        .stating(
            NORMALISATION_CONSTRUCT,
            ALLOMETRIC_RULE,
            &[("provenance", "fitted")],
        )
        .on(&trial);
    let refusal = declined(&fitted, ALLOMETRIC_RULE)
        .unwrap_or_else(|| panic!("a fitted exponent nobody stated was filled in"));
    println!("fitted with no exponent stated: {refusal}");
    assert!(refusal.contains("exponent"), "{refusal}");
}

/// Early force as a share of the peak, taken against the net peak the caller's own rule
/// reported.
///
/// The entry is incompatible with the gross convention, because a ratio whose numerator and
/// denominator both carry a bodyweight offset is not a fraction of anything. So the number it
/// divides by is `net_peak_force`'s answer, and the chain says so.
#[test]
fn early_force_is_a_share_of_the_net_peak_the_analysis_reported() {
    let trial = committed_trial("subject01_trial1");
    let mut shares = Vec::new();
    for seconds in ["0.05", "0.15"] {
        let response = Asked::new()
            .stating(
                NORMALISATION_CONSTRUCT,
                PERCENT_OF_PEAK_RULE,
                &[("time_after_onset_seconds", seconds)],
            )
            .on(&trial);
        let net_peak = number(&response, "net_peak_force_newtons").expect("the net peak ran");
        let early = number(&response, "early_net_force_newtons").expect("the rule ran");
        let share =
            number(&response, "early_net_force_share_of_peak_percent").expect("the rule ran");
        println!("{seconds} s after onset: {early:9.4} N, {share:8.4} percent of {net_peak:.4} N");
        assert!((early / net_peak * 100.0 - share).abs() < 1e-9);
        let chain = response
            .metric("early_net_force_share_of_peak_percent")
            .expect("the metric is present")
            .contributing_method_ids
            .clone();
        assert!(
            chain.iter().any(|id| id == NET_PEAK_RULE),
            "the chain does not name the rule behind the denominator: {chain:?}"
        );
        shares.push(share);
    }
    assert!(
        (shares[0] - shares[1]).abs() > 1.0,
        "the stated instant moved the share by {:.6} percent, so it reached no arithmetic",
        (shares[0] - shares[1]).abs()
    );
}

/// The window a caller states is the window the candidates are judged against, and a rejected
/// candidate is counted rather than dropped.
///
/// The entry exists because three shipped tools apply a window and none reports what it threw
/// away, so the bounds reaching the verdict is the whole of it. Read at three windows on one
/// recording: one that admits the flight this analysis found, one whose floor sits above it,
/// and one whose ceiling sits below it. Without this a rule that read neither bound would
/// answer on every trial and pass every other guard in this file.
#[test]
fn the_window_a_caller_states_decides_which_candidates_survive() {
    let trial = committed_trial("subject01_trial1");
    let mut verdicts = Vec::new();
    for (lower, upper) in [("0.1", "2.0"), ("0.9", "2.0"), ("0.01", "0.05")] {
        let response = Asked::new()
            .stating(
                VALIDITY_CONSTRUCT,
                FLIGHT_WINDOW_RULE,
                &[
                    ("selection", "first_qualifying"),
                    ("flight_threshold_n", "10"),
                    ("lower_seconds", lower),
                    ("upper_seconds", upper),
                ],
            )
            .on(&trial);
        let read = number(&response, "flight_candidates_read_count").expect("the gate ran");
        let rejected = number(&response, "flight_candidates_rejected_count").expect("the gate ran");
        let accepted = number(&response, "accepted_flight_seconds");
        let admitted =
            number(&response, "trial_validity_flight_window_admitted").expect("the gate ran");
        println!(
            "{lower} to {upper} s: {rejected:.0} of {read:.0} candidates rejected, accepted \
             {accepted:?}, admitted {admitted}"
        );
        verdicts.push((admitted, rejected, accepted));
    }
    assert_eq!(
        verdicts[0].0, 1.0,
        "the published window rejected this jump"
    );
    assert_eq!(
        verdicts[1].0, 0.0,
        "a floor above this flight time still admitted it, so lower_seconds reached no comparison"
    );
    assert_eq!(
        verdicts[2].0, 0.0,
        "a ceiling below this flight time still admitted it, so upper_seconds reached no comparison"
    );
    assert!(
        verdicts[1].1 > verdicts[0].1 && verdicts[2].1 > verdicts[0].1,
        "a window that admitted nothing counted no more rejections than one that admitted the \
         jump: {verdicts:?}"
    );
    assert!(
        verdicts[0].2.is_some() && verdicts[1].2.is_none(),
        "a window that qualified no candidate still reported a duration"
    );
}

/// What the four gates say about the six committed trials, with their denominators.
///
/// Printed rather than asserted trial by trial, because the numbers are a property of the
/// corpus. What is asserted is that every gate answered on every trial it could reach, and
/// that the flight window answered on exactly the trials that hold a landing: five of the six
/// were trimmed before the athlete came back down, so a gate reading past takeoff has a
/// denominator of one and saying six would be the wrong one.
#[test]
fn every_gate_answers_or_says_what_it_could_not_reach_across_the_committed_trials() {
    let gates: &[(&str, &str)] = &[
        (PRETENSION_RULE, "trial_validity_pretension_admitted"),
        (
            CONTAMINATION_RULE,
            "trial_validity_countermovement_admitted",
        ),
        (TRANSIENT_RULE, "trial_validity_transient_peaks_admitted"),
        (FLIGHT_WINDOW_RULE, "trial_validity_flight_window_admitted"),
    ];
    let stated: &[(&str, &[(&str, &str)])] = &[
        (
            PRETENSION_RULE,
            &[("criterion", "absolute_newtons_above_bodyweight")],
        ),
        (CONTAMINATION_RULE, &[]),
        (TRANSIENT_RULE, &[]),
        (
            FLIGHT_WINDOW_RULE,
            &[
                ("selection", "first_qualifying"),
                ("flight_threshold_n", "10"),
            ],
        ),
    ];

    let mut answered = 0usize;
    let mut attempted = 0usize;
    let mut flight_answers = 0usize;
    for name in COMMITTED_TRIALS {
        let trial = committed_trial(name);
        let mut row = format!("{name:18}");
        for ((rule, key), (_, values)) in gates.iter().zip(stated) {
            attempted += 1;
            let response = Asked::new()
                .stating(VALIDITY_CONSTRUCT, rule, values)
                .on(&trial);
            match number(&response, key) {
                Some(verdict) => {
                    answered += 1;
                    if *rule == FLIGHT_WINDOW_RULE {
                        flight_answers += 1;
                    }
                    let short = rule.rsplit('.').next().unwrap_or(rule);
                    row.push_str(&format!(
                        "  {short} {}",
                        if verdict > 0.5 { "admit " } else { "reject" }
                    ));
                }
                None => row.push_str("  declined"),
            }
        }
        println!("{row}");
    }
    println!(
        "{answered} of {attempted} gate readings answered across {} trials",
        COMMITTED_TRIALS.len()
    );
    assert_eq!(
        answered, attempted,
        "a gate reached no answer on a trial it was handed every boundary for"
    );
    assert_eq!(
        flight_answers,
        COMMITTED_TRIALS.len(),
        "the flight window answered on {flight_answers} of {} trials",
        COMMITTED_TRIALS.len()
    );
}

/// What the transient-peak gate counts on a recording nobody filtered.
///
/// The rule offers a lower filter cutoff as an alternative to discarding the trial, which
/// conflates a data-quality failure with a filter setting: a user taking that option has
/// changed the signal every other entry is computed from in order to pass a gate. The count on
/// the raw corpus is what that trade looks like before anybody takes it.
#[test]
fn the_transient_peak_count_is_a_property_of_the_signal_as_it_arrives() {
    let mut counted = Vec::new();
    for name in COMMITTED_TRIALS {
        let trial = committed_trial(name);
        let response = Asked::new()
            .rule(VALIDITY_CONSTRUCT, TRANSIENT_RULE)
            .on(&trial);
        let count = number(&response, "braking_transient_peak_count").expect("the gate ran");
        let greatest = number(&response, "braking_greatest_force_newtons").expect("the gate ran");
        let admitted = number(&response, "trial_validity_transient_peaks_admitted");
        println!("{name:18} {count:6.0} peaks, greatest {greatest:9.4} N, admitted {admitted:?}");
        counted.push(count);
    }
    let lowest = counted.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        lowest > 3.0,
        "the raw corpus counted {lowest} peaks in braking on its quietest trial, so the \
         published ceiling of three has nothing to reject here"
    );
}
