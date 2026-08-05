//! The power and work family, on the committed subject-01 trials.
//!
//! Power is force times velocity and work is the integral of power, so every number here rests
//! on the centre-of-mass velocity trace, which rests on the weighing epoch and the onset. That
//! makes this family the one most exposed to landmark choice in the whole registry, and the
//! exposure is what is measured below rather than worked around.
//!
//! What is read here: the size of the disagreement between the three peak rules on one
//! recording, the four intervals a caller picks between and what picking one costs, the two
//! quadrature routes the registry files as one quantity and the vendor product it files as a
//! bias, and the ten regression coefficient sets held equal to the registry that publishes
//! them.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice};

use crate::common::{committed_trial, default_request};

const POWER_CONSTRUCT: &str = "mechanical_power";
const POWER_RULE: &str = "power.instantaneous.force_x_velocity";
const PEAK_CONSTRUCT: &str = "mechanical_power.peak";
const PEAK_KEY: &str = "peak_power_watts";
const MEAN_CONSTRUCT: &str = "mechanical_power.mean";
const MEAN_KEY: &str = "mean_power_watts";
const WORK_CONSTRUCT: &str = "mechanical_work";
const WORK_KEY: &str = "mechanical_work_joules";
const OBJECT_CONSTRUCT: &str = "mechanical_object";
const OBJECT_KEY: &str = "mechanical_object_mass_kilograms";
const DENOMINATOR_CONSTRUCT: &str = "normalisation_basis";
const DENOMINATOR_KEY: &str = "normalisation_denominator_kilograms";

/// The boundary rules every phase name here resolves through.
const WINDOW_CONSTRUCT: &str = "analysis_window";
const WINDOW_RULE: &str = "window_end.takeoff.detected";
const BRAKING_CONSTRUCT: &str = "braking_phase_start";
const BRAKING_RULE: &str = "phase.braking_start.zero_net_force";
const PROPULSION_START_CONSTRUCT: &str = "propulsion_phase_start";
const PROPULSION_START_RULE: &str = "phase.propulsion_start.zero_velocity";
const PROPULSION_END_CONSTRUCT: &str = "propulsion_phase_end";
const PROPULSION_END_RULE: &str = "phase.propulsion_end.peak_com_velocity";

/// The four values the five phase-reading entries publish.
const PHASES: &[&str] = &["braking", "propulsion", "movement", "analysis_window"];

/// A request carrying every boundary rule a phase name can resolve through, plus one rule of
/// this family with what it was asked for.
fn asking(construct: &str, method_id: &str, options: &[(&str, &str)]) -> AnalysisRequest {
    let mut request = default_request();
    for (boundary_construct, rule, stated) in [
        (WINDOW_CONSTRUCT, WINDOW_RULE, None),
        (BRAKING_CONSTRUCT, BRAKING_RULE, None),
        (PROPULSION_START_CONSTRUCT, PROPULSION_START_RULE, None),
        (
            PROPULSION_END_CONSTRUCT,
            PROPULSION_END_RULE,
            Some(("search_signal", "velocity_argmax")),
        ),
    ] {
        request.derived.insert(
            boundary_construct.to_string(),
            MethodChoice {
                method_id: rule.to_string(),
                options: stated
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
                ..Default::default()
            },
        );
    }
    request.derived.insert(
        construct.to_string(),
        MethodChoice {
            method_id: method_id.to_string(),
            options: options
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect(),
            ..Default::default()
        },
    );
    request
}

/// The three names a rule that reads a power series over an interval cannot run without.
fn over(phase: &str) -> Vec<(&'static str, String)> {
    vec![
        ("force_term", "total".to_string()),
        ("sign_convention", "upward_positive".to_string()),
        ("phase", phase.to_string()),
    ]
}

fn stating<'a>(pairs: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    pairs
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect()
}

fn answered(
    trial: &plateforce_core::Trial,
    construct: &str,
    method_id: &str,
    options: &[(&str, &str)],
    key: &str,
) -> Option<f64> {
    let response = run(trial, &asking(construct, method_id, options)).expect("the request runs");
    response.metric(key).and_then(|metric| metric.value)
}

fn responded(
    trial: &plateforce_core::Trial,
    construct: &str,
    method_id: &str,
    options: &[(&str, &str)],
) -> AnalysisResponse {
    run(trial, &asking(construct, method_id, options)).expect("the request runs")
}

/// The sentence a rule that declined wrote, so a guard about refusing names the reason rather
/// than only the absence.
fn refusal_for(response: &AnalysisResponse, method_id: &str) -> Option<String> {
    response
        .refusals
        .iter()
        .find(|rule| rule.method_id == method_id)
        .map(|rule| plateforce_analysis::document::refusal_from_rule(rule).to_string())
}

// -----------------------------------------------------------------------------------------
// The registry is the source of the numbers, and the code is held to it.
// -----------------------------------------------------------------------------------------

/// The ten coefficient sets a rule holds are the ten the registry publishes, coefficient by
/// coefficient and unit by unit.
///
/// A rule cannot read the registry while it runs, so the sets exist twice: as
/// `[[method.parameter.value.number]]` rows and as a table in the rule. Two copies of one
/// table are free to disagree, and the disagreement would be invisible, because every number
/// either copy produces is a plausible peak power. This is what makes the registry the source
/// and the table the transcription: an edit to one and not the other is a failing test.
#[test]
fn the_coefficient_sets_are_the_ones_the_registry_publishes() {
    use plateforce_analysis::slots::power_peak::from_height_regression::COEFFICIENT_SETS;

    let registry = crate::common::registry();
    let entry = registry
        .methods
        .get("power.peak_from_height.regression")
        .expect("the entry is in the shipped registry");
    let parameter = entry
        .parameters
        .iter()
        .find(|parameter| parameter.name == "population")
        .expect("the entry publishes the population parameter");

    assert_eq!(
        parameter.named_values.len(),
        COEFFICIENT_SETS.len(),
        "the registry publishes {} sets and the rule holds {}",
        parameter.named_values.len(),
        COEFFICIENT_SETS.len()
    );

    let mut compared = 0usize;
    for (published, (key, held)) in parameter.named_values.iter().zip(COEFFICIENT_SETS) {
        assert_eq!(&published.key, key, "the sets are in different orders");
        let number = |name: &str| {
            published
                .numbers
                .iter()
                .find(|number| number.name == name)
                .unwrap_or_else(|| panic!("{key} publishes no {name}"))
        };
        let height = number("jump_height_coefficient");
        assert_eq!(height.value, held.jump_height_coefficient, "{key} height");
        assert_eq!(height.unit, held.jump_height_unit, "{key} height unit");
        assert_eq!(
            number("body_mass_coefficient").value,
            held.body_mass_coefficient,
            "{key} mass"
        );
        assert_eq!(
            number("intercept").value,
            held.intercept_watts,
            "{key} intercept"
        );
        compared += 1;
    }
    println!("{compared} of {} coefficient sets compared against the registry, every coefficient and every unit", COEFFICIENT_SETS.len());
    assert_eq!(compared, 10, "ten sets are published and ten were read");
}

/// Every phase name a rule of this family accepts is one its entry publishes, and every name
/// its entry publishes is one the rule accepts.
///
/// Both directions, because each fails differently. A rule accepting a name the registry does
/// not publish offers an interface a reader cannot look up. A registry publishing a name the
/// rule refuses draws a control that produces nothing.
#[test]
fn the_phase_names_a_rule_accepts_are_the_ones_its_entry_publishes() {
    let registry = crate::common::registry();
    let trial = committed_trial("subject01_trial1");
    let mut checked = 0usize;

    for (construct, method_id) in [
        (PEAK_CONSTRUCT, "power.peak.instantaneous"),
        (MEAN_CONSTRUCT, "power.mean.phase"),
        (WORK_CONSTRUCT, "work.integral_power_dt"),
        (WORK_CONSTRUCT, "work.integral_force_ds"),
        (WORK_CONSTRUCT, "work.single_force_displacement_product"),
    ] {
        let entry = registry.methods.get(method_id).expect("the entry loads");
        let parameter = entry
            .parameters
            .iter()
            .find(|parameter| parameter.name == "phase")
            .unwrap_or_else(|| panic!("{method_id} publishes no phase parameter"));
        let published: Vec<&str> = parameter
            .named_values
            .iter()
            .map(|value| value.key.as_str())
            .collect();
        assert_eq!(published, PHASES, "{method_id} publishes {published:?}");

        for phase in PHASES {
            let stated = over(phase);
            let response = responded(&trial, construct, method_id, &stating(&stated));
            assert!(
                refusal_for(&response, method_id).is_none(),
                "{method_id} refused the published phase {phase}: {:?}",
                refusal_for(&response, method_id)
            );
            checked += 1;
        }

        // And a name the entry does not publish is refused by name rather than mapped onto
        // whichever interval is nearest.
        let mut unpublished = over("whole_recording");
        unpublished.retain(|(name, _)| *name != "phase" || true);
        let response = responded(&trial, construct, method_id, &stating(&unpublished));
        let refusal = refusal_for(&response, method_id).unwrap_or_else(|| {
            panic!("{method_id} accepted a phase the registry does not publish")
        });
        assert!(
            refusal.contains("whole_recording"),
            "{method_id} refused without naming the value: {refusal}"
        );
        checked += 1;
    }
    println!("{checked} phase names checked across 5 entries, both directions");
}

// -----------------------------------------------------------------------------------------
// What the choice of interval and the choice of rule cost, on one recording.
// -----------------------------------------------------------------------------------------

/// The interval a caller names moves the mean and the work, and moves the peak wherever the
/// intervals do not share the instant it sits on.
///
/// The registry says the phase is load-bearing and usually omitted, and this is that claim
/// made into a measurement rather than repeated. It comes apart into two findings. The mean
/// moves by more than a thousand watts across the four, because a mean carries its interval
/// and the four intervals have four durations. The peak does not move between three of them,
/// because the largest instantaneous power on this trace sits inside the push and all three of
/// those intervals contain the push, so they select one sample.
///
/// A guard asserting that all four peaks differ would have been asserting something untrue
/// about physics, and it would have passed for any rule wired to ignore the interval. What
/// holds instead is that braking is the interval that does not contain the push, so every
/// quantity read over it differs from every quantity read over the other three.
#[test]
fn the_interval_a_caller_names_moves_the_mean_and_the_work_and_the_peak_where_it_can() {
    let trial = committed_trial("subject01_trial1");
    let mut readings: Vec<(&str, f64, f64, f64)> = Vec::new();

    for phase in PHASES {
        let stated = over(phase);
        let options = stating(&stated);
        let peak = answered(
            &trial,
            PEAK_CONSTRUCT,
            "power.peak.instantaneous",
            &options,
            PEAK_KEY,
        )
        .unwrap_or_else(|| panic!("no peak over {phase}"));
        let mean = answered(
            &trial,
            MEAN_CONSTRUCT,
            "power.mean.phase",
            &options,
            MEAN_KEY,
        )
        .unwrap_or_else(|| panic!("no mean over {phase}"));
        let work = answered(
            &trial,
            WORK_CONSTRUCT,
            "work.integral_power_dt",
            &options,
            WORK_KEY,
        )
        .unwrap_or_else(|| panic!("no work over {phase}"));
        println!("{phase:16} peak {peak:10.1} W   mean {mean:9.1} W   work {work:9.1} J");
        readings.push((phase, peak, mean, work));
    }

    let read = |wanted: &str| {
        readings
            .iter()
            .find(|(phase, _, _, _)| *phase == wanted)
            .copied()
            .expect("all four ran")
    };

    // The mean has no reason to repeat across two intervals of different duration, so it is
    // the quantity read pairwise.
    for (index, (left, _, left_mean, _)) in readings.iter().enumerate() {
        for (right, _, right_mean, _) in readings.iter().skip(index + 1) {
            assert!(
                (left_mean - right_mean).abs() > 1.0,
                "the mean over {left} and over {right} agree to within a watt: {left_mean} \
                 against {right_mean}, so the interval reached no arithmetic"
            );
        }
    }
    let means: Vec<f64> = readings.iter().map(|(_, _, mean, _)| *mean).collect();
    let widest = means.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - means.iter().copied().fold(f64::INFINITY, f64::min);
    println!("the four intervals span {widest:.1} W of mean power on one recording");
    assert!(widest > 1000.0, "the four means span only {widest} W");

    // Braking is the one interval that does not contain the push, which is what separates a
    // phase reaching the arithmetic from a phase riding along beside it.
    let (_, braking_peak, _, braking_work) = read("braking");
    for phase in ["propulsion", "movement", "analysis_window"] {
        let (_, peak, _, work) = read(phase);
        assert!(
            (peak - braking_peak).abs() > 1.0,
            "the peak over braking and over {phase} agree: {braking_peak} against {peak}"
        );
        assert!(
            (work - braking_work).abs() > 1.0,
            "the work over braking and over {phase} agree: {braking_work} against {work}"
        );
    }

    // The movement and the analysis window do the same work and report different means, which
    // is one number over two durations and is what the entry means by a mean that must never
    // be displayed without its interval.
    let (_, _, movement_mean, movement_work) = read("movement");
    let (_, _, window_mean, window_work) = read("analysis_window");
    println!(
        "the movement and the analysis window do the same {movement_work:.1} J and report means \
         of {movement_mean:.1} W and {window_mean:.1} W"
    );
    assert!(
        (movement_work - window_work).abs() < 1e-6,
        "two intervals that both close at takeoff did different work: {movement_work} against \
         {window_work}"
    );
    assert!(
        (movement_mean / window_mean) > 2.0,
        "the two means are within a factor of two: {movement_mean} against {window_mean}"
    );
}

/// The three peak rules disagree, and the two that never form a power series disagree with the
/// one that does.
///
/// The registry's whole case for this construct is that the estimates from height and mass are
/// not the measured peak. Both are labelled peak power and a reader drawing them against each
/// other is drawing two different quantities.
#[test]
fn the_three_peak_rules_disagree_on_one_recording() {
    let trial = committed_trial("subject01_trial1");
    let measured = answered(
        &trial,
        PEAK_CONSTRUCT,
        "power.peak.instantaneous",
        &stating(&over("propulsion")),
        PEAK_KEY,
    )
    .expect("the measured peak answers");
    let lewis = answered(
        &trial,
        PEAK_CONSTRUCT,
        "power.peak_from_height.lewis",
        &[],
        PEAK_KEY,
    )
    .expect("the dimensional estimate answers");

    let mut regressions: Vec<(String, f64)> = Vec::new();
    for population in [
        "harman1991",
        "sayers1999_squat_jump",
        "sayers1999_countermovement",
        "shetty2002",
        "canavan2004",
        "lara2006_male_sport_science",
        "lara2006_female_elite_volleyball",
        "lara2006_female_medium_volleyball",
        "lara2006_female_sport_science",
        "lara2006_female_university",
    ] {
        let watts = answered(
            &trial,
            PEAK_CONSTRUCT,
            "power.peak_from_height.regression",
            &[("population", population)],
            PEAK_KEY,
        )
        .unwrap_or_else(|| panic!("{population} produced nothing"));
        regressions.push((population.to_string(), watts));
    }

    println!("measured peak over the push          {measured:9.1} W");
    println!("power.peak_from_height.lewis         {lewis:9.1} W");
    for (population, watts) in &regressions {
        println!("{population:36} {watts:9.1} W");
    }

    let lowest = regressions
        .iter()
        .fold(f64::INFINITY, |held, (_, watts)| held.min(*watts));
    let highest = regressions
        .iter()
        .fold(f64::NEG_INFINITY, |held, (_, watts)| held.max(*watts));
    println!(
        "ten populations span {:.1} W, {:.1} percent of the lowest",
        highest - lowest,
        (highest - lowest) / lowest * 100.0
    );

    assert!(
        (measured - lewis).abs() > 100.0,
        "the dimensional estimate and the measured peak agree to within 100 W, so one of them \
         is not reading what it says it reads: {measured} against {lewis}"
    );
    assert!(
        highest - lowest > 100.0,
        "the ten populations span {:.1} W, which is not the disagreement the registry records",
        highest - lowest
    );

    // Shetty's coefficient is per metre and the other nine are per centimetre. Read under the
    // wrong unit it would be a hundred times the others, which is the only way this arithmetic
    // can be wrong and the reason the unit is data.
    let shetty = regressions
        .iter()
        .find(|(population, _)| population == "shetty2002")
        .map(|(_, watts)| *watts)
        .expect("shetty is among the ten");
    let others: Vec<f64> = regressions
        .iter()
        .filter(|(population, _)| population != "shetty2002")
        .map(|(_, watts)| *watts)
        .collect();
    let mean_of_others = others.iter().sum::<f64>() / others.len() as f64;
    println!(
        "shetty2002 {shetty:.1} W against a mean of {mean_of_others:.1} W over the other nine"
    );
    assert!(
        (shetty / mean_of_others - 1.0).abs() < 1.0,
        "shetty2002 reads {shetty} W against {mean_of_others} W over the nine per-centimetre \
         sets, which is the factor-of-a-hundred the height unit exists to prevent"
    );
}

/// The two quadrature routes are one integral in this build, and the vendor product is not.
///
/// The registry files the two routes as a naming disagreement, so they have to agree. It also
/// notes that an error in the weighing epoch propagates linearly through one and quadratically
/// through the other, and that is checked here rather than repeated: the displacement this
/// build integrates comes from the same velocity the power series was formed from, so the two
/// routes are one integral and a wrong weight moves both by the same amount. Where a measured
/// displacement signal existed they would be two instruments and could differ.
#[test]
fn the_two_quadrature_routes_are_one_integral_here_and_the_vendor_product_is_not() {
    let trial = committed_trial("subject01_trial1");
    let stated = over("propulsion");
    let options = stating(&stated);

    let by_power = answered(
        &trial,
        WORK_CONSTRUCT,
        "work.integral_power_dt",
        &options,
        WORK_KEY,
    )
    .expect("the power-time route answers");
    let by_force = answered(
        &trial,
        WORK_CONSTRUCT,
        "work.integral_force_ds",
        &options,
        WORK_KEY,
    )
    .expect("the force-displacement route answers");
    let vendor = answered(
        &trial,
        WORK_CONSTRUCT,
        "work.single_force_displacement_product",
        &[("phase", "propulsion")],
        WORK_KEY,
    )
    .expect("the vendor product answers");

    println!("work.integral_power_dt                  {by_power:9.4} J");
    println!("work.integral_force_ds                  {by_force:9.4} J");
    println!("work.single_force_displacement_product  {vendor:9.4} J");
    println!(
        "the vendor product is {:.1} percent of the integral",
        vendor / by_power * 100.0
    );

    assert_eq!(
        by_power, by_force,
        "the registry files these two as one quantity by two names and this build gave two \
         numbers"
    );
    assert!(
        (vendor - by_power).abs() / by_power.abs() > 0.1,
        "the vendor product came to {vendor} J against {by_power} J, within a tenth, so the \
         bias the registry records against it is not in this build's arithmetic"
    );

    // The weighing epoch is the input the registry's note is about, so it is moved and both
    // routes are read again. A shorter window over a trace that is not perfectly still gives a
    // different system weight, which is the error the note describes.
    let mut request = asking(WORK_CONSTRUCT, "work.integral_power_dt", &options);
    request
        .weighing
        .parameters
        .insert("duration".to_string(), 0.3);
    let moved_by_power = run(&trial, &request)
        .expect("the request runs")
        .metric(WORK_KEY)
        .and_then(|metric| metric.value)
        .expect("the power-time route answers");
    let mut request = asking(WORK_CONSTRUCT, "work.integral_force_ds", &options);
    request
        .weighing
        .parameters
        .insert("duration".to_string(), 0.3);
    let moved_by_force = run(&trial, &request)
        .expect("the request runs")
        .metric(WORK_KEY)
        .and_then(|metric| metric.value)
        .expect("the force-displacement route answers");

    println!(
        "a 0.3 s weighing window moves the integral from {by_power:.4} J to {moved_by_power:.4} J, \
         and both routes by the same {:.4} J",
        moved_by_power - by_power
    );
    assert_ne!(
        moved_by_power, by_power,
        "the weighing window moved nothing, so this reading says nothing about either route"
    );
    assert_eq!(
        moved_by_power, moved_by_force,
        "one route moved further than the other under a changed weighing epoch"
    );
}

/// Both choices behind the power series reach every number read off it.
///
/// The two force terms differ by system weight times velocity and the sign convention reverses
/// the series, so a rule that recorded either and passed neither would report one number under
/// both and the record would show a reader who had chosen.
#[test]
fn the_force_term_and_the_sign_reach_every_number_read_off_the_series() {
    let trial = committed_trial("subject01_trial1");
    let mut checked = 0usize;

    for (construct, method_id, key) in [
        (PEAK_CONSTRUCT, "power.peak.instantaneous", PEAK_KEY),
        (MEAN_CONSTRUCT, "power.mean.phase", MEAN_KEY),
        (WORK_CONSTRUCT, "work.integral_power_dt", WORK_KEY),
    ] {
        let total = answered(
            &trial,
            construct,
            method_id,
            &[
                ("force_term", "total"),
                ("sign_convention", "upward_positive"),
                ("phase", "propulsion"),
            ],
            key,
        )
        .expect("the total term answers");
        let net = answered(
            &trial,
            construct,
            method_id,
            &[
                ("force_term", "net"),
                ("sign_convention", "upward_positive"),
                ("phase", "propulsion"),
            ],
            key,
        )
        .expect("the net term answers");
        let downward = answered(
            &trial,
            construct,
            method_id,
            &[
                ("force_term", "total"),
                ("sign_convention", "downward_positive"),
                ("phase", "propulsion"),
            ],
            key,
        )
        .expect("the reversed sign answers");
        println!("{method_id:40} total {total:10.2}  net {net:10.2}  reversed {downward:10.2}");
        assert_ne!(
            total, net,
            "{method_id}: the force term reached no arithmetic"
        );
        // Reversing the sign of every sample reverses a peak into a trough rather than
        // negating the peak, so what is asserted is that it moved, not that it negated.
        assert_ne!(
            total, downward,
            "{method_id}: the sign convention reached no arithmetic"
        );
        checked += 1;
    }

    // And an unstated one is refused by name rather than run under a value nobody chose.
    for (construct, method_id) in [
        (POWER_CONSTRUCT, POWER_RULE),
        (PEAK_CONSTRUCT, "power.peak.instantaneous"),
        (MEAN_CONSTRUCT, "power.mean.phase"),
        (WORK_CONSTRUCT, "work.integral_power_dt"),
    ] {
        let response = responded(&trial, construct, method_id, &[]);
        let refusal = refusal_for(&response, method_id)
            .unwrap_or_else(|| panic!("{method_id} ran with nothing stated"));
        assert!(
            refusal.contains("force_term"),
            "{method_id} declined without naming what it needed: {refusal}"
        );
        println!("{method_id:40} unstated: {refusal}");
        checked += 1;
    }
    println!("{checked} readings taken across 4 rules");
}

/// A number read off a power series names what power meant and what bounded the interval.
///
/// Three things have to be in the chain and none of them placed a landmark this rule read by
/// name: the entry that owns the force term and the sign, and the four integration entries the
/// velocity was integrated under. The phase rules are the fourth, and they arrive through the
/// samples the rule asked for.
#[test]
fn a_power_number_names_what_power_meant_and_which_rules_bounded_it() {
    let trial = committed_trial("subject01_trial1");
    let response = responded(
        &trial,
        PEAK_CONSTRUCT,
        "power.peak.instantaneous",
        &stating(&over("propulsion")),
    );
    let metric = response.metric(PEAK_KEY).expect("the peak answered");
    let behind: Vec<&str> = metric
        .contributing_method_ids
        .iter()
        .map(String::as_str)
        .collect();
    println!("{PEAK_KEY} rests on {behind:?}");

    for expected in [
        POWER_RULE,
        PROPULSION_START_RULE,
        PROPULSION_END_RULE,
        "onset.threshold.noise_relative",
        "bwepoch.fixed_window",
    ] {
        assert!(
            behind.contains(&expected),
            "the chain behind the peak does not name {expected}: {behind:?}"
        );
    }
    // The four integration entries, which are choices inside the arithmetic rather than rules
    // that placed a sample, so nothing else would put them in the chain.
    let integration = behind
        .iter()
        .filter(|id| id.starts_with("integration."))
        .count();
    assert_eq!(
        integration, 4,
        "the chain names {integration} integration entries and the velocity was read under 4: \
         {behind:?}"
    );

    // A peak over a different phase names different rules, so the chain reports the interval
    // that produced this number rather than every rule that ran.
    let over_window = responded(
        &trial,
        PEAK_CONSTRUCT,
        "power.peak.instantaneous",
        &stating(&over("analysis_window")),
    );
    let window_behind: Vec<String> = over_window
        .metric(PEAK_KEY)
        .expect("the peak answered")
        .contributing_method_ids
        .clone();
    println!("over the analysis window it rests on {window_behind:?}");
    assert!(
        !window_behind.iter().any(|id| id == PROPULSION_END_RULE),
        "a peak over the analysis window named the rule that ended the push: {window_behind:?}"
    );
    assert!(
        window_behind.iter().any(|id| id == WINDOW_RULE),
        "a peak over the analysis window did not name the rule that placed it: {window_behind:?}"
    );
}

// -----------------------------------------------------------------------------------------
// The object a number describes, and the mass it is divided by.
// -----------------------------------------------------------------------------------------

/// The two entries that name a mass resolve the same three names to the same three masses, and
/// the three add up.
///
/// One function serves both, so a record cannot declare a quantity to describe the bar and
/// divide it by the athlete. The addition is the check that the function is right rather than
/// merely single: bar plus body is what the plate weighed.
#[test]
fn the_object_a_number_describes_and_the_mass_it_is_divided_by_are_one_answer() {
    let mut trial_request = default_request();
    trial_request.body_mass_kilograms = Some(52.0);
    let trial = committed_trial("subject01_trial1");

    let mass_for = |construct: &str, method_id: &str, name: &str, value: &str, key: &str| {
        let mut request = asking(construct, method_id, &[(name, value)]);
        request.body_mass_kilograms = Some(52.0);
        run(&trial, &request)
            .expect("the request runs")
            .metric(key)
            .and_then(|metric| metric.value)
    };

    let mut masses = Vec::new();
    for (object, denominator) in [
        ("barbell", "barbell_mass"),
        ("body", "body_mass"),
        ("system", "system_mass"),
    ] {
        let declared = mass_for(
            OBJECT_CONSTRUCT,
            "declaration.computed_on_object",
            "object",
            object,
            OBJECT_KEY,
        )
        .unwrap_or_else(|| panic!("no mass for {object}"));
        let divisor = mass_for(
            DENOMINATOR_CONSTRUCT,
            "normalise.denominator",
            "denominator",
            denominator,
            DENOMINATOR_KEY,
        )
        .unwrap_or_else(|| panic!("no divisor for {denominator}"));
        println!("{object:8} {declared:8.4} kg   {denominator:12} {divisor:8.4} kg");
        assert_eq!(
            declared, divisor,
            "{object} and {denominator} are the same mass and came back different"
        );
        masses.push((object, declared));
    }

    let of = |wanted: &str| {
        masses
            .iter()
            .find(|(object, _)| *object == wanted)
            .map(|(_, mass)| *mass)
            .expect("all three ran")
    };
    let gap = (of("barbell") + of("body") - of("system")).abs();
    println!(
        "bar {:.4} + body {:.4} = {:.4} kg against the weighed {:.4} kg",
        of("barbell"),
        of("body"),
        of("barbell") + of("body"),
        of("system")
    );
    assert!(
        gap < 1e-9,
        "the bar and the athlete came to {gap} kg more than the plate weighed"
    );

    // The athlete's own mass is not the weighed system, so a rule that needs it and was not
    // given it refuses by name rather than dividing by the other one.
    let response = responded(
        &trial,
        DENOMINATOR_CONSTRUCT,
        "normalise.denominator",
        &[("denominator", "body_mass")],
    );
    let refusal =
        refusal_for(&response, "normalise.denominator").expect("an unstated body mass is refused");
    println!("unstated: {refusal}");
    assert!(
        refusal.contains("body_mass_kilograms"),
        "the refusal does not name what it needed: {refusal}"
    );

    // A stated mass heavier than the plate weighed leaves a bar of negative mass, which is
    // refused rather than reported as a measurement.
    let mut request = asking(
        OBJECT_CONSTRUCT,
        "declaration.computed_on_object",
        &[("object", "barbell")],
    );
    request.body_mass_kilograms = Some(500.0);
    let response = run(&trial, &request).expect("the request runs");
    let refusal = refusal_for(&response, "declaration.computed_on_object")
        .expect("a body mass above the weighed system is refused");
    println!("500 kg athlete: {refusal}");
    assert!(
        refusal.contains("500"),
        "the refusal does not name the value: {refusal}"
    );
}

/// What the family reads on every committed trial, so the numbers in the report are the
/// software's rather than one trial's.
///
/// The three rules that read the power series answer on all six. The dimensional estimate
/// answers on the trials whose recording holds a landing, and that is a fact about the corpus
/// rather than about the rule: it needs a flight time, a flight time needs the return to the
/// plate, and the corpus was trimmed. The count is reported with its denominator rather than
/// rounded to a pass, and the estimate is asserted to answer exactly where a landing was
/// placed, which is the claim that it declines for want of the recording rather than for want
/// of a rule.
#[test]
fn the_family_answers_on_every_committed_trial() {
    let stated = over("propulsion");
    let options = stating(&stated);
    let mut from_the_series = 0usize;
    let mut series_attempted = 0usize;
    let mut trials_carrying_a_landing = 0usize;

    for name in crate::common::COMMITTED_TRIALS {
        let trial = committed_trial(name);
        let response = responded(&trial, PEAK_CONSTRUCT, "power.peak.instantaneous", &options);
        let peak = response.metric(PEAK_KEY).and_then(|metric| metric.value);
        let mean = answered(
            &trial,
            MEAN_CONSTRUCT,
            "power.mean.phase",
            &options,
            MEAN_KEY,
        );
        let work = answered(
            &trial,
            WORK_CONSTRUCT,
            "work.integral_power_dt",
            &options,
            WORK_KEY,
        );
        let lewis = answered(
            &trial,
            PEAK_CONSTRUCT,
            "power.peak_from_height.lewis",
            &[],
            PEAK_KEY,
        );
        let landed = response.touchdown_index.is_some();
        trials_carrying_a_landing += usize::from(landed);
        let shown = |value: Option<f64>| {
            value
                .map(|number| format!("{number:.1}"))
                .unwrap_or_else(|| "none".to_string())
        };
        println!(
            "{name:20} samples {:5}  takeoff {:>6}  landing {:>6}  peak {:>8}  mean {:>8}  \
             work {:>7}  lewis {:>8}",
            trial.len(),
            response
                .takeoff_index
                .map(|index| index.to_string())
                .unwrap_or_else(|| "none".to_string()),
            response
                .touchdown_index
                .map(|index| index.to_string())
                .unwrap_or_else(|| "none".to_string()),
            shown(peak),
            shown(mean),
            shown(work),
            shown(lewis),
        );
        for reading in [peak, mean, work] {
            series_attempted += 1;
            from_the_series += usize::from(reading.is_some());
        }
        assert_eq!(
            lewis.is_some(),
            landed,
            "{name}: the dimensional estimate and the recording's landing disagree"
        );
    }

    println!(
        "{from_the_series} of {series_attempted} readings off the power series answered, and \
         {trials_carrying_a_landing} of {} committed trials carry a landing",
        crate::common::COMMITTED_TRIALS.len()
    );
    assert_eq!(
        from_the_series, series_attempted,
        "a rule reading the power series declined on a committed trial"
    );

    // Peak power on a countermovement jump runs into the thousands of watts for an athlete of
    // this mass, and the propulsion phase is a fraction of a second, so the work over it is
    // hundreds of joules. A rule off by the sampling rate, by gravity, or by a factor of a
    // thousand would still pass every assertion above.
    let trial = committed_trial("subject01_trial1");
    let peak = answered(
        &trial,
        PEAK_CONSTRUCT,
        "power.peak.instantaneous",
        &options,
        PEAK_KEY,
    )
    .expect("the peak answered");
    let work = answered(
        &trial,
        WORK_CONSTRUCT,
        "work.integral_power_dt",
        &options,
        WORK_KEY,
    )
    .expect("the work answered");
    assert!(
        (1000.0..10000.0).contains(&peak),
        "peak power came to {peak} W, outside the thousands the literature reports for a \
         countermovement jump"
    );
    assert!(
        (50.0..1000.0).contains(&work),
        "the work over the push came to {work} J, outside the hundreds a jump of this height by \
         an athlete of this mass costs"
    );
}

/// The request every reading above was taken under, printed once so the numbers in the report
/// can be reproduced without reading the code.
#[test]
fn the_request_behind_every_reading_here() {
    let request: BTreeMap<String, String> = asking(
        PEAK_CONSTRUCT,
        "power.peak.instantaneous",
        &stating(&over("propulsion")),
    )
    .derived
    .iter()
    .map(|(construct, choice)| (construct.clone(), choice.method_id.clone()))
    .collect();
    for (construct, method_id) in &request {
        println!("{construct:28} {method_id}");
    }
    assert!(request.len() >= 5, "{request:?}");
}
