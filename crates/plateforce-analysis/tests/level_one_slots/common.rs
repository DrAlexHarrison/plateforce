//! What every module here reads: the registry on disk, and trials the rules can run on.

use std::collections::BTreeMap;

use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial, STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED};
use plateforce_registry::{assemble, read_sources, Registry, Source};

const REGISTRY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry");
const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plateforce-conformance/fixtures"
);

/// The founding corpus samples at 1200 Hz. Reading these traces at 1000 corrupts every
/// velocity, displacement, impulse and rate by 20 percent.
pub const CORPUS_SAMPLE_RATE_HZ: f64 = 1200.0;

/// The subject-01 trials committed to this repository, named as their files are.
pub const COMMITTED_TRIALS: &[&str] = &[
    "subject01_trial1",
    "subject01_trial2",
    "subject01_trial3",
    "subject01_trial4",
    "subject01_trial5",
    "subject01_trial6",
];

/// The registry as a directory holds it, which is what the terminal, Python and R load.
/// The browser embeds the same files through the same assembly, so a guard here reads what
/// every surface reads.
pub fn registry() -> Registry {
    let sources: Vec<Source> = read_sources(REGISTRY_ROOT).expect("the registry root is readable");
    assemble(sources.iter().map(Source::pair))
        .expect("the registry assembles")
        .registry
}

pub fn committed_trial(name: &str) -> Trial {
    let path = format!("{FIXTURE_ROOT}/{name}.force.txt");
    let (trial, _) = read_trial_from_path(&path, '\t', 0, CORPUS_SAMPLE_RATE_HZ)
        .unwrap_or_else(|error| panic!("{path} did not read: {error}"));
    trial
}

/// A request naming one rule per shipped slot, so a measurement over the corpus runs the
/// rules the software runs rather than a set assembled for the measurement.
pub fn default_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
    }
}
