//! Binding registry entries to the core, and running one analysis.
//!
//! No quantity is computed here. Every number comes back from `plateforce_core`; this
//! module decides which core function a registry id names, what parameters it was given,
//! and what provenance travels with the answer.

use std::collections::BTreeMap;

use plateforce_core::trial::{
    onset_noise_relative, takeoff_absolute_threshold, takeoff_velocity_meters_per_second,
};
use plateforce_core::{
    jump_height_from_flight_time, jump_height_from_takeoff_velocity, Landmarks, Trial,
    WeighingEpoch,
};
use serde::{Deserialize, Serialize};

/// The decision slots an analysis has to fill. Each draws its candidates from the
/// registry by construct, so the interface never carries a hardcoded method list.
pub const WEIGHING_CONSTRUCT: &str = "system_weight";
pub const ONSET_CONSTRUCT: &str = "movement_onset";
pub const TAKEOFF_CONSTRUCT: &str = "takeoff";

/// Method ids this build can execute. A registry entry absent from this list is offered
/// to the user as unavailable with its reason rather than quietly left off the menu.
pub const EXECUTABLE_METHOD_IDS: &[&str] = &[
    "bwepoch.fixed_window",
    "onset.threshold.noise_relative",
    "takeoff.threshold.absolute",
];

/// Slots the registry does not yet carry an entry for. The analysis still runs, and the
/// provenance says the method is not registry backed so the number is never mistaken for
/// one whose citation was checked.
pub fn fallback_method_id(construct: &str) -> Option<&'static str> {
    match construct {
        WEIGHING_CONSTRUCT => Some("bwepoch.fixed_window"),
        TAKEOFF_CONSTRUCT => Some("takeoff.threshold.absolute"),
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MethodChoice {
    pub method_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    /// Set when the user dragged the marker. An override is a provenance fact, not a
    /// bypass, so it is reported next to the number it changed.
    #[serde(default)]
    pub manual_index: Option<usize>,
}

impl MethodChoice {
    fn parameter(&self, name: &str, fallback: f64) -> f64 {
        self.parameters.get(name).copied().unwrap_or(fallback)
    }

    fn bound(&self) -> Vec<(String, f64)> {
        self.parameters
            .iter()
            .map(|(name, value)| (name.clone(), *value))
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeighingChoice {
    pub method_id: String,
    pub start_index: usize,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisRequest {
    pub weighing: WeighingChoice,
    pub onset: MethodChoice,
    pub takeoff: MethodChoice,
    #[serde(default)]
    pub touchdown_index: Option<usize>,
    /// Registry ids the interface knows about, so provenance can say which of the bound
    /// methods were checked against a citation and which are build defaults.
    #[serde(default)]
    pub registry_backed_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoundMethod {
    pub method_id: String,
    pub bound_parameters: Vec<(String, f64)>,
    pub registry_backed: bool,
    pub manual_override: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub key: &'static str,
    pub label: &'static str,
    pub value: Option<f64>,
    pub unit: &'static str,
    /// Which bound methods this number depends on. The interface joins these back to the
    /// registry for citations, bias and failure rate.
    pub contributing_method_ids: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Levels {
    pub system_weight_newtons: f64,
    pub weighing_standard_deviation_newtons: f64,
    pub onset_band_lower_newtons: Option<f64>,
    pub onset_band_upper_newtons: Option<f64>,
    pub takeoff_threshold_newtons: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisResponse {
    pub weighing_start_index: usize,
    pub weighing_end_index: usize,
    pub onset_index: Option<usize>,
    pub takeoff_index: Option<usize>,
    pub touchdown_index: Option<usize>,
    pub levels: Levels,
    pub bound_methods: Vec<BoundMethod>,
    pub metrics: Vec<Metric>,
    pub warnings: Vec<String>,
}

/// The weighing epoch at an arbitrary start, without a second implementation of the mean
/// and the standard deviation. `WeighingEpoch::fixed_window` anchors at sample zero, so
/// the window is fed a trace that starts where the window does and the indices are
/// restated against the original trace afterwards.
pub fn weighing_epoch_at(
    trial: &Trial,
    start_index: usize,
    duration_seconds: f64,
) -> Result<WeighingEpoch, String> {
    let start_index = start_index.min(trial.len().saturating_sub(2));
    let shifted = Trial::new(trial.force()[start_index..].to_vec(), trial.sample_rate_hz())
        .map_err(|e| e.to_string())?;
    let mut epoch =
        WeighingEpoch::fixed_window(&shifted, duration_seconds).map_err(|e| e.to_string())?;
    epoch.start_index += start_index;
    epoch.end_index += start_index;
    Ok(epoch)
}

fn is_backed(request: &AnalysisRequest, method_id: &str) -> bool {
    request.registry_backed_ids.iter().any(|id| id == method_id)
}

pub fn run(trial: &Trial, request: &AnalysisRequest) -> Result<AnalysisResponse, String> {
    let mut warnings = Vec::new();

    let epoch = weighing_epoch_at(trial, request.weighing.start_index, request.weighing.duration_seconds)?;

    let mut bound_methods = vec![BoundMethod {
        method_id: request.weighing.method_id.clone(),
        bound_parameters: vec![
            ("start_seconds".into(), trial.time_at(epoch.start_index)),
            ("duration_seconds".into(), request.weighing.duration_seconds),
        ],
        registry_backed: is_backed(request, &request.weighing.method_id),
        manual_override: request.weighing.start_index != 0,
    }];

    let k_standard_deviations = request.onset.parameter("k", 5.0);
    let back_offset_seconds = request.onset.parameter("back_offset", 0.030);
    let onset_band = k_standard_deviations * epoch.standard_deviation_newtons;

    let onset_index = match request.onset.manual_index {
        Some(index) => Some(index.min(trial.len() - 1)),
        None => {
            match onset_noise_relative(
                trial,
                &epoch,
                k_standard_deviations,
                back_offset_seconds,
                trial.duration_seconds(),
            ) {
                Ok(index) => Some(index),
                Err(error) => {
                    warnings.push(error.to_string());
                    None
                }
            }
        }
    };
    bound_methods.push(BoundMethod {
        method_id: request.onset.method_id.clone(),
        bound_parameters: request.onset.bound(),
        registry_backed: is_backed(request, &request.onset.method_id),
        manual_override: request.onset.manual_index.is_some(),
    });

    let takeoff_threshold_newtons = request.takeoff.parameter("threshold_newtons", 10.0);
    let minimum_flight_seconds = request.takeoff.parameter("minimum_flight_seconds", 0.100);

    let takeoff_index = match request.takeoff.manual_index {
        Some(index) => Some(index.min(trial.len() - 1)),
        None => match takeoff_absolute_threshold(
            trial,
            takeoff_threshold_newtons,
            minimum_flight_seconds,
            epoch.end_index,
        ) {
            Ok(index) => Some(index),
            Err(error) => {
                warnings.push(error.to_string());
                None
            }
        },
    };
    bound_methods.push(BoundMethod {
        method_id: request.takeoff.method_id.clone(),
        bound_parameters: request.takeoff.bound(),
        registry_backed: is_backed(request, &request.takeoff.method_id),
        manual_override: request.takeoff.manual_index.is_some(),
    });

    // Touchdown is the return above the same threshold that defined takeoff, so it is not
    // an independent choice and it is not offered as one.
    let touchdown_index = request.touchdown_index.or_else(|| {
        takeoff_index.and_then(|takeoff| {
            trial.force()[takeoff..]
                .iter()
                .position(|&force| force > takeoff_threshold_newtons)
                .map(|offset| offset + takeoff)
        })
    });

    if let (Some(onset), Some(takeoff)) = (onset_index, takeoff_index) {
        if onset >= takeoff {
            warnings.push(
                "onset is at or after takeoff, so every interval below is meaningless".into(),
            );
        }
    }

    let mut metrics = Vec::new();
    let weighing_ids = vec![request.weighing.method_id.clone()];
    let onset_ids = vec![request.onset.method_id.clone()];
    let takeoff_ids = vec![request.takeoff.method_id.clone()];
    let interval_ids = vec![
        request.onset.method_id.clone(),
        request.takeoff.method_id.clone(),
    ];

    metrics.push(Metric {
        key: "system_weight_newtons",
        label: "System weight",
        value: Some(epoch.system_weight_newtons),
        unit: "N",
        contributing_method_ids: weighing_ids.clone(),
        note: Some("Includes any external load. System weight is not bodyweight.".into()),
    });
    metrics.push(Metric {
        key: "system_mass_kilograms",
        label: "System mass",
        value: Some(epoch.system_mass_kilograms()),
        unit: "kg",
        contributing_method_ids: weighing_ids,
        note: None,
    });
    metrics.push(Metric {
        key: "onset_time_seconds",
        label: "Movement onset",
        value: onset_index.map(|index| trial.time_at(index)),
        unit: "s",
        contributing_method_ids: onset_ids,
        note: None,
    });
    metrics.push(Metric {
        key: "takeoff_time_seconds",
        label: "Takeoff",
        value: takeoff_index.map(|index| trial.time_at(index)),
        unit: "s",
        contributing_method_ids: takeoff_ids,
        note: None,
    });

    let time_to_takeoff_seconds = match (onset_index, takeoff_index) {
        (Some(onset), Some(takeoff)) if takeoff > onset => {
            Some(trial.time_at(takeoff) - trial.time_at(onset))
        }
        _ => None,
    };
    metrics.push(Metric {
        key: "time_to_takeoff_seconds",
        label: "Time to takeoff",
        value: time_to_takeoff_seconds,
        unit: "s",
        contributing_method_ids: interval_ids.clone(),
        note: Some(
            "Bounded by two threshold crossings, which is why it is the least reproducible number here.".into(),
        ),
    });

    let flight_time_seconds = match (takeoff_index, touchdown_index) {
        (Some(takeoff), Some(touchdown)) if touchdown > takeoff => {
            Some(trial.time_at(touchdown) - trial.time_at(takeoff))
        }
        _ => None,
    };
    metrics.push(Metric {
        key: "flight_time_seconds",
        label: "Flight time",
        value: flight_time_seconds,
        unit: "s",
        contributing_method_ids: takeoff_ids_for_flight(request),
        note: None,
    });

    let landmarks = match (onset_index, takeoff_index) {
        (Some(onset), Some(takeoff)) if takeoff > onset => Some(Landmarks {
            onset_index: onset,
            takeoff_index: takeoff,
            touchdown_index: touchdown_index.unwrap_or(trial.len() - 1),
        }),
        _ => None,
    };

    let takeoff_velocity = landmarks
        .as_ref()
        .map(|marks| takeoff_velocity_meters_per_second(trial, &epoch, marks));
    metrics.push(Metric {
        key: "takeoff_velocity_meters_per_second",
        label: "Takeoff velocity",
        value: takeoff_velocity,
        unit: "m/s",
        contributing_method_ids: interval_ids.clone(),
        note: Some("Net impulse over system mass. An identity, not an estimate.".into()),
    });

    let net_impulse = landmarks.as_ref().map(|marks| {
        let gross = trial.integrate_newton_seconds(marks.onset_index, marks.takeoff_index);
        let spanned = marks
            .takeoff_index
            .saturating_sub(marks.onset_index)
            .saturating_sub(1) as f64;
        gross - epoch.system_weight_newtons * spanned * trial.sample_interval_seconds()
    });
    metrics.push(Metric {
        key: "net_impulse_newton_seconds",
        label: "Net impulse",
        value: net_impulse,
        unit: "N.s",
        contributing_method_ids: interval_ids,
        note: None,
    });

    metrics.push(Metric {
        key: "jump_height_from_takeoff_meters",
        label: "Jump height, takeoff frame",
        value: takeoff_velocity.map(jump_height_from_takeoff_velocity),
        unit: "m",
        contributing_method_ids: vec![
            request.onset.method_id.clone(),
            request.takeoff.method_id.clone(),
            request.weighing.method_id.clone(),
        ],
        note: Some(
            "Rise from the instant of takeoff. Not comparable with the standing frame without a declared correction.".into(),
        ),
    });
    metrics.push(Metric {
        key: "jump_height_from_flight_time_meters",
        label: "Jump height, flight time",
        value: flight_time_seconds.map(jump_height_from_flight_time),
        unit: "m",
        contributing_method_ids: takeoff_ids_for_flight(request),
        note: Some(
            "A different construct from the takeoff frame figure above, not a different way of computing it.".into(),
        ),
    });

    Ok(AnalysisResponse {
        weighing_start_index: epoch.start_index,
        weighing_end_index: epoch.end_index,
        onset_index,
        takeoff_index,
        touchdown_index,
        levels: Levels {
            system_weight_newtons: epoch.system_weight_newtons,
            weighing_standard_deviation_newtons: epoch.standard_deviation_newtons,
            onset_band_lower_newtons: Some(epoch.system_weight_newtons - onset_band),
            onset_band_upper_newtons: Some(epoch.system_weight_newtons + onset_band),
            takeoff_threshold_newtons: Some(takeoff_threshold_newtons),
        },
        bound_methods,
        metrics,
        warnings,
    })
}

/// Flight time and the height derived from it depend on the takeoff rule at both ends of
/// the interval, because touchdown is the same threshold crossed the other way.
fn takeoff_ids_for_flight(request: &AnalysisRequest) -> Vec<String> {
    vec![request.takeoff.method_id.clone()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Trial {
        let mut force = vec![600.0; 1200];
        force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
        force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
        force.extend(std::iter::repeat(0.0).take(600));
        force.extend(std::iter::repeat(1400.0).take(240));
        Trial::new(force, 1200.0).unwrap()
    }

    fn request() -> AnalysisRequest {
        AnalysisRequest {
            weighing: WeighingChoice {
                method_id: "bwepoch.fixed_window".into(),
                start_index: 0,
                duration_seconds: 0.8,
            },
            onset: MethodChoice {
                method_id: "onset.threshold.noise_relative".into(),
                parameters: BTreeMap::new(),
                manual_index: None,
            },
            takeoff: MethodChoice {
                method_id: "takeoff.threshold.absolute".into(),
                parameters: BTreeMap::new(),
                manual_index: None,
            },
            touchdown_index: None,
            registry_backed_ids: vec!["onset.threshold.noise_relative".into()],
        }
    }

    #[test]
    fn a_moved_weighing_window_keeps_the_weight_and_restates_the_indices() {
        let trial = synthetic();
        let anchored = weighing_epoch_at(&trial, 0, 0.5).unwrap();
        let moved = weighing_epoch_at(&trial, 240, 0.5).unwrap();
        assert_eq!(moved.start_index, 240);
        assert_eq!(moved.end_index, 240 + 600);
        assert!((moved.system_weight_newtons - anchored.system_weight_newtons).abs() < 1e-9);
    }

    #[test]
    fn every_metric_names_the_methods_that_produced_it() {
        let response = run(&synthetic(), &request()).unwrap();
        assert!(!response.metrics.is_empty());
        for metric in &response.metrics {
            assert!(
                !metric.contributing_method_ids.is_empty(),
                "{} carries no provenance",
                metric.key
            );
        }
    }

    #[test]
    fn a_method_absent_from_the_registry_is_marked_unbacked_rather_than_hidden() {
        let response = run(&synthetic(), &request()).unwrap();
        let takeoff = response
            .bound_methods
            .iter()
            .find(|m| m.method_id == "takeoff.threshold.absolute")
            .unwrap();
        assert!(!takeoff.registry_backed);
    }

    #[test]
    fn dragging_a_marker_is_recorded_as_an_override() {
        let mut request = request();
        request.onset.manual_index = Some(1100);
        let response = run(&synthetic(), &request).unwrap();
        assert_eq!(response.onset_index, Some(1100));
        let onset = response
            .bound_methods
            .iter()
            .find(|m| m.method_id.starts_with("onset."))
            .unwrap();
        assert!(onset.manual_override);
    }
}
