//! A sample carrying no number cannot become a plausible peak or level crossing.

use std::collections::BTreeMap;

use plateforce_analysis::{run, AnalysisRequest, AnalysisResponse, MethodChoice, WeighingChoice};
use plateforce_core::{RefusalCode, Trial};

mod common;

const SAMPLE_RATE_HZ: f64 = 1200.0;
const PEAK_KEY: &str = "peak_force_newtons";
const RATE_KEY: &str = "rate_of_force_development_newtons_per_second";

fn ramp(missing: Option<(usize, f64)>) -> Trial {
    let mut force = vec![600.0; 3000];
    for (offset, sample) in force.iter_mut().enumerate().take(2401).skip(1200) {
        *sample = 600.0 + 2000.0 * (offset - 1200) as f64 / SAMPLE_RATE_HZ;
    }
    if let Some((index, value)) = missing {
        force[index] = value;
    }
    Trial::new(force, SAMPLE_RATE_HZ).expect("the trace is a trial")
}

fn request(construct: &str, method_id: &str) -> AnalysisRequest {
    let mut derived = BTreeMap::from([(
        "analysis_window".to_string(),
        MethodChoice {
            method_id: "window.stated.by_caller".to_string(),
            parameters: BTreeMap::from([
                ("start_seconds".to_string(), 1.0),
                ("end_seconds".to_string(), 2.0),
            ]),
            ..Default::default()
        },
    )]);
    let mut choice = MethodChoice {
        method_id: method_id.to_string(),
        ..Default::default()
    };
    if method_id == "rfd.between_force_levels" {
        choice.parameters = BTreeMap::from([
            ("lower_level".to_string(), 700.0),
            ("upper_level".to_string(), 900.0),
        ]);
        choice
            .options
            .insert("reference_basis".to_string(), "absolute".to_string());
    }
    derived.insert(construct.to_string(), choice);

    common::prepared(AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".to_string(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".to_string(),
            manual_index: Some(1200),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".to_string(),
            manual_index: Some(2400),
            ..Default::default()
        },
        derived,
        ..Default::default()
    })
}

fn value(response: &AnalysisResponse, key: &str) -> Option<f64> {
    response.metric(key).and_then(|metric| metric.value)
}

fn missing_refusal(response: &AnalysisResponse, method_id: &str) -> plateforce_core::Refusal {
    let declined = response
        .refusals
        .iter()
        .find(|refusal| refusal.method_id == method_id)
        .unwrap_or_else(|| panic!("{method_id} left no refusal in {:?}", response.refusals));
    plateforce_analysis::document::refusal_from_rule(declined)
}

#[test]
fn a_non_finite_sample_in_a_peak_window_refuses_the_peak() {
    let request = request("peak_force", "force.peak.gross");
    let clean = run(&ramp(None), &request).expect("the clean request runs");
    assert_eq!(value(&clean, PEAK_KEY), Some(2600.0));

    for (index, carried) in [(1300usize, f64::NAN), (2300, f64::INFINITY)] {
        let response = run(&ramp(Some((index, carried))), &request)
            .expect("the interrupted request returns a document");
        assert_eq!(value(&response, PEAK_KEY), None, "sample {index}");
        assert_eq!(response.samples_carrying_no_number, 1, "1 of 3000 samples");
        let refusal = missing_refusal(&response, "force.peak.gross");
        assert_eq!(refusal.code, RefusalCode::TraceTooShort);
        assert_eq!(refusal.detail["samples_carrying_no_number"], 1.0);
        assert_eq!(refusal.detail["samples_read"], 1201.0);
        assert_eq!(
            refusal.detail["first_sample_carrying_no_number"],
            index as f64
        );
    }
}

#[test]
fn a_non_finite_sample_in_a_level_search_refuses_the_rate() {
    let request = request("rate_of_force_development", "rfd.between_force_levels");
    let clean = run(&ramp(None), &request).expect("the clean request runs");
    let clean_rate = value(&clean, RATE_KEY).expect("the clean ramp has a rate");
    assert!((clean_rate - 2000.0).abs() < 1e-9, "{clean_rate}");

    for (index, carried) in [(1250usize, f64::NAN), (1320, f64::NEG_INFINITY)] {
        let response = run(&ramp(Some((index, carried))), &request)
            .expect("the interrupted request returns a document");
        assert_eq!(value(&response, RATE_KEY), None, "sample {index}");
        assert_eq!(response.samples_carrying_no_number, 1, "1 of 3000 samples");
        let refusal = missing_refusal(&response, "rfd.between_force_levels");
        assert_eq!(refusal.code, RefusalCode::TraceTooShort);
        assert_eq!(refusal.detail["samples_carrying_no_number"], 1.0);
        assert_eq!(refusal.detail["samples_read"], 1201.0);
        assert_eq!(
            refusal.detail["first_sample_carrying_no_number"],
            index as f64
        );
    }
}
