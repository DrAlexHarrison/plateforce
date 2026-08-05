//! What every batch test needs: a registry, a bound request, and the committed fixtures.

// Each test binary links this and uses part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_batch::{BatchRequest, SourceFormat, TrialIdentity};
use plateforce_registry::Registry;

pub const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures"
);

pub fn registry() -> Registry {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the committed registry loads")
}

/// The committed traces are one bare value per line at 1200 Hz, which the 2011 corpus is not,
/// so the column index is declared per run rather than defaulted.
pub fn committed_format() -> SourceFormat {
    SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["force.txt".to_string()],
        sentinel: None,
    }
}

pub fn synthetic_format() -> SourceFormat {
    SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["txt".to_string()],
        sentinel: None,
    }
}

pub fn declared_pattern() -> TrialIdentity {
    TrialIdentity::DeclaredPattern {
        template: "AT{subject}_{trial}".to_string(),
    }
}

/// A request naming a rule for every construct on the path, with the choices recorded as
/// deliberate. Every test that expects numbers rather than a refusal starts here.
pub fn bound_request() -> BatchRequest {
    BatchRequest::new(analysis_request(1.0)).resolving(&[
        "system_weight",
        "movement_onset",
        "takeoff",
    ])
}

/// A capture somebody recorded, for the guards that compare one run's fingerprint against
/// another's.
///
/// A run whose acquisition block is unfilled publishes no fingerprint, because a result whose
/// plate settings nobody recorded cannot be declared to match another. So a guard asserting
/// that two runs fingerprint differently has to be given two runs that fingerprint at all;
/// over unfilled blocks it would be comparing two absences and proving nothing about the
/// digests it was written to separate.
pub fn a_recorded_plate() -> plateforce_core::Acquisition {
    plateforce_core::Acquisition {
        filter_at_capture: Some("none".to_string()),
        tare_state: Some("tared_before_trial".to_string()),
        plate_natural_frequency_hz: Some(400.0),
        floor_surface: Some("concrete".to_string()),
        firmware_version: Some("2.4.1".to_string()),
    }
}

pub fn bound_request_describing_the_plate() -> BatchRequest {
    bound_request().describing(a_recorded_plate())
}

pub fn analysis_request(weighing_duration_seconds: f64) -> AnalysisRequest {
    let mut request = AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".to_string(),
            parameters: BTreeMap::from([("duration".to_string(), weighing_duration_seconds)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".to_string(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".to_string(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: 9.80665,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    };
    // What every surface does before running: the rules read the registry's declared
    // defaults rather than copies of them, and a request that skipped this reads nothing
    // and refuses.
    request.reading(&registry());
    request
}

/// A directory of its own, so a denominator is what the test put there rather than what the
/// repository happens to hold.
///
/// The system temporary directory is shared by every checkout on the machine, so the name
/// alone is not its own: two suites running at once walk and wipe each other's corpora, and
/// the failure reads as a wrong count rather than as interference.
pub fn tempdir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "plateforce-batch-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

pub fn copy_committed_fixtures(into: &Path) -> usize {
    let mut copied = 0;
    for entry in std::fs::read_dir(FIXTURES).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        if name.ends_with("force.txt") {
            std::fs::copy(&path, into.join(&name)).unwrap();
            copied += 1;
        }
    }
    copied
}
