//! What a propulsion subdivision has to be: an instant strictly inside the interval it
//! divides.
//!
//! A split equal to either boundary divides the interval into all of it and none of it, so a
//! sub-phase metric taken against it is either the whole phase or nothing while the key still
//! reads `propulsion_subdivision_seconds`. The number is not the quantity its name claims and
//! nothing on the result says so, which is the failure this platform exists to end.
//!
//! Both recordings are run, and `by_time` runs beside `by_force_crossing` as the control. The
//! control splits the same interval through the same request construction, so it comes back
//! empty for the same reasons the rule under test would.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

const START: &str = "propulsion_phase_start_seconds";
const END: &str = "propulsion_phase_end_seconds";
const SPLIT: &str = "propulsion_subdivision_seconds";

const BY_FORCE_CROSSING: &str = "phase.propulsion_subdivision.by_force_crossing";
const BY_TIME: &str = "phase.propulsion_subdivision.by_time";

/// A countermovement jump with a landing, matching the trace the pipeline suite reads.
fn synthetic() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(1400.0, 240));
    Trial::new(force, 1200.0).unwrap()
}

fn subject01_trial1() -> Trial {
    let (trial, _) = plateforce_core::read::read_trial_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
        ),
        '\t',
        0,
        1200.0,
    )
    .expect("the committed trial reads");
    trial
}

fn recordings() -> Vec<(&'static str, Trial)> {
    vec![
        ("the synthetic trace", synthetic()),
        ("subject 01 trial 1", subject01_trial1()),
    ]
}

/// A request reaching the subdivision: one rule for every construct the subdivision reads,
/// with the propulsion-end signal stated because that entry publishes no default.
fn request(subdivision_id: &str, propulsion_end_id: &str, search_signal: &str) -> AnalysisRequest {
    let mut derived = BTreeMap::new();
    derived.insert(
        "braking_phase_start".to_string(),
        MethodChoice {
            method_id: "phase.braking_start.zero_net_force".into(),
            ..Default::default()
        },
    );
    derived.insert(
        "propulsion_phase_start".to_string(),
        MethodChoice {
            method_id: "phase.propulsion_start.zero_velocity".into(),
            ..Default::default()
        },
    );
    derived.insert(
        "propulsion_phase_end".to_string(),
        MethodChoice {
            method_id: propulsion_end_id.into(),
            options: BTreeMap::from([("search_signal".to_string(), search_signal.to_string())]),
            ..Default::default()
        },
    );
    derived.insert(
        "propulsion_subdivision".to_string(),
        MethodChoice {
            method_id: subdivision_id.into(),
            ..Default::default()
        },
    );
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
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
        derived,
        ..Default::default()
    }
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
}

fn refusal_of(response: &AnalysisResponse, method_id: &str) -> Option<String> {
    response
        .refusals
        .iter()
        .find(|rule| rule.method_id == method_id)
        .map(|rule| rule.refusal.to_string())
}

/// What one combination of recording, subdivision rule and propulsion-end signal produced.
struct Placement {
    start: Option<f64>,
    end: Option<f64>,
    split: Option<f64>,
    refusal: Option<String>,
}

fn placement(trial: &Trial, subdivision_id: &str, search_signal: &str) -> Placement {
    let response = run(
        trial,
        &request(
            subdivision_id,
            "phase.propulsion_end.peak_com_velocity",
            search_signal,
        ),
    )
    .expect("the request is well formed");
    Placement {
        start: value(&response, START),
        end: value(&response, END),
        split: value(&response, SPLIT),
        refusal: refusal_of(&response, subdivision_id),
    }
}

/// Every combination printed, so a reader sees the rule under test and its control against the
/// same interval rather than a verdict about one of them.
fn read_every_combination() -> Vec<(String, Placement)> {
    let mut read = Vec::new();
    for (recording, trial) in recordings() {
        for signal in ["velocity_argmax", "force_bw_crossing"] {
            for rule in [BY_FORCE_CROSSING, BY_TIME] {
                let short = rule.rsplit('.').next().unwrap_or(rule);
                read.push((
                    format!("{short} / {signal} / {recording}"),
                    placement(&trial, rule, signal),
                ));
            }
        }
    }
    read
}

/// A split equal to either boundary of the interval it divides is refused rather than
/// published.
///
/// Stated over both recordings and both signals, so the reading cannot rest on the one
/// combination that happens to fire. The control's presence is what makes an empty result
/// readable: `by_time` places a split strictly inside on every combination, so a rule
/// refusing on all of them is refusing something, not failing to be reached.
#[test]
fn no_subdivision_lands_on_a_boundary_of_the_interval_it_divides() {
    let read = read_every_combination();
    let mut inside = 0usize;
    let mut degenerate: Vec<String> = Vec::new();
    for (label, placement) in &read {
        let Placement {
            start,
            end,
            split,
            refusal,
        } = placement;
        if let (Some(start), Some(end), Some(split)) = (start, end, split) {
            let on_a_boundary = split <= start || split >= end;
            println!(
                "{label}: start={start:.4} end={end:.4} split={split:.4}{}",
                if on_a_boundary { " AT A BOUNDARY" } else { "" }
            );
            if on_a_boundary {
                degenerate.push(format!(
                    "{label} split at {split:.4} s against an interval of {start:.4} s to \
                     {end:.4} s"
                ));
            } else {
                inside += 1;
            }
        } else {
            println!(
                "{label}: start={} end={} no split, {}",
                start.map(|value| format!("{value:.4}")).unwrap_or_default(),
                end.map(|value| format!("{value:.4}")).unwrap_or_default(),
                refusal.as_deref().unwrap_or("no refusal either")
            );
        }
    }
    println!(
        "{inside} of {} combinations placed a split strictly inside, {} on a boundary",
        read.len(),
        degenerate.len()
    );

    // The control's own line, so a run of all-refusals cannot read as this guard holding.
    let control_inside = read
        .iter()
        .filter(|(label, placement)| {
            label.starts_with("by_time")
                && matches!(
                    (placement.start, placement.end, placement.split),
                    (Some(start), Some(end), Some(split)) if split > start && split < end
                )
        })
        .count();
    assert_eq!(
        control_inside, 4,
        "the control placed {control_inside} splits strictly inside rather than 4, so this run \
         says nothing about the rule under test"
    );

    assert!(
        degenerate.is_empty(),
        "a subdivision was published on a boundary of the interval it divides, which divides \
         that interval into all of it and none of it: {}",
        degenerate.join("; ")
    );
}
