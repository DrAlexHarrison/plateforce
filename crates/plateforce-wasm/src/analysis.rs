//! Binding registry entries to the core, and running one analysis.
//!
//! No quantity is computed here. Every number comes back from `plateforce_core`; this
//! module decides which core function a registry id names, what parameters it was given,
//! and what provenance travels with the answer.
//!
//! Where an id below is absent from the registry it is still offered, flagged as not
//! registry backed. Hiding an executable rule because its paperwork is unfinished is the
//! silent exclusion this project exists to document.

use std::collections::BTreeMap;

use plateforce_core::onset::{
    backtrack, onset_adaptive_trailing_window, onset_last_sample_within_noise_band,
    onset_noise_relative, onset_relative_to_reference, sustained_excursion, BandSides,
    CrossingSearch, CrossingSelection, DegenerateBandPolicy, ExcursionBand, PostCrossingRule,
};
use plateforce_core::takeoff::{
    takeoff_descending_crossing, takeoff_first_sustained_run, takeoff_longest_run,
    takeoff_reestimated_flight_threshold, ResidualComparison, ShortRunHandling,
};
use plateforce_core::trial::CentralTendency;
use plateforce_core::{
    flight_time_seconds, jump_height_from_flight_time, jump_height_from_takeoff_velocity,
    reactive_strength_index_modified, takeoff_velocity_meters_per_second, time_to_takeoff_seconds,
    DispersionEstimator, Landmarks, Trial, VarianceAccumulation, WeighingEpoch,
    STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
};
use serde::{Deserialize, Serialize};

pub const WEIGHING_CONSTRUCT: &str = "system_weight";
pub const ONSET_CONSTRUCT: &str = "movement_onset";
pub const TAKEOFF_CONSTRUCT: &str = "takeoff";

/// One rule this build can run, and the slot it fills.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Binding {
    pub id: &'static str,
    pub slot: &'static str,
    pub construct: &'static str,
    pub title: &'static str,
    /// What the interface should say when the registry has no entry under this id.
    pub note: &'static str,
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        id: "bwepoch.fixed_window",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Fixed window at the start of the recording",
        note: "",
    },
    Binding {
        id: "bwepoch.adaptive_lowest_variance",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Quietest window in the recording",
        note: "",
    },
    Binding {
        id: "bwepoch.manual_placement",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Window placed by hand",
        note: "",
    },
    Binding {
        id: "onset.threshold.noise_relative",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Noise-relative threshold, k SD of the quiet epoch",
        note: "",
    },
    Binding {
        id: "onset.threshold.relative_to_system_weight",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Fixed fraction below system weight",
        note: "",
    },
    Binding {
        id: "onset.threshold.absolute_force",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Absolute departure in newtons",
        note: "",
    },
    Binding {
        id: "onset.threshold.last_within_band",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Last sample still inside the noise band",
        note: "Executable here, and the registry carries no entry under this id yet.",
    },
    Binding {
        id: "onset.threshold.adaptive_trailing_window",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Threshold recomputed from a trailing window",
        note: "Executable here, and the registry carries no entry under this id yet.",
    },
    Binding {
        id: "takeoff.threshold.absolute_force",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "First sustained run below a residual threshold",
        note: "",
    },
    Binding {
        id: "takeoff.threshold.longest_run",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Longest run below the threshold",
        note: "Executable here, and the registry carries no entry under this id yet.",
    },
    Binding {
        id: "takeoff.threshold.descending_crossing",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Sample before a confirmed descending crossing",
        note: "Executable here, and the registry carries no entry under this id yet.",
    },
    Binding {
        id: "takeoff.threshold.flight_noise_k_sd",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Threshold re-estimated from the flight phase itself",
        note: "",
    },
];

pub fn bindings_for(slot: &str) -> impl Iterator<Item = &'static Binding> + '_ {
    BINDINGS.iter().filter(move |binding| binding.slot == slot)
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MethodChoice {
    pub method_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    /// Enumerated settings that are not numbers, such as which divisor a standard
    /// deviation uses. They are choices in exactly the same sense and are fingerprinted
    /// the same way.
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Set when the user dragged the marker. An override is a provenance fact, not a
    /// bypass, so it is reported next to the number it changed.
    #[serde(default)]
    pub manual_index: Option<usize>,
}

impl MethodChoice {
    fn number(&self, name: &str, fallback: f64) -> f64 {
        self.parameters.get(name).copied().unwrap_or(fallback)
    }

    fn samples(&self, name: &str, fallback_seconds: f64, sample_rate_hz: f64) -> usize {
        (self.number(name, fallback_seconds) * sample_rate_hz).round().max(0.0) as usize
    }

    fn option(&self, name: &str, fallback: &'static str) -> String {
        self.options.get(name).cloned().unwrap_or_else(|| fallback.to_string())
    }

    fn dispersion(&self) -> DispersionEstimator {
        match self.option("dispersion", "sample").as_str() {
            "population" => DispersionEstimator::Population,
            _ => DispersionEstimator::Sample,
        }
    }

    fn bound(&self) -> Vec<(String, String)> {
        let mut bound: Vec<(String, String)> = self
            .parameters
            .iter()
            .map(|(name, value)| (name.clone(), format_number(*value)))
            .collect();
        bound.extend(self.options.iter().map(|(k, v)| (k.clone(), v.clone())));
        bound
    }
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeighingChoice {
    pub method_id: String,
    #[serde(default)]
    pub start_index: Option<usize>,
    pub duration_seconds: f64,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisRequest {
    pub weighing: WeighingChoice,
    pub onset: MethodChoice,
    pub takeoff: MethodChoice,
    #[serde(default)]
    pub touchdown_index: Option<usize>,
    /// Gravity is a bound parameter because the tools disagree on it, and the core takes
    /// it as an argument for the same reason.
    #[serde(default = "standard_gravity")]
    pub gravity_meters_per_second_squared: f64,
    #[serde(default)]
    pub registry_backed_ids: Vec<String>,
}

fn standard_gravity() -> f64 {
    STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED
}

#[derive(Debug, Clone, Serialize)]
pub struct BoundMethod {
    pub method_id: String,
    pub bound_parameters: Vec<(String, String)>,
    pub registry_backed: bool,
    pub manual_override: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub key: &'static str,
    pub label: &'static str,
    pub value: Option<f64>,
    pub unit: &'static str,
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

/// A weighing window at an arbitrary start, without a second implementation of the mean
/// and the standard deviation. `WeighingEpoch::fixed_window` anchors at sample zero, so
/// the window is fed a trace that starts where the window does and the indices are
/// restated against the original trace afterwards.
pub fn weighing_epoch_at(
    trial: &Trial,
    start_index: usize,
    duration_seconds: f64,
    centre: CentralTendency,
    dispersion: DispersionEstimator,
) -> Result<WeighingEpoch, String> {
    if start_index == 0 {
        return WeighingEpoch::fixed_window(trial, duration_seconds, centre, dispersion)
            .map_err(|e| e.to_string());
    }
    let start_index = start_index.min(trial.len().saturating_sub(2));
    let shifted = Trial::new(trial.force()[start_index..].to_vec(), trial.sample_rate_hz())
        .map_err(|e| e.to_string())?;
    let mut epoch = WeighingEpoch::fixed_window(&shifted, duration_seconds, centre, dispersion)
        .map_err(|e| e.to_string())?;
    epoch.start_index += start_index;
    epoch.end_index += start_index;
    Ok(epoch)
}

fn resolve_weighing(
    trial: &Trial,
    choice: &WeighingChoice,
    warnings: &mut Vec<String>,
) -> Result<WeighingEpoch, String> {
    let centre = match choice.options.get("centre").map(String::as_str) {
        Some("median") => CentralTendency::Median,
        _ => CentralTendency::Mean,
    };
    let dispersion = match choice.options.get("dispersion").map(String::as_str) {
        Some("population") => DispersionEstimator::Population,
        _ => DispersionEstimator::Sample,
    };
    let window_samples = (choice.duration_seconds * trial.sample_rate_hz()).round() as usize;

    if choice.method_id == "bwepoch.adaptive_lowest_variance" && choice.start_index.is_none() {
        let accumulation = match choice.options.get("accumulation").map(String::as_str) {
            Some("cumulative_sum_of_squares") => VarianceAccumulation::CumulativeSumOfSquares,
            _ => VarianceAccumulation::TwoPass,
        };
        let epoch = WeighingEpoch::lowest_variance(
            trial,
            window_samples,
            trial.len(),
            Some(20.0),
            accumulation,
            dispersion,
        )
        .map_err(|e| e.to_string())?;
        // A minimum with exact ties does not identify one window, and on this corpus the
        // tie has run to 138 windows on the worst trial.
        if epoch.tied_window_count > 1 {
            warnings.push(format!(
                "the lowest-variance rule found {} windows tied on variance, so it does not identify a single weighing epoch",
                epoch.tied_window_count
            ));
        }
        return Ok(epoch);
    }

    weighing_epoch_at(
        trial,
        choice.start_index.unwrap_or(0),
        choice.duration_seconds,
        centre,
        dispersion,
    )
}

fn onset_search(trial: &Trial, epoch: &WeighingEpoch, choice: &MethodChoice) -> CrossingSearch {
    CrossingSearch {
        start_index: choice
            .parameters
            .get("search_floor_seconds")
            .map(|seconds| (seconds * trial.sample_rate_hz()).round() as usize)
            .unwrap_or(epoch.end_index),
        end_index: trial.len(),
        persistence_samples: choice.samples("persistence_seconds", 0.0, trial.sample_rate_hz()).max(1),
        selection: match choice.option("crossing_selection", "first").as_str() {
            "last" => CrossingSelection::Last,
            _ => CrossingSelection::First,
        },
    }
}

fn resolve_onset(
    trial: &Trial,
    epoch: &WeighingEpoch,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> Option<usize> {
    let force = trial.force();
    let rate = trial.sample_rate_hz();
    let search = onset_search(trial, epoch, choice);
    let back_offset_samples = choice.samples("back_offset", 0.030, rate);

    let crossing = match choice.method_id.as_str() {
        "onset.threshold.relative_to_system_weight" => onset_relative_to_reference(
            force,
            epoch.system_weight_newtons,
            choice.number("percent", 5.0) / 100.0,
            &search,
            rate,
        )
        .map_err(|e| e.to_string()),

        "onset.threshold.absolute_force" => {
            let departure = choice.number("threshold_newtons", 20.0);
            let band = match choice.option("direction", "below").as_str() {
                "both" => ExcursionBand::centred(epoch.system_weight_newtons, departure),
                _ => ExcursionBand::below(epoch.system_weight_newtons - departure),
            };
            sustained_excursion(force, &band, &search).ok_or_else(|| {
                format!(
                    "onset.threshold.absolute_force(threshold_newtons = {departure}) found no crossing"
                )
            })
        }

        "onset.threshold.last_within_band" => onset_last_sample_within_noise_band(
            force,
            epoch.system_weight_newtons,
            epoch.standard_deviation_newtons,
            choice.number("k", 5.0),
            plateforce_core::takeoff::force_minimum_index(force, trial.len()).unwrap_or(trial.len() - 1),
            choice.samples("inverse_lookback", 0.5, rate),
            PostCrossingRule::FixedOffset(back_offset_samples),
            rate,
        )
        .map(|outcome| {
            if outcome.clamped_at_start {
                warnings.push(
                    "the onset backtrack ran off the front of the recording and was clamped at sample zero".into(),
                );
            }
            outcome.index
        })
        .map_err(|e| e.to_string()),

        "onset.threshold.adaptive_trailing_window" => onset_adaptive_trailing_window(
            force,
            choice.samples("trailing_window", 0.5, rate).max(2),
            choice.number("k", 5.0),
            choice.dispersion(),
            rate,
        )
        .map_err(|e| e.to_string()),

        _ => onset_noise_relative(
            force,
            epoch.system_weight_newtons,
            epoch.standard_deviation_newtons,
            choice.number("k", 5.0),
            match choice.option("direction", "both").as_str() {
                "below" => BandSides::BelowOnly,
                _ => BandSides::BothSides,
            },
            // Refuse rather than substitute. A collapsed band means the window the rule
            // assumed was quiet was not, and a silent fallback would hide that.
            match choice.parameters.get("degenerate_fraction") {
                Some(fraction) => DegenerateBandPolicy::FractionOfReference(*fraction),
                None => DegenerateBandPolicy::Refuse,
            },
            &search,
            rate,
        )
        .map_err(|e| e.to_string()),
    };

    match crossing {
        Ok(index) => {
            // The trailing-window and last-within-band rules resolve their own backtrack.
            let applies_backtrack = matches!(
                choice.method_id.as_str(),
                "onset.threshold.noise_relative"
                    | "onset.threshold.relative_to_system_weight"
                    | "onset.threshold.absolute_force"
            );
            if !applies_backtrack {
                return Some(index);
            }
            let outcome = backtrack(index, back_offset_samples);
            if outcome.clamped_at_start {
                warnings.push(
                    "the onset backtrack ran off the front of the recording and was clamped at sample zero".into(),
                );
            }
            Some(outcome.index)
        }
        Err(message) => {
            warnings.push(message);
            None
        }
    }
}

struct TakeoffOutcome {
    index: Option<usize>,
    threshold_newtons: f64,
}

fn resolve_takeoff(
    trial: &Trial,
    epoch: &WeighingEpoch,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> TakeoffOutcome {
    let force = trial.force();
    let rate = trial.sample_rate_hz();
    let mut threshold_newtons = choice.number("threshold_newtons", 10.0);
    let minimum_flight_samples = choice.samples("minimum_flight", 0.100, rate).max(1);
    let comparison = match choice.option("comparison", "signed").as_str() {
        "magnitude" => ResidualComparison::Magnitude,
        _ => ResidualComparison::SignedValue,
    };

    let index = match choice.method_id.as_str() {
        "takeoff.threshold.longest_run" => {
            let handling = match choice.option("short_run_handling", "rank_then_filter").as_str() {
                "filter_then_rank" => ShortRunHandling::FilterThenRank,
                _ => ShortRunHandling::RankThenFilter,
            };
            takeoff_longest_run(force, threshold_newtons, minimum_flight_samples, comparison, handling, rate)
                .map(|selection| {
                    // On an untrimmed recording the longest low-force run is often the
                    // athlete standing off the plate. Two tools report nothing when it is.
                    if !selection.selected_is_first_qualifying {
                        warnings.push(format!(
                            "the longest-run rule skipped past the first of {} qualifying flight phases, which on an untrimmed recording places takeoff after the athlete has already landed",
                            selection.qualifying_run_count
                        ));
                    }
                    selection.start_index
                })
                .map_err(|e| e.to_string())
        }

        "takeoff.threshold.descending_crossing" => takeoff_descending_crossing(
            force,
            threshold_newtons,
            choice.samples("confirmation", 0.020, rate).max(1),
            rate,
        )
        .map_err(|e| e.to_string()),

        "takeoff.threshold.flight_noise_k_sd" => takeoff_reestimated_flight_threshold(
            force,
            epoch.end_index,
            threshold_newtons,
            choice.number("trim_fraction", 0.25),
            choice.number("k", 5.0),
            choice.dispersion(),
            rate,
        )
        .map(|flight| {
            threshold_newtons = flight.threshold_newtons;
            flight.takeoff_index
        })
        .map_err(|e| e.to_string()),

        _ => takeoff_first_sustained_run(
            force,
            threshold_newtons,
            minimum_flight_samples,
            comparison,
            epoch.end_index,
            rate,
        )
        .map_err(|e| e.to_string()),
    };

    match index {
        Ok(index) => TakeoffOutcome { index: Some(index), threshold_newtons },
        Err(message) => {
            warnings.push(message);
            TakeoffOutcome { index: None, threshold_newtons }
        }
    }
}

fn is_backed(request: &AnalysisRequest, method_id: &str) -> bool {
    request.registry_backed_ids.iter().any(|id| id == method_id)
}

pub fn run(trial: &Trial, request: &AnalysisRequest) -> Result<AnalysisResponse, String> {
    let mut warnings = Vec::new();
    let gravity = request.gravity_meters_per_second_squared;
    let epoch = resolve_weighing(trial, &request.weighing, &mut warnings)?;

    let mut bound_methods = vec![BoundMethod {
        method_id: request.weighing.method_id.clone(),
        bound_parameters: vec![
            ("start_seconds".into(), format!("{:.4}", trial.time_at(epoch.start_index))),
            ("duration_seconds".into(), format_number(request.weighing.duration_seconds)),
        ],
        registry_backed: is_backed(request, &request.weighing.method_id),
        manual_override: request.weighing.start_index.is_some_and(|start| start != 0),
    }];

    let onset_index = match request.onset.manual_index {
        Some(index) => Some(index.min(trial.len() - 1)),
        None => resolve_onset(trial, &epoch, &request.onset, &mut warnings),
    };
    bound_methods.push(BoundMethod {
        method_id: request.onset.method_id.clone(),
        bound_parameters: request.onset.bound(),
        registry_backed: is_backed(request, &request.onset.method_id),
        manual_override: request.onset.manual_index.is_some(),
    });

    let takeoff = resolve_takeoff(trial, &epoch, &request.takeoff, &mut warnings);
    let takeoff_index = match request.takeoff.manual_index {
        Some(index) => Some(index.min(trial.len() - 1)),
        None => takeoff.index,
    };
    bound_methods.push(BoundMethod {
        method_id: request.takeoff.method_id.clone(),
        bound_parameters: request.takeoff.bound(),
        registry_backed: is_backed(request, &request.takeoff.method_id),
        manual_override: request.takeoff.manual_index.is_some(),
    });

    // Touchdown is the return above the threshold that defined takeoff, so it is not an
    // independent choice and it is not offered as one.
    let touchdown_index = request.touchdown_index.or_else(|| {
        takeoff_index.and_then(|from| {
            trial.force()[from..]
                .iter()
                .position(|&force| force > takeoff.threshold_newtons)
                .map(|offset| offset + from)
        })
    });

    if let (Some(onset), Some(takeoff_at)) = (onset_index, takeoff_index) {
        if onset >= takeoff_at {
            warnings.push("onset is at or after takeoff, so every interval below is meaningless".into());
        }
    }

    let onset_band = request.onset.number("k", 5.0) * epoch.standard_deviation_newtons;
    let landmarks = match (onset_index, takeoff_index) {
        (Some(onset), Some(takeoff_at)) if takeoff_at > onset => Some(Landmarks {
            onset_index: onset,
            takeoff_index: takeoff_at,
            touchdown_index: touchdown_index.unwrap_or(trial.len() - 1),
        }),
        _ => None,
    };

    let interval = vec![request.onset.method_id.clone(), request.takeoff.method_id.clone()];
    let weighing_ids = vec![request.weighing.method_id.clone()];
    let takeoff_ids = vec![request.takeoff.method_id.clone()];
    let full = vec![
        request.weighing.method_id.clone(),
        request.onset.method_id.clone(),
        request.takeoff.method_id.clone(),
    ];

    let interval_seconds = landmarks
        .as_ref()
        .map(|marks| time_to_takeoff_seconds(marks, trial.sample_interval_seconds()));
    let flight = landmarks.as_ref().and_then(|marks| {
        touchdown_index.map(|_| flight_time_seconds(marks, trial.sample_interval_seconds()))
    });
    let velocity = landmarks
        .as_ref()
        .map(|marks| takeoff_velocity_meters_per_second(trial, &epoch, marks, gravity));
    let height_takeoff = velocity.map(|v| jump_height_from_takeoff_velocity(v, gravity));

    let metrics = vec![
        Metric {
            key: "system_weight_newtons",
            label: "System weight",
            value: Some(epoch.system_weight_newtons),
            unit: "N",
            contributing_method_ids: weighing_ids.clone(),
            note: Some("Includes any external load. System weight is not bodyweight.".into()),
        },
        Metric {
            key: "system_mass_kilograms",
            label: "System mass",
            value: Some(epoch.system_mass_kilograms(gravity)),
            unit: "kg",
            contributing_method_ids: weighing_ids,
            note: Some(format!("At g = {gravity} m/s2, which is itself a bound choice.")),
        },
        Metric {
            key: "onset_time_seconds",
            label: "Movement onset",
            value: onset_index.map(|index| trial.time_at(index)),
            unit: "s",
            contributing_method_ids: vec![request.onset.method_id.clone()],
            note: None,
        },
        Metric {
            key: "takeoff_time_seconds",
            label: "Takeoff",
            value: takeoff_index.map(|index| trial.time_at(index)),
            unit: "s",
            contributing_method_ids: takeoff_ids.clone(),
            note: None,
        },
        Metric {
            key: "time_to_takeoff_seconds",
            label: "Time to takeoff",
            value: interval_seconds,
            unit: "s",
            contributing_method_ids: interval.clone(),
            note: Some(
                "Bounded by two threshold crossings, which is why it is the least reproducible number here."
                    .into(),
            ),
        },
        Metric {
            key: "flight_time_seconds",
            label: "Flight time",
            value: flight,
            unit: "s",
            contributing_method_ids: takeoff_ids,
            note: None,
        },
        Metric {
            key: "takeoff_velocity_meters_per_second",
            label: "Takeoff velocity",
            value: velocity,
            unit: "m/s",
            contributing_method_ids: full.clone(),
            note: Some("Net impulse over system mass. An identity, not an estimate.".into()),
        },
        Metric {
            key: "net_impulse_newton_seconds",
            label: "Net impulse",
            value: landmarks.as_ref().map(|marks| {
                trial.integrate_offset_newton_seconds(
                    marks.onset_index,
                    marks.takeoff_index,
                    epoch.system_weight_newtons,
                )
            }),
            unit: "N.s",
            contributing_method_ids: full.clone(),
            note: None,
        },
        Metric {
            key: "jump_height_from_takeoff_meters",
            label: "Jump height, takeoff frame",
            value: height_takeoff,
            unit: "m",
            contributing_method_ids: full.clone(),
            note: Some(
                "Rise from the instant of takeoff. Not comparable with the standing frame without a declared correction."
                    .into(),
            ),
        },
        Metric {
            key: "jump_height_from_flight_time_meters",
            label: "Jump height, flight time",
            value: flight.map(|seconds| jump_height_from_flight_time(seconds, gravity)),
            unit: "m",
            contributing_method_ids: vec![request.takeoff.method_id.clone()],
            note: Some(
                "A different construct from the takeoff frame figure above, not a different way of computing it."
                    .into(),
            ),
        },
        Metric {
            key: "reactive_strength_index_modified",
            label: "RSI modified",
            value: match (height_takeoff, interval_seconds) {
                (Some(height), Some(seconds)) => reactive_strength_index_modified(height, seconds),
                _ => None,
            },
            unit: "m/s",
            contributing_method_ids: full,
            note: Some("Jump height over time to takeoff, so it inherits both choices.".into()),
        },
    ];

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
            takeoff_threshold_newtons: Some(takeoff.threshold_newtons),
        },
        bound_methods,
        metrics,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> Trial {
        let mut force = vec![600.0; 1200];
        for (index, value) in force.iter_mut().enumerate() {
            *value += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
        force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
        force.extend(std::iter::repeat(0.0).take(600));
        force.extend(std::iter::repeat(1400.0).take(240));
        Trial::new(force, 1200.0).unwrap()
    }

    fn request(onset_id: &str, takeoff_id: &str) -> AnalysisRequest {
        AnalysisRequest {
            weighing: WeighingChoice {
                method_id: "bwepoch.fixed_window".into(),
                start_index: None,
                duration_seconds: 0.8,
                options: BTreeMap::new(),
            },
            onset: MethodChoice { method_id: onset_id.into(), ..Default::default() },
            takeoff: MethodChoice { method_id: takeoff_id.into(), ..Default::default() },
            touchdown_index: None,
            gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            registry_backed_ids: vec!["onset.threshold.noise_relative".into()],
        }
    }

    #[test]
    fn every_binding_this_build_advertises_actually_runs() {
        let trial = synthetic();
        for binding in BINDINGS {
            if binding.slot == "weighing" {
                continue;
            }
            let mut candidate = request("onset.threshold.noise_relative", "takeoff.threshold.absolute_force");
            if binding.slot == "onset" {
                candidate.onset.method_id = binding.id.to_string();
            } else {
                candidate.takeoff.method_id = binding.id.to_string();
            }
            let response = run(&trial, &candidate)
                .unwrap_or_else(|error| panic!("{} failed to run: {error}", binding.id));
            assert!(
                !response.metrics.is_empty(),
                "{} produced no metrics",
                binding.id
            );
        }
    }

    #[test]
    fn the_weighing_bindings_all_produce_a_window() {
        let trial = synthetic();
        for method_id in ["bwepoch.fixed_window", "bwepoch.adaptive_lowest_variance"] {
            let mut candidate = request("onset.threshold.noise_relative", "takeoff.threshold.absolute_force");
            candidate.weighing.method_id = method_id.to_string();
            let response = run(&trial, &candidate).unwrap();
            assert!(response.weighing_end_index > response.weighing_start_index, "{method_id}");
        }
    }

    #[test]
    fn a_moved_weighing_window_keeps_the_weight_and_restates_the_indices() {
        let trial = synthetic();
        let anchored = weighing_epoch_at(&trial, 0, 0.5, CentralTendency::Mean, DispersionEstimator::Sample).unwrap();
        let moved = weighing_epoch_at(&trial, 240, 0.5, CentralTendency::Mean, DispersionEstimator::Sample).unwrap();
        assert_eq!(moved.start_index, 240);
        assert_eq!(moved.end_index, 240 + 600);
        assert!((moved.system_weight_newtons - anchored.system_weight_newtons).abs() < 5.0);
    }

    #[test]
    fn every_metric_names_the_methods_that_produced_it() {
        let response = run(&synthetic(), &request("onset.threshold.noise_relative", "takeoff.threshold.absolute_force")).unwrap();
        for metric in &response.metrics {
            assert!(!metric.contributing_method_ids.is_empty(), "{} carries no provenance", metric.key);
        }
    }

    #[test]
    fn a_method_absent_from_the_registry_is_marked_unbacked_rather_than_hidden() {
        let response = run(&synthetic(), &request("onset.threshold.noise_relative", "takeoff.threshold.absolute_force")).unwrap();
        let takeoff = response
            .bound_methods
            .iter()
            .find(|m| m.method_id.starts_with("takeoff."))
            .unwrap();
        assert!(!takeoff.registry_backed);
    }

    #[test]
    fn dragging_a_marker_is_recorded_as_an_override() {
        let mut candidate = request("onset.threshold.noise_relative", "takeoff.threshold.absolute_force");
        candidate.onset.manual_index = Some(1100);
        let response = run(&synthetic(), &candidate).unwrap();
        assert_eq!(response.onset_index, Some(1100));
        assert!(response.bound_methods.iter().any(|m| m.manual_override));
    }

    /// The rule that skips past the first qualifying flight phase has to say so. Two open
    /// tools do this on 155 of 244 trials and report nothing.
    #[test]
    fn the_longest_run_rule_warns_when_it_skips_the_first_flight_phase() {
        let mut force = vec![600.0; 1200];
        force.extend(std::iter::repeat(0.0).take(400));
        force.extend(std::iter::repeat(600.0).take(300));
        force.extend(std::iter::repeat(0.0).take(1200));
        let trial = Trial::new(force, 1200.0).unwrap();

        let mut candidate = request("onset.threshold.noise_relative", "takeoff.threshold.longest_run");
        candidate.weighing.duration_seconds = 0.5;
        let response = run(&trial, &candidate).unwrap();
        assert!(
            response.warnings.iter().any(|w| w.contains("qualifying flight phases")),
            "silent misplacement went unreported: {:?}",
            response.warnings
        );
    }
}
