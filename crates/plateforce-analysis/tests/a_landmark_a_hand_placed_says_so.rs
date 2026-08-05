//! A landmark a hand placed fingerprints apart from a detection, and apart from a hand
//! placement somewhere else.
//!
//! The fingerprint answers one question: did two labs compute this number the same way. A
//! reader who drags the onset marker has supplied a sample no rule produced, so the two runs
//! did not compute it the same way however identical the rules and their values read.
//!
//! Two collisions, and both were live rather than latent. A dragged onset sheds the values its
//! rule would have read, so the record thins rather than changing, and a rule that reads
//! nothing sheds nothing: `onset.threshold.noise_relative` binds no operator and states no value,
//! and its record is byte-identical whether the sample came from the rule or from a hand. A
//! dragged takeoff does not even thin, because the takeoff rule runs under a dragged marker to
//! resolve the threshold touchdown is found against, so its record is the detection's record
//! exactly. And neither carries the sample, so two hands placing two different takeoffs reached
//! one digest.

use std::collections::BTreeMap;

use plateforce_analysis::chain::chain_of;
use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::reporting::fingerprint;
use plateforce_core::{Acquisition, Trial};

const SAMPLE_RATE_HZ: f64 = 1200.0;
const INTERVAL: &str = "time_to_takeoff_seconds";
const FLIGHT: &str = "flight_time_seconds";

/// A countermovement jump that leaves the plate and lands back on it.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, 811));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn request(onset_rule: &str, onset_at: Option<usize>, takeoff_at: Option<usize>) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: onset_rule.into(),
            manual_index: onset_at,
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            manual_index: takeoff_at,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn stamp() -> RegistryStamp {
    RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    }
}

/// A plate whose settings were all recorded. An unfilled block publishes no digest at all and
/// matches nothing including itself, so a comparison over two of them proves nothing either way.
fn a_recorded_plate() -> Acquisition {
    Acquisition {
        filter_at_capture: Some("none".to_string()),
        tare_state: Some("tared_before_trial".to_string()),
        plate_natural_frequency_hz: Some(400.0),
        floor_surface: Some("concrete".to_string()),
        firmware_version: Some("2.4.1".to_string()),
    }
}

fn analysed(request: AnalysisRequest) -> AnalysisResponse {
    run(&a_jump_that_lands(), &request).expect("the trace supports an analysis")
}

fn value(response: &AnalysisResponse, key: &str) -> f64 {
    response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .and_then(|metric| metric.value)
        .unwrap_or_else(|| panic!("{key} carries no number, so there is nothing to compare"))
}

fn digest(response: &AnalysisResponse, key: &str) -> String {
    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .unwrap_or_else(|| panic!("{key} is absent, so there is no chain to fingerprint"));
    let chain = chain_of(response, metric, &stamp(), true);
    fingerprint(&chain.provenance, &a_recorded_plate(), SAMPLE_RATE_HZ)
        .published()
        .expect("the acquisition block is filled, so this digest publishes")
        .to_string()
}

#[test]
fn probe_report_the_collisions() {
    let detected = analysed(request("onset.threshold.noise_relative", None, None));
    let placed_onset = analysed(request("onset.threshold.noise_relative", Some(1180), None));
    let placed_elsewhere = analysed(request("onset.threshold.noise_relative", Some(1120), None));

    println!(
        "onset detected      {INTERVAL}={:.6}  digest={}",
        value(&detected, INTERVAL),
        digest(&detected, INTERVAL)
    );
    println!(
        "onset placed 1180   {INTERVAL}={:.6}  digest={}",
        value(&placed_onset, INTERVAL),
        digest(&placed_onset, INTERVAL)
    );
    println!(
        "onset placed 1120   {INTERVAL}={:.6}  digest={}",
        value(&placed_elsewhere, INTERVAL),
        digest(&placed_elsewhere, INTERVAL)
    );

    let takeoff_detected = analysed(request("onset.threshold.noise_relative", None, None));
    let takeoff_placed = analysed(request("onset.threshold.noise_relative", None, Some(2300)));
    let takeoff_placed_elsewhere =
        analysed(request("onset.threshold.noise_relative", None, Some(2360)));
    println!(
        "takeoff detected    {FLIGHT}={:.6}  digest={}",
        value(&takeoff_detected, FLIGHT),
        digest(&takeoff_detected, FLIGHT)
    );
    println!(
        "takeoff placed 2300 {FLIGHT}={:.6}  digest={}",
        value(&takeoff_placed, FLIGHT),
        digest(&takeoff_placed, FLIGHT)
    );
    println!(
        "takeoff placed 2360 {FLIGHT}={:.6}  digest={}",
        value(&takeoff_placed_elsewhere, FLIGHT),
        digest(&takeoff_placed_elsewhere, FLIGHT)
    );
}

/// The material `fingerprint()` hashes, rebuilt here so a collision can be read rather than
/// inferred from two equal digests.
fn material(response: &AnalysisResponse, key: &str) -> Vec<String> {
    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == key)
        .expect("the quantity is reported");
    let chain = chain_of(response, metric, &stamp(), true);
    let mut lines = Vec::new();
    for (depth, step) in chain.provenance.flattened().iter().enumerate() {
        lines.push(format!(
            "{depth:04} {} source={:?} manual_override={}",
            step.method_id, step.method_source, step.manual_override
        ));
        for parameter in &step.parameters {
            lines.push(format!(
                "{depth:04}   parameter {} = {} {}",
                parameter.name,
                parameter.value,
                parameter.source.wire_name()
            ));
        }
        for choice in &step.choices {
            lines.push(format!(
                "{depth:04}   choice {} = {} {}",
                choice.name,
                choice.value,
                choice.source.wire_name()
            ));
        }
    }
    lines
}

#[test]
fn probe_report_the_material() {
    for (name, left, right, key) in [
        (
            "onset detected vs placed 1180",
            request("onset.threshold.noise_relative", None, None),
            request("onset.threshold.noise_relative", Some(1180), None),
            INTERVAL,
        ),
        (
            "onset placed 1180 vs placed 1120",
            request("onset.threshold.noise_relative", Some(1180), None),
            request("onset.threshold.noise_relative", Some(1120), None),
            INTERVAL,
        ),
        (
            "takeoff detected vs placed 2360",
            request("onset.threshold.noise_relative", None, None),
            request("onset.threshold.noise_relative", None, Some(2360)),
            FLIGHT,
        ),
        (
            "takeoff placed 2300 vs placed 2360",
            request("onset.threshold.noise_relative", None, Some(2300)),
            request("onset.threshold.noise_relative", None, Some(2360)),
            FLIGHT,
        ),
    ] {
        let one = analysed(left);
        let other = analysed(right);
        let (a, b) = (material(&one, key), material(&other, key));
        let differing: Vec<String> = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| x != y)
            .map(|(x, y)| format!("      {x}   |   {y}"))
            .collect();
        println!(
            "== {name}: {} vs {} | material lines {} vs {} | differing {}",
            value(&one, key),
            value(&other, key),
            a.len(),
            b.len(),
            differing.len()
        );
        for line in differing.iter().take(6) {
            println!("{line}");
        }
    }
}
