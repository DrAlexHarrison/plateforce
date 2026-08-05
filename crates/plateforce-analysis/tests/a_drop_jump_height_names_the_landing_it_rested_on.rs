//! A drop-jump height, and what the arrival it integrates from does and does not decide.
//!
//! McMahon's single-plate method starts at the instant the athlete reached the plate and
//! recovers the velocity they arrived with from the standing period the recording ends in.
//! That instant is the `landing` construct, and until its rules were bound nothing placed it.
//!
//! Two things are measured here and they point opposite ways. The arrival decides whether the
//! rule runs: a landing that follows takeoff leaves the integration no interval, which is what
//! a jump begun from standing gives, and the rule refuses rather than reporting the number the
//! arithmetic would produce. The arrival does not decide the height: the correction is defined
//! by what the standing period reads, so moving the start shifts the series by a constant that
//! cancels, exactly as the stated box height cancels.
//!
//! Built rather than read from the corpus, because every corpus trial is a countermovement jump
//! trimmed to one jump and none of them holds a drop. The trace is assembled from velocity
//! changes, so what each phase is worth is known before the software reads it, and the height
//! the rule reports can be checked against arithmetic done outside it.
//!
//! `cargo test -p plateforce-analysis --test a_drop_jump_height_names_the_landing_it_rested_on`

use std::collections::BTreeMap;

use plateforce_analysis::document::refusal_from_rule;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{RefusalCode, Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED as GRAVITY};

const RATE_HZ: f64 = 1000.0;
const MASS_KILOGRAMS: f64 = 70.0;

/// What the athlete is actually travelling at when the first foot reaches the plate, which is
/// the number the whole method exists to recover. Below the 2.4258 m/s a 0.30 m box implies,
/// because the centre of mass does not fall the whole height of the box: this is the source's
/// own worked example, whose corrected arrival is 2.31 m/s from a 0.30 m box.
const ARRIVAL_VELOCITY: f64 = -2.31;
const TAKEOFF_VELOCITY: f64 = 2.60;
/// The box the athlete stepped off, which the deprecated route substitutes for the fall.
const BOX_HEIGHT_METERS: f64 = 0.30;

const ON_THE_BOX_SAMPLES: usize = 250;
const CONTACT_SAMPLES: usize = 250;
const LANDING_SAMPLES: usize = 250;
const STANDING_SAMPLES: usize = 1200;

const ARRIVAL_INDEX: usize = ON_THE_BOX_SAMPLES;
const TAKEOFF_INDEX: usize = ON_THE_BOX_SAMPLES + CONTACT_SAMPLES;

const TAKEOFF_FRAME: &str = "jump_height.takeoff_frame";
const TAKEOFF_KEY: &str = "jump_height_from_takeoff_meters";
const LANDING_CONSTRUCT: &str = "landing";
const MCMAHON: &str = "jumpheight.dj.mcmahon_correction_factor";
const DROP_FROM_BOX: &str = "jumpheight.dj.box_height_as_drop_height";
const LANDING_ABSOLUTE: &str = "landing.threshold.absolute_force";
const LANDING_TIED: &str = "landing.threshold.tied_to_takeoff";

fn system_weight_newtons() -> f64 {
    MASS_KILOGRAMS * GRAVITY
}

/// A drop jump, written as the force each phase needs to be worth the velocity change it makes.
///
/// The plate reads nothing while the athlete stands on the box. They arrive at
/// `ARRIVAL_VELOCITY`, the contact phase reverses that to `TAKEOFF_VELOCITY`, flight lasts
/// exactly as long as gravity takes to turn the one into its negative, the landing brings them
/// back to rest, and they stand still to the end of the recording.
///
/// Flight is longer than the stretch on the box, so the longest low-force run in the file is
/// the jump rather than the wait, and a takeoff rule flooring at the start of the trial finds
/// the right one. That ordering is deliberate: the opposite recording is the untrimmed-file
/// defect this project was founded on.
fn synthetic_drop_jump() -> Trial {
    let weight = system_weight_newtons();
    let interval = 1.0 / RATE_HZ;

    let flight_seconds = 2.0 * TAKEOFF_VELOCITY / GRAVITY;
    let flight_samples = (flight_seconds * RATE_HZ).round() as usize;

    let net_newtons = |velocity_change: f64, samples: usize| {
        MASS_KILOGRAMS * velocity_change / (samples as f64 * interval)
    };
    let contact = weight + net_newtons(TAKEOFF_VELOCITY - ARRIVAL_VELOCITY, CONTACT_SAMPLES);
    let landing = weight + net_newtons(TAKEOFF_VELOCITY, LANDING_SAMPLES);

    let mut force = Vec::new();
    force.extend(std::iter::repeat_n(0.0, ON_THE_BOX_SAMPLES));
    force.extend(std::iter::repeat_n(contact, CONTACT_SAMPLES));
    force.extend(std::iter::repeat_n(0.0, flight_samples));
    force.extend(std::iter::repeat_n(landing, LANDING_SAMPLES));
    force.extend(std::iter::repeat_n(weight, STANDING_SAMPLES));
    Trial::new(force, RATE_HZ).expect("the assembled drop jump is a trial")
}

/// The standing period the athlete holds at the end, which is where this method's body weight
/// and its velocity reference both come from.
fn standing_period_start(trial: &Trial) -> usize {
    trial.len() - STANDING_SAMPLES
}

/// The spine a drop jump needs: weighed over the standing period after the landing, and a
/// takeoff rule that considers every sample rather than flooring at the weighing window, which
/// on this protocol sits at the end of the file.
fn base(trial: &Trial) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: Some(standing_period_start(trial)),
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.longest_run".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn asking(
    trial: &Trial,
    landing_rule: &str,
    height_rule: &str,
    stated: &[(&str, f64)],
) -> AnalysisRequest {
    let mut request = base(trial);
    request.derived.insert(
        LANDING_CONSTRUCT.to_string(),
        MethodChoice {
            method_id: landing_rule.to_string(),
            ..Default::default()
        },
    );
    request.derived.insert(
        TAKEOFF_FRAME.to_string(),
        MethodChoice {
            method_id: height_rule.to_string(),
            parameters: stated
                .iter()
                .map(|(name, value)| ((*name).to_string(), *value))
                .collect(),
            ..Default::default()
        },
    );
    request
}

fn height_in(response: &AnalysisResponse) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == TAKEOFF_KEY)
        .and_then(|metric| metric.value)
}

fn refusal_naming(response: &AnalysisResponse, method_id: &str) -> Option<plateforce_core::Refusal> {
    response
        .refusals
        .iter()
        .find(|declined| declined.method_id == method_id)
        .map(refusal_from_rule)
}

/// The height the construction is worth, computed outside the software it is checking.
fn height_the_trace_was_built_to_give() -> f64 {
    TAKEOFF_VELOCITY.powi(2) / (2.0 * GRAVITY)
}

#[test]
fn the_spine_reads_the_drop_jump_the_way_the_trace_was_built() {
    let trial = synthetic_drop_jump();
    let response = run(&trial, &base(&trial)).expect("the spine runs");
    assert_eq!(
        response.takeoff_index,
        Some(TAKEOFF_INDEX),
        "the longest low-force run has to be the flight phase, not the wait on the box"
    );
    let weight = system_weight_newtons();
    let weighed = response
        .levels
        .system_weight_newtons
        .expect("the weighing rule ran");
    assert!(
        (weighed - weight).abs() < 1e-6,
        "the weighing window sits in the standing period and should read {weight} N, not {weighed} N"
    );
}

#[test]
fn a_drop_jump_height_rests_on_the_arrival_the_standing_period_recovers() {
    let trial = synthetic_drop_jump();
    let response = run(&trial, &asking(&trial, LANDING_ABSOLUTE, MCMAHON, &[]))
        .expect("the drop-jump height runs");

    let expected = height_the_trace_was_built_to_give();
    let reported = height_in(&response).expect("the rule produced a height");
    assert!(
        (reported - expected).abs() < 0.005,
        "the trace was built to give {expected:.4} m and the rule reported {reported:.4} m"
    );
}

/// The arrival is a landmark a rule placed, so the height has to name that rule. Before the
/// `landing` construct was bound there was no rule to name, and the instant the integration
/// started at rested on nothing a reader could look up.
#[test]
fn the_height_names_the_landing_rule_that_placed_its_start() {
    let trial = synthetic_drop_jump();
    let response = run(&trial, &asking(&trial, LANDING_ABSOLUTE, MCMAHON, &[]))
        .expect("the drop-jump height runs");

    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == TAKEOFF_KEY)
        .expect("the height is reported");
    assert!(
        metric
            .contributing_method_ids
            .iter()
            .any(|id| id == LANDING_ABSOLUTE),
        "the height rests on the arrival and its chain is {:?}",
        metric.contributing_method_ids
    );
    assert!(
        response
            .bound_methods
            .iter()
            .any(|bound| bound.method_id == LANDING_ABSOLUTE),
        "the landing rule ran, so it is on the record"
    );
}

/// The two landing entries are two claims about the same instant, and on a drop jump they part
/// company: one searches the recording and finds the arrival off the box, the other searches
/// forward from takeoff and finds the return after the jump. The registry calls the
/// disagreement genuine, and here it is, in samples.
#[test]
fn the_two_landing_rules_place_different_instants_on_a_drop_jump() {
    let trial = synthetic_drop_jump();
    let landing_seconds = |rule: &str| {
        let response = run(&trial, &asking(&trial, rule, MCMAHON, &[])).expect("the landing runs");
        response
            .metrics
            .iter()
            .find(|metric| metric.key == "landing_seconds")
            .and_then(|metric| metric.value)
    };
    let searched = landing_seconds(LANDING_ABSOLUTE).expect("the searching rule placed one");
    let tied = landing_seconds(LANDING_TIED).expect("the tied rule placed one");
    assert!(
        (searched - trial.time_at(ARRIVAL_INDEX)).abs() < 1e-9,
        "the searching rule reads the arrival off the box at {:.4} s, not {searched:.4} s",
        trial.time_at(ARRIVAL_INDEX)
    );
    assert!(
        tied > trial.time_at(TAKEOFF_INDEX),
        "the tied rule searches forward from takeoff, so its landing follows the jump: {tied:.4} s"
    );
}

/// A jump begun from standing has no arrival before takeoff, and the tied rule on a drop jump
/// finds the return after the jump. Integrating from there to takeoff runs backwards, so the
/// rule refuses instead of reporting the number that arithmetic would produce.
#[test]
fn a_landing_after_takeoff_is_refused_rather_than_integrated_backwards() {
    let trial = synthetic_drop_jump();
    let response =
        run(&trial, &asking(&trial, LANDING_TIED, MCMAHON, &[])).expect("the analysis runs");

    assert_eq!(
        height_in(&response),
        None,
        "no height is reported when the integration has no interval to run over"
    );
    // A search this recording gave nothing to, not a value the caller mis-stated: nobody typed
    // the landing, a rule placed it, and the recording is what holds no arrival before takeoff.
    let refusal = refusal_naming(&response, MCMAHON).expect("the rule said why");
    assert_eq!(refusal.code, RefusalCode::NoCrossing);
    assert_eq!(refusal.parameter.as_deref(), Some("landing"));
}

/// The body weight and the velocity reference are one window in the source, and it is the
/// standing period after landing. A window taken at the start of a drop-jump recording weighs
/// an empty plate, so the rule names the window rather than reporting what it would give.
#[test]
fn a_weighing_window_before_the_jump_is_refused_by_name() {
    let trial = synthetic_drop_jump();
    let mut request = asking(&trial, LANDING_ABSOLUTE, MCMAHON, &[]);
    request.weighing.start_index = Some(0);
    let response = run(&trial, &request).expect("the analysis runs");

    assert_eq!(height_in(&response), None);
    let refusal = refusal_naming(&response, MCMAHON).expect("the rule said why");
    assert_eq!(refusal.code, RefusalCode::ValueNotAccepted);
    assert_eq!(
        refusal.parameter.as_deref(),
        Some("weighing_window_start_seconds")
    );
}

/// The arrival is what the height rests on, so moving it has to move the height. A rising-edge
/// threshold high enough to miss the arrival takes the rule to a later sample of the contact
/// phase, and the interval it integrates over shortens.
/// The same trace with the arrival rising over 40 samples rather than stepping, so a threshold
/// stated for the rising edge lands on a different sample at each value.
fn synthetic_drop_jump_with_a_gradual_arrival() -> Trial {
    let weight = system_weight_newtons();
    let interval = 1.0 / RATE_HZ;
    let flight_samples = (2.0 * TAKEOFF_VELOCITY / GRAVITY * RATE_HZ).round() as usize;
    let net = |change: f64, samples: usize| MASS_KILOGRAMS * change / (samples as f64 * interval);
    let contact = weight + net(TAKEOFF_VELOCITY - ARRIVAL_VELOCITY, CONTACT_SAMPLES);
    let landing = weight + net(TAKEOFF_VELOCITY, LANDING_SAMPLES);

    let mut force = Vec::new();
    force.extend(std::iter::repeat_n(0.0, ON_THE_BOX_SAMPLES));
    let rise_samples = 40;
    for step in 0..rise_samples {
        force.push(contact * (step + 1) as f64 / rise_samples as f64);
    }
    force.extend(std::iter::repeat_n(contact, CONTACT_SAMPLES - rise_samples));
    force.extend(std::iter::repeat_n(0.0, flight_samples));
    force.extend(std::iter::repeat_n(landing, LANDING_SAMPLES));
    force.extend(std::iter::repeat_n(weight, STANDING_SAMPLES));
    Trial::new(force, RATE_HZ).expect("the assembled drop jump is a trial")
}

/// What the correction costs and what it buys, measured rather than described.
///
/// The queue this lane came from held that the drop-jump height was blocked on the arrival
/// being placed, because the integration has to start there. The integration does start there,
/// and the height it produces is the same wherever it started: shifting the start by `d`
/// shifts every sample of the zero-anchored series by the same constant, and the correction is
/// defined as what the standing period reads, so the constant cancels out of the answer. It is
/// the same cancellation that lets the rule run without a stated box height.
///
/// Written down as a guard because it is surprising and because the opposite is the natural
/// assumption. What the arrival decides is whether the rule runs at all, which the refusal
/// below covers, not what it reports.
#[test]
fn the_correction_leaves_the_height_where_the_integration_started_out_of_it() {
    let trial = synthetic_drop_jump_with_a_gradual_arrival();
    let at_threshold = |newtons: f64| {
        let mut request = asking(&trial, LANDING_ABSOLUTE, MCMAHON, &[]);
        request
            .derived
            .get_mut(LANDING_CONSTRUCT)
            .expect("the landing rule is named")
            .parameters
            .insert("threshold_n".to_string(), newtons);
        let response = run(&trial, &request).expect("the analysis runs");
        let placed = response
            .metrics
            .iter()
            .find(|metric| metric.key == "landing_seconds")
            .and_then(|metric| metric.value)
            .expect("the landing rule placed an arrival");
        (placed, height_in(&response).expect("a height"))
    };

    let probes: Vec<(f64, f64)> = [20.0, 500.0, 1000.0, 1500.0, 2000.0]
        .into_iter()
        .map(at_threshold)
        .collect();
    let earliest = probes.first().expect("probes ran");
    let latest = probes.last().expect("probes ran");
    assert!(
        latest.0 - earliest.0 > 0.030,
        "the probes have to move the arrival before they can say anything about the height: \
         {:.4} s to {:.4} s",
        earliest.0,
        latest.0
    );
    for (placed, height) in &probes {
        assert!(
            (height - earliest.1).abs() < 1e-9,
            "an arrival at {placed:.4} s gave {height:.12} m against {:.12} m at {:.4} s",
            earliest.1,
            earliest.0
        );
    }
}

/// The two drop-jump routes on one trace. The box height is 0.30 m and the athlete's centre of
/// mass fell less than that, which is the whole of the registry's 0.066 m bias, so the route
/// that assumes the box and the route that measures the arrival do not agree.
#[test]
fn assuming_the_box_and_recovering_the_arrival_do_not_agree() {
    let trial = synthetic_drop_jump();

    let recovered = height_in(
        &run(&trial, &asking(&trial, LANDING_ABSOLUTE, MCMAHON, &[])).expect("mcmahon runs"),
    )
    .expect("mcmahon produced a height");

    // The box route integrates from the onset it was handed, so the onset is placed at the
    // arrival by hand and the record says a hand placed it. Both routes then run over the same
    // interval and the only thing left between them is the arrival velocity.
    let mut request = asking(
        &trial,
        LANDING_ABSOLUTE,
        DROP_FROM_BOX,
        &[("box_height_m", BOX_HEIGHT_METERS)],
    );
    request.onset.manual_index = Some(ARRIVAL_INDEX);
    let assumed = height_in(&run(&trial, &request).expect("the box route runs"))
        .expect("the box route produced a height");

    assert!(
        recovered > assumed,
        "the box implies a faster arrival than the athlete had, so it should report the smaller \
         height: recovered {recovered:.4} m against assumed {assumed:.4} m"
    );
    let gap = recovered - assumed;
    assert!(
        gap > 0.01 && gap < 0.10,
        "the gap between the two routes is {gap:.4} m, against the 0.066 m the registry records"
    );
}
