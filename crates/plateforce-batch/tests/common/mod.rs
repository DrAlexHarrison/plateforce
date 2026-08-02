//! What every batch test needs: a registry, a bound request, and the committed fixtures.

// Each test binary links this and uses part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    }
}

pub fn synthetic_format() -> SourceFormat {
    SourceFormat {
        delimiter: '\t',
        force_column_index: 0,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["txt".to_string()],
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

pub fn analysis_request(weighing_duration_seconds: f64) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".to_string(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), weighing_duration_seconds)]),
            options: BTreeMap::new(),
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".to_string(),
            parameters: BTreeMap::from([("k".to_string(), 5.0)]),
            options: BTreeMap::new(),
            manual_index: None,
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".to_string(),
            parameters: BTreeMap::from([("threshold_n".to_string(), 20.0)]),
            options: BTreeMap::new(),
            manual_index: None,
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: 9.80665,
        registry_backed_ids: Vec::new(),
    }
}

/// A directory of its own, so a denominator is what the test put there rather than what the
/// repository happens to hold.
pub fn tempdir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("plateforce-batch-{name}"));
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
