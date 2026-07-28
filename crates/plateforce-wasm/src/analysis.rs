//! Binding registry entries to the core, and running one analysis.
//!
//! No quantity is computed here. Every number comes back from `plateforce_core`; this
//! module decides which core function a registry id names, what parameters it was given,
//! and what provenance travels with the answer.
//!
//! Where an id below is absent from the registry it is still offered, flagged. Hiding an
//! executable rule because its paperwork is unfinished is the silent exclusion this
//! project exists to document.
//!
//! Two kinds of absence, and they are not the same claim. A composition is a registry
//! method bound with an operator, so it inherits that row's citations and needs no row of
//! its own. An unfiled rule is one the registry files elsewhere or does not carry.

use std::collections::{BTreeMap, BTreeSet};

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
    /// The registry row this id binds an operator on. Under the per-kind-of-rule grain a
    /// composition is a method plus bound parameters, so it carries the base row's
    /// citations and the fingerprint carries the binding.
    pub composed_from: Option<&'static str>,
    pub note: &'static str,
}

pub const BINDINGS: &[Binding] = &[
    Binding {
        id: "bwepoch.fixed_window",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Fixed window at the start of the recording",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "bwepoch.adaptive_lowest_variance",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Quietest window in the recording",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "bwepoch.manual_placement",
        slot: "weighing",
        construct: WEIGHING_CONSTRUCT,
        title: "Window placed by hand",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.noise_relative",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Noise-relative threshold, k SD of the quiet epoch",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.relative_to_system_weight",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Fixed fraction below system weight",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.absolute_force",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Absolute departure in newtons",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "onset.threshold.last_within_band",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Last sample still inside the noise band",
        composed_from: Some("onset.threshold.noise_relative"),
        note: "Composition: onset.op.crossing_selection bound to last.",
    },
    Binding {
        id: "onset.threshold.adaptive_trailing_window",
        slot: "onset",
        construct: ONSET_CONSTRUCT,
        title: "Threshold recomputed from a trailing window",
        composed_from: None,
        note: "The registry files this concept as bwepoch.rolling_trailing_window, in group B with the reference-epoch rules.",
    },
    Binding {
        id: "takeoff.threshold.absolute_force",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "First sustained run below a residual threshold",
        composed_from: None,
        note: "",
    },
    Binding {
        id: "takeoff.threshold.longest_run",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Longest run below the threshold",
        composed_from: Some("takeoff.threshold.absolute_force"),
        note: "Composition: onset.op.crossing_selection bound to longest_run at the falling edge.",
    },
    Binding {
        id: "takeoff.threshold.descending_crossing",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Sample before a confirmed descending crossing",
        composed_from: Some("takeoff.threshold.absolute_force"),
        note: "Composition: onset.op.direction bound at the falling edge.",
    },
    Binding {
        id: "takeoff.threshold.flight_noise_k_sd",
        slot: "takeoff",
        construct: TAKEOFF_CONSTRUCT,
        title: "Threshold re-estimated from the flight phase itself",
        composed_from: None,
        note: "plateforce_core reports this rule's failures under takeoff.threshold.reestimated_flight.",
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

/// What one rule read while it ran, and where each value came from.
///
/// Built from the request instead, a fingerprint omits every value the rule chose for
/// itself, which is the silent default this project exists to document.
struct Resolution<'a> {
    parameters: &'a BTreeMap<String, f64>,
    options: &'a BTreeMap<String, String>,
    read: Vec<(String, String)>,
    assumed: Vec<String>,
    consulted: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct BoundValues {
    parameters: Vec<(String, String)>,
    assumed: Vec<String>,
    unread: Vec<String>,
}

impl<'a> Resolution<'a> {
    fn over(parameters: &'a BTreeMap<String, f64>, options: &'a BTreeMap<String, String>) -> Self {
        Self {
            parameters,
            options,
            read: Vec::new(),
            assumed: Vec::new(),
            consulted: BTreeSet::new(),
        }
    }

    fn record(&mut self, name: &str, value: String, assumed: bool) {
        self.read.push((name.to_string(), value));
        if assumed {
            self.assumed.push(name.to_string());
        }
    }

    /// The request's value for this name, and a note that the rule asked either way.
    fn stated(&mut self, name: &str) -> Option<f64> {
        self.consulted.insert(name.to_string());
        self.parameters.get(name).copied()
    }

    fn number(&mut self, name: &str, fallback: f64) -> f64 {
        let stated = self.stated(name);
        let value = stated.unwrap_or(fallback);
        self.record(name, format_number(value), stated.is_none());
        value
    }

    fn seconds_as_samples(&mut self, name: &str, fallback_seconds: f64, sample_rate_hz: f64) -> usize {
        (self.number(name, fallback_seconds) * sample_rate_hz).round().max(0.0) as usize
    }

    fn option(&mut self, name: &str, fallback: &'static str) -> String {
        self.consulted.insert(name.to_string());
        let stated = self.options.get(name).cloned();
        let assumed = stated.is_none();
        let value = stated.unwrap_or_else(|| fallback.to_string());
        self.record(name, value.clone(), assumed);
        value
    }

    fn dispersion(&mut self) -> DispersionEstimator {
        match self.option("dispersion", "sample").as_str() {
            "population" => DispersionEstimator::Population,
            _ => DispersionEstimator::Sample,
        }
    }

    fn residual_comparison(&mut self) -> ResidualComparison {
        match self.option("comparison", "signed").as_str() {
            "magnitude" => ResidualComparison::Magnitude,
            _ => ResidualComparison::SignedValue,
        }
    }

    /// Sorted, so the same binding fingerprints the same however the request was ordered.
    fn finish(mut self) -> BoundValues {
        let unread = self
            .parameters
            .keys()
            .chain(self.options.keys())
            .filter(|name| !self.consulted.contains(*name))
            .cloned()
            .collect();
        self.read.sort();
        self.assumed.sort();
        BoundValues { parameters: self.read, assumed: self.assumed, unread }
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
    /// Names in `bound_parameters` the request did not carry, so the rule used its own.
    pub assumed_parameters: Vec<String>,
    /// Names the request carried that this rule does not read.
    pub unread_parameters: Vec<String>,
    pub registry_backed: bool,
    pub manual_override: bool,
}

fn bound_method(
    method_id: &str,
    values: BoundValues,
    registry_backed: bool,
    manual_override: bool,
) -> BoundMethod {
    BoundMethod {
        method_id: method_id.to_string(),
        bound_parameters: values.parameters,
        assumed_parameters: values.assumed,
        unread_parameters: values.unread,
        registry_backed,
        manual_override,
    }
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

/// Force at or below which a candidate weighing window is refused, keeping the unloaded
/// flight phase out of the search. No registry entry carries this value.
const ADAPTIVE_WINDOW_REJECT_AT_OR_BELOW_NEWTONS: f64 = 20.0;

struct WeighingOutcome {
    epoch: WeighingEpoch,
    bound: BoundValues,
}

fn resolve_weighing(
    trial: &Trial,
    choice: &WeighingChoice,
    warnings: &mut Vec<String>,
) -> Result<WeighingOutcome, String> {
    let no_parameters = BTreeMap::new();
    let mut resolved = Resolution::over(&no_parameters, &choice.options);
    resolved.record("duration_seconds", format_number(choice.duration_seconds), false);
    let dispersion = resolved.dispersion();
    let window_samples = (choice.duration_seconds * trial.sample_rate_hz()).round() as usize;

    if choice.method_id == "bwepoch.adaptive_lowest_variance" && choice.start_index.is_none() {
        let accumulation = match resolved.option("accumulation", "two_pass").as_str() {
            "cumulative_sum_of_squares" => VarianceAccumulation::CumulativeSumOfSquares,
            _ => VarianceAccumulation::TwoPass,
        };
        resolved.record(
            "reject_at_or_below_newtons",
            format_number(ADAPTIVE_WINDOW_REJECT_AT_OR_BELOW_NEWTONS),
            true,
        );
        let epoch = WeighingEpoch::lowest_variance(
            trial,
            window_samples,
            trial.len(),
            Some(ADAPTIVE_WINDOW_REJECT_AT_OR_BELOW_NEWTONS),
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
        // This rule finds the window in the trace rather than being told where it sits.
        resolved.record("start_seconds", format!("{:.4}", trial.time_at(epoch.start_index)), false);
        return Ok(WeighingOutcome { epoch, bound: resolved.finish() });
    }

    let centre = match resolved.option("centre", "mean").as_str() {
        "median" => CentralTendency::Median,
        _ => CentralTendency::Mean,
    };
    let epoch = weighing_epoch_at(
        trial,
        choice.start_index.unwrap_or(0),
        choice.duration_seconds,
        centre,
        dispersion,
    )?;
    resolved.record(
        "start_seconds",
        format!("{:.4}", trial.time_at(epoch.start_index)),
        choice.start_index.is_none(),
    );
    Ok(WeighingOutcome { epoch, bound: resolved.finish() })
}

fn onset_search(
    trial: &Trial,
    epoch: &WeighingEpoch,
    resolved: &mut Resolution,
) -> CrossingSearch {
    let rate = trial.sample_rate_hz();
    // Unstated, the search starts where the weighing window ended, which the weighing rule
    // decided rather than a constant.
    let start_index = match resolved.stated("search_floor_seconds") {
        Some(seconds) => {
            resolved.record("search_floor_seconds", format_number(seconds), false);
            (seconds * rate).round() as usize
        }
        None => {
            resolved.record(
                "search_floor_seconds",
                format!("{:.4}", trial.time_at(epoch.end_index)),
                true,
            );
            epoch.end_index
        }
    };
    CrossingSearch {
        start_index,
        end_index: trial.len(),
        persistence_samples: resolved.seconds_as_samples("persistence_seconds", 0.0, rate).max(1),
        selection: match resolved.option("crossing_selection", "first").as_str() {
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
) -> (Option<usize>, BoundValues) {
    let force = trial.force();
    let rate = trial.sample_rate_hz();
    let mut resolved = Resolution::over(&choice.parameters, &choice.options);

    let crossing = match choice.method_id.as_str() {
        "onset.threshold.relative_to_system_weight" => {
            let search = onset_search(trial, epoch, &mut resolved);
            let percent = resolved.number("percent", 5.0);
            onset_relative_to_reference(
                force,
                epoch.system_weight_newtons,
                percent / 100.0,
                &search,
                rate,
            )
            .map_err(|e| e.to_string())
        }

        "onset.threshold.absolute_force" => {
            let search = onset_search(trial, epoch, &mut resolved);
            let departure = resolved.number("threshold_newtons", 20.0);
            let band = match resolved.option("direction", "below").as_str() {
                "both" => ExcursionBand::centred(epoch.system_weight_newtons, departure),
                _ => ExcursionBand::below(epoch.system_weight_newtons - departure),
            };
            sustained_excursion(force, &band, &search).ok_or_else(|| {
                format!(
                    "onset.threshold.absolute_force(threshold_newtons = {departure}) found no crossing"
                )
            })
        }

        "onset.threshold.last_within_band" => {
            let k = resolved.number("k", 5.0);
            let lookback_samples = resolved.seconds_as_samples("inverse_lookback", 0.5, rate);
            let back_offset_samples = resolved.seconds_as_samples("back_offset", 0.030, rate);
            onset_last_sample_within_noise_band(
                force,
                epoch.system_weight_newtons,
                epoch.standard_deviation_newtons,
                k,
                plateforce_core::takeoff::force_minimum_index(force, trial.len())
                    .unwrap_or(trial.len() - 1),
                lookback_samples,
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
            .map_err(|e| e.to_string())
        }

        "onset.threshold.adaptive_trailing_window" => {
            let window_samples = resolved.seconds_as_samples("trailing_window", 0.5, rate).max(2);
            let k = resolved.number("k", 5.0);
            let dispersion = resolved.dispersion();
            onset_adaptive_trailing_window(force, window_samples, k, dispersion, rate)
                .map_err(|e| e.to_string())
        }

        _ => {
            let search = onset_search(trial, epoch, &mut resolved);
            let k = resolved.number("k", 5.0);
            let sides = match resolved.option("direction", "both").as_str() {
                "below" => BandSides::BelowOnly,
                _ => BandSides::BothSides,
            };
            // Refuse rather than substitute. A collapsed band means the window the rule
            // assumed was quiet was not, and a silent fallback would hide that.
            let degenerate_band = match resolved.stated("degenerate_fraction") {
                Some(fraction) => {
                    resolved.record("degenerate_fraction", format_number(fraction), false);
                    DegenerateBandPolicy::FractionOfReference(fraction)
                }
                None => {
                    resolved.record("degenerate_band", "refuse".into(), true);
                    DegenerateBandPolicy::Refuse
                }
            };
            onset_noise_relative(
                force,
                epoch.system_weight_newtons,
                epoch.standard_deviation_newtons,
                k,
                sides,
                degenerate_band,
                &search,
                rate,
            )
            .map_err(|e| e.to_string())
        }
    };

    let index = match crossing {
        Ok(index) => {
            // The trailing-window and last-within-band rules resolve their own backtrack.
            let applies_backtrack = matches!(
                choice.method_id.as_str(),
                "onset.threshold.noise_relative"
                    | "onset.threshold.relative_to_system_weight"
                    | "onset.threshold.absolute_force"
            );
            if applies_backtrack {
                let back_offset_samples = resolved.seconds_as_samples("back_offset", 0.030, rate);
                let outcome = backtrack(index, back_offset_samples);
                if outcome.clamped_at_start {
                    warnings.push(
                        "the onset backtrack ran off the front of the recording and was clamped at sample zero".into(),
                    );
                }
                Some(outcome.index)
            } else {
                Some(index)
            }
        }
        Err(message) => {
            warnings.push(message);
            None
        }
    };

    (index, resolved.finish())
}

struct TakeoffOutcome {
    index: Option<usize>,
    threshold_newtons: f64,
    bound: BoundValues,
}

fn resolve_takeoff(
    trial: &Trial,
    epoch: &WeighingEpoch,
    choice: &MethodChoice,
    warnings: &mut Vec<String>,
) -> TakeoffOutcome {
    let force = trial.force();
    let rate = trial.sample_rate_hz();
    let mut resolved = Resolution::over(&choice.parameters, &choice.options);
    let mut threshold_newtons = resolved.number("threshold_newtons", 10.0);

    let index = match choice.method_id.as_str() {
        "takeoff.threshold.longest_run" => {
            let minimum_flight_samples =
                resolved.seconds_as_samples("minimum_flight", 0.100, rate).max(1);
            let comparison = resolved.residual_comparison();
            let handling = match resolved.option("short_run_handling", "rank_then_filter").as_str() {
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

        "takeoff.threshold.descending_crossing" => {
            let confirmation_samples =
                resolved.seconds_as_samples("confirmation", 0.020, rate).max(1);
            takeoff_descending_crossing(force, threshold_newtons, confirmation_samples, rate)
                .map_err(|e| e.to_string())
        }

        "takeoff.threshold.flight_noise_k_sd" => {
            let trim_fraction = resolved.number("trim_fraction", 0.25);
            let k = resolved.number("k", 5.0);
            let dispersion = resolved.dispersion();
            takeoff_reestimated_flight_threshold(
                force,
                epoch.end_index,
                threshold_newtons,
                trim_fraction,
                k,
                dispersion,
                rate,
            )
            .map(|flight| {
                threshold_newtons = flight.threshold_newtons;
                // The seed threshold above is replaced by one measured from the flight
                // phase, and it is that one every later comparison runs against.
                resolved.record(
                    "reestimated_threshold_newtons",
                    format!("{:.4}", flight.threshold_newtons),
                    false,
                );
                flight.takeoff_index
            })
            .map_err(|e| e.to_string())
        }

        _ => {
            let minimum_flight_samples =
                resolved.seconds_as_samples("minimum_flight", 0.100, rate).max(1);
            let comparison = resolved.residual_comparison();
            takeoff_first_sustained_run(
                force,
                threshold_newtons,
                minimum_flight_samples,
                comparison,
                epoch.end_index,
                rate,
            )
            .map_err(|e| e.to_string())
        }
    };

    let bound = resolved.finish();
    match index {
        Ok(index) => TakeoffOutcome { index: Some(index), threshold_newtons, bound },
        Err(message) => {
            warnings.push(message);
            TakeoffOutcome { index: None, threshold_newtons, bound }
        }
    }
}

fn is_backed(request: &AnalysisRequest, method_id: &str) -> bool {
    request.registry_backed_ids.iter().any(|id| id == method_id)
}

pub fn run(trial: &Trial, request: &AnalysisRequest) -> Result<AnalysisResponse, String> {
    let mut warnings = Vec::new();
    let gravity = request.gravity_meters_per_second_squared;
    let weighing = resolve_weighing(trial, &request.weighing, &mut warnings)?;
    let epoch = weighing.epoch;

    let mut bound_methods = vec![bound_method(
        &request.weighing.method_id,
        weighing.bound,
        is_backed(request, &request.weighing.method_id),
        request.weighing.start_index.is_some_and(|start| start != 0),
    )];

    let (onset_index, onset_bound) = match request.onset.manual_index {
        // A dragged marker stands in for the rule, so no value the rule reads produced it.
        Some(index) => (Some(index.min(trial.len() - 1)), BoundValues::default()),
        None => resolve_onset(trial, &epoch, &request.onset, &mut warnings),
    };
    bound_methods.push(bound_method(
        &request.onset.method_id,
        onset_bound,
        is_backed(request, &request.onset.method_id),
        request.onset.manual_index.is_some(),
    ));

    // The takeoff rule runs even under a dragged marker, because the threshold it resolves
    // is what touchdown is found against.
    let takeoff = resolve_takeoff(trial, &epoch, &request.takeoff, &mut warnings);
    let takeoff_index = match request.takeoff.manual_index {
        Some(index) => Some(index.min(trial.len() - 1)),
        None => takeoff.index,
    };
    bound_methods.push(bound_method(
        &request.takeoff.method_id,
        takeoff.bound,
        is_backed(request, &request.takeoff.method_id),
        request.takeoff.manual_index.is_some(),
    ));

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

    // The band drawn on the trace is k SD wide whichever onset rule ran, so it is not part
    // of any one rule's binding.
    let onset_band =
        request.onset.parameters.get("k").copied().unwrap_or(5.0) * epoch.standard_deviation_newtons;
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

    fn two_flight_phases() -> Trial {
        let mut force = vec![600.0; 1200];
        force.extend(std::iter::repeat(0.0).take(400));
        force.extend(std::iter::repeat(600.0).take(300));
        force.extend(std::iter::repeat(0.0).take(1200));
        Trial::new(force, 1200.0).unwrap()
    }

    /// A weight shift above system weight before the countermovement, the only shape on
    /// which a band open on both sides and one open below disagree.
    fn pre_movement_bump() -> Trial {
        let mut force = vec![600.0; 1000];
        for (index, sample) in force.iter_mut().enumerate() {
            *sample += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend(std::iter::repeat_n(680.0, 120));
        force.extend(std::iter::repeat_n(600.0, 240));
        force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
        force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
        force.extend(std::iter::repeat_n(0.0, 600));
        force.extend(std::iter::repeat_n(1400.0, 240));
        Trial::new(force, 1200.0).unwrap()
    }

    /// The rule that skips past the first qualifying flight phase has to say so. Two open
    /// tools do this on 155 of 244 trials and report nothing.
    #[test]
    fn the_longest_run_rule_warns_when_it_skips_the_first_flight_phase() {
        let trial = two_flight_phases();

        let mut candidate = request("onset.threshold.noise_relative", "takeoff.threshold.longest_run");
        candidate.weighing.duration_seconds = 0.5;
        let response = run(&trial, &candidate).unwrap();
        assert!(
            response.warnings.iter().any(|w| w.contains("qualifying flight phases")),
            "silent misplacement went unreported: {:?}",
            response.warnings
        );
    }

    type CaseParameters = &'static [(&'static str, f64)];
    type CaseOptions = &'static [(&'static str, &'static str)];

    const ONSET_CASES: &[(&str, &str, CaseParameters, CaseOptions)] = &[
        ("noise_relative bare", "onset.threshold.noise_relative", &[], &[]),
        ("noise_relative k", "onset.threshold.noise_relative", &[("k", 2.0)], &[]),
        ("noise_relative direction below", "onset.threshold.noise_relative", &[], &[("direction", "below")]),
        ("noise_relative direction both", "onset.threshold.noise_relative", &[], &[("direction", "both")]),
        ("noise_relative selection last", "onset.threshold.noise_relative", &[], &[("crossing_selection", "last")]),
        ("noise_relative selection first", "onset.threshold.noise_relative", &[], &[("crossing_selection", "first")]),
        ("noise_relative persistence", "onset.threshold.noise_relative", &[("persistence_seconds", 0.01)], &[]),
        ("noise_relative search floor", "onset.threshold.noise_relative", &[("search_floor_seconds", 0.9)], &[]),
        ("noise_relative back offset", "onset.threshold.noise_relative", &[("back_offset", 0.05)], &[]),
        ("noise_relative degenerate fraction", "onset.threshold.noise_relative", &[("degenerate_fraction", 0.2)], &[]),
        (
            "noise_relative every value stated",
            "onset.threshold.noise_relative",
            &[
                ("k", 3.0),
                ("persistence_seconds", 0.005),
                ("search_floor_seconds", 0.85),
                ("back_offset", 0.02),
                ("degenerate_fraction", 0.1),
            ],
            &[("direction", "below"), ("crossing_selection", "last")],
        ),
        ("noise_relative registry names", "onset.threshold.noise_relative", &[("threshold_n", 50.0), ("pct", 1.0)], &[]),
        ("relative_to_system_weight bare", "onset.threshold.relative_to_system_weight", &[], &[]),
        ("relative_to_system_weight percent", "onset.threshold.relative_to_system_weight", &[("percent", 2.5)], &[]),
        ("relative_to_system_weight registry names", "onset.threshold.relative_to_system_weight", &[("pct", 2.5)], &[]),
        (
            "relative_to_system_weight every value stated",
            "onset.threshold.relative_to_system_weight",
            &[("percent", 10.0), ("search_floor_seconds", 0.85), ("persistence_seconds", 0.004), ("back_offset", 0.01)],
            &[("crossing_selection", "last")],
        ),
        ("absolute_force bare", "onset.threshold.absolute_force", &[], &[]),
        ("absolute_force threshold", "onset.threshold.absolute_force", &[("threshold_newtons", 50.0)], &[]),
        ("absolute_force registry names", "onset.threshold.absolute_force", &[("threshold_n", 50.0)], &[]),
        ("absolute_force direction both", "onset.threshold.absolute_force", &[], &[("direction", "both")]),
        ("absolute_force direction below", "onset.threshold.absolute_force", &[], &[("direction", "below")]),
        (
            "absolute_force every value stated",
            "onset.threshold.absolute_force",
            &[("threshold_newtons", 40.0), ("search_floor_seconds", 0.82), ("persistence_seconds", 0.006), ("back_offset", 0.04)],
            &[("direction", "both"), ("crossing_selection", "last")],
        ),
        ("last_within_band bare", "onset.threshold.last_within_band", &[], &[]),
        ("last_within_band k", "onset.threshold.last_within_band", &[("k", 3.0)], &[]),
        ("last_within_band inverse lookback", "onset.threshold.last_within_band", &[("inverse_lookback", 0.25)], &[]),
        ("last_within_band back offset", "onset.threshold.last_within_band", &[("back_offset", 0.05)], &[]),
        (
            "last_within_band every value stated",
            "onset.threshold.last_within_band",
            &[("k", 2.0), ("inverse_lookback", 0.75), ("back_offset", 0.01)],
            &[("crossing_selection", "last")],
        ),
        ("adaptive_trailing_window bare", "onset.threshold.adaptive_trailing_window", &[], &[]),
        ("adaptive_trailing_window window", "onset.threshold.adaptive_trailing_window", &[("trailing_window", 0.25)], &[]),
        ("adaptive_trailing_window k", "onset.threshold.adaptive_trailing_window", &[("k", 3.0)], &[]),
        ("adaptive_trailing_window population", "onset.threshold.adaptive_trailing_window", &[], &[("dispersion", "population")]),
        ("adaptive_trailing_window sample", "onset.threshold.adaptive_trailing_window", &[], &[("dispersion", "sample")]),
        (
            "adaptive_trailing_window every value stated",
            "onset.threshold.adaptive_trailing_window",
            &[("trailing_window", 0.75), ("k", 4.0), ("back_offset", 0.05)],
            &[("dispersion", "population")],
        ),
    ];

    const BUMP_ONSET_CASES: &[(&str, &str, CaseParameters, CaseOptions)] = &[
        ("absolute_force bare", "onset.threshold.absolute_force", &[], &[]),
        ("absolute_force direction below", "onset.threshold.absolute_force", &[], &[("direction", "below")]),
        ("absolute_force direction both", "onset.threshold.absolute_force", &[], &[("direction", "both")]),
        ("absolute_force both and last", "onset.threshold.absolute_force", &[], &[("direction", "both"), ("crossing_selection", "last")]),
        ("noise_relative bare", "onset.threshold.noise_relative", &[], &[]),
        ("noise_relative direction below", "onset.threshold.noise_relative", &[], &[("direction", "below")]),
        ("noise_relative direction both", "onset.threshold.noise_relative", &[], &[("direction", "both")]),
        ("noise_relative persistence", "onset.threshold.noise_relative", &[("persistence_seconds", 0.2)], &[("direction", "both")]),
        ("relative_to_system_weight bare", "onset.threshold.relative_to_system_weight", &[], &[]),
        ("last_within_band bare", "onset.threshold.last_within_band", &[], &[]),
        ("adaptive_trailing_window bare", "onset.threshold.adaptive_trailing_window", &[], &[]),
    ];

    const TAKEOFF_CASES: &[(&str, &str, CaseParameters, CaseOptions)] = &[
        ("absolute_force bare", "takeoff.threshold.absolute_force", &[], &[]),
        ("absolute_force threshold", "takeoff.threshold.absolute_force", &[("threshold_newtons", 25.0)], &[]),
        ("absolute_force registry names", "takeoff.threshold.absolute_force", &[("threshold_n", 30.0), ("persistence_ms", 30.0)], &[]),
        ("absolute_force minimum flight", "takeoff.threshold.absolute_force", &[("minimum_flight", 0.05)], &[]),
        ("absolute_force magnitude", "takeoff.threshold.absolute_force", &[], &[("comparison", "magnitude")]),
        ("absolute_force signed", "takeoff.threshold.absolute_force", &[], &[("comparison", "signed")]),
        (
            "absolute_force every value stated",
            "takeoff.threshold.absolute_force",
            &[("threshold_newtons", 30.0), ("minimum_flight", 0.2)],
            &[("comparison", "magnitude")],
        ),
        ("longest_run bare", "takeoff.threshold.longest_run", &[], &[]),
        ("longest_run filter then rank", "takeoff.threshold.longest_run", &[], &[("short_run_handling", "filter_then_rank")]),
        ("longest_run rank then filter", "takeoff.threshold.longest_run", &[], &[("short_run_handling", "rank_then_filter")]),
        (
            "longest_run every value stated",
            "takeoff.threshold.longest_run",
            &[("threshold_newtons", 25.0), ("minimum_flight", 0.05)],
            &[("comparison", "magnitude"), ("short_run_handling", "filter_then_rank")],
        ),
        ("descending_crossing bare", "takeoff.threshold.descending_crossing", &[], &[]),
        ("descending_crossing confirmation", "takeoff.threshold.descending_crossing", &[("confirmation", 0.05)], &[]),
        ("descending_crossing threshold", "takeoff.threshold.descending_crossing", &[("threshold_newtons", 25.0)], &[]),
        ("flight_noise_k_sd bare", "takeoff.threshold.flight_noise_k_sd", &[], &[]),
        ("flight_noise_k_sd trim", "takeoff.threshold.flight_noise_k_sd", &[("trim_fraction", 0.4)], &[]),
        ("flight_noise_k_sd k", "takeoff.threshold.flight_noise_k_sd", &[("k", 3.0)], &[]),
        ("flight_noise_k_sd population", "takeoff.threshold.flight_noise_k_sd", &[], &[("dispersion", "population")]),
        (
            "flight_noise_k_sd every value stated",
            "takeoff.threshold.flight_noise_k_sd",
            &[("trim_fraction", 0.1), ("k", 8.0), ("threshold_newtons", 20.0)],
            &[("dispersion", "population")],
        ),
    ];

    const WEIGHING_CASES: &[(&str, &str, Option<usize>, f64, CaseOptions)] = &[
        ("fixed_window bare", "bwepoch.fixed_window", None, 0.8, &[]),
        ("fixed_window half second", "bwepoch.fixed_window", None, 0.5, &[]),
        ("fixed_window moved", "bwepoch.fixed_window", Some(240), 0.8, &[]),
        ("fixed_window median", "bwepoch.fixed_window", None, 0.8, &[("centre", "median")]),
        ("fixed_window mean", "bwepoch.fixed_window", None, 0.8, &[("centre", "mean")]),
        ("fixed_window population", "bwepoch.fixed_window", None, 0.8, &[("dispersion", "population")]),
        ("fixed_window sample", "bwepoch.fixed_window", None, 0.8, &[("dispersion", "sample")]),
        (
            "fixed_window every value stated",
            "bwepoch.fixed_window",
            Some(120),
            0.6,
            &[("centre", "median"), ("dispersion", "population")],
        ),
        ("manual_placement moved", "bwepoch.manual_placement", Some(240), 0.5, &[]),
        ("manual_placement anchored", "bwepoch.manual_placement", None, 0.5, &[]),
        ("adaptive_lowest_variance bare", "bwepoch.adaptive_lowest_variance", None, 0.8, &[]),
        (
            "adaptive_lowest_variance cumulative",
            "bwepoch.adaptive_lowest_variance",
            None,
            0.8,
            &[("accumulation", "cumulative_sum_of_squares")],
        ),
        ("adaptive_lowest_variance two pass", "bwepoch.adaptive_lowest_variance", None, 0.8, &[("accumulation", "two_pass")]),
        ("adaptive_lowest_variance population", "bwepoch.adaptive_lowest_variance", None, 0.8, &[("dispersion", "population")]),
        ("adaptive_lowest_variance moved", "bwepoch.adaptive_lowest_variance", Some(240), 0.8, &[("centre", "median")]),
        ("adaptive_lowest_variance half second", "bwepoch.adaptive_lowest_variance", None, 0.5, &[]),
    ];

    struct CharacterisationCase {
        name: String,
        trial: &'static str,
        request: AnalysisRequest,
    }

    fn case_parameters(pairs: CaseParameters) -> BTreeMap<String, f64> {
        pairs.iter().map(|(name, value)| ((*name).to_string(), *value)).collect()
    }

    fn case_options(pairs: CaseOptions) -> BTreeMap<String, String> {
        pairs.iter().map(|(name, value)| ((*name).to_string(), (*value).to_string())).collect()
    }

    fn characterisation_cases() -> Vec<CharacterisationCase> {
        let mut cases = Vec::new();
        let baseline = || request("onset.threshold.noise_relative", "takeoff.threshold.absolute_force");

        for (name, method_id, parameters, options) in ONSET_CASES {
            let mut candidate = baseline();
            candidate.onset = MethodChoice {
                method_id: (*method_id).into(),
                parameters: case_parameters(parameters),
                options: case_options(options),
                manual_index: None,
            };
            cases.push(CharacterisationCase {
                name: format!("onset {name}"),
                trial: "synthetic",
                request: candidate,
            });
        }

        for (name, method_id, parameters, options) in TAKEOFF_CASES {
            let mut candidate = baseline();
            candidate.takeoff = MethodChoice {
                method_id: (*method_id).into(),
                parameters: case_parameters(parameters),
                options: case_options(options),
                manual_index: None,
            };
            cases.push(CharacterisationCase {
                name: format!("takeoff {name}"),
                trial: "synthetic",
                request: candidate,
            });
        }

        for (name, method_id, start_index, duration_seconds, options) in WEIGHING_CASES {
            let mut candidate = baseline();
            candidate.weighing = WeighingChoice {
                method_id: (*method_id).into(),
                start_index: *start_index,
                duration_seconds: *duration_seconds,
                options: case_options(options),
            };
            cases.push(CharacterisationCase {
                name: format!("weighing {name}"),
                trial: "synthetic",
                request: candidate,
            });
        }

        let mut low_gravity = baseline();
        low_gravity.gravity_meters_per_second_squared = 9.8;
        cases.push(CharacterisationCase { name: "gravity 9.8".into(), trial: "synthetic", request: low_gravity });

        let mut high_gravity = baseline();
        high_gravity.gravity_meters_per_second_squared = 9.81;
        cases.push(CharacterisationCase { name: "gravity 9.81".into(), trial: "synthetic", request: high_gravity });

        let mut stated_touchdown = baseline();
        stated_touchdown.touchdown_index = Some(2300);
        cases.push(CharacterisationCase { name: "touchdown stated".into(), trial: "synthetic", request: stated_touchdown });

        let mut dragged_onset = baseline();
        dragged_onset.onset.manual_index = Some(1100);
        dragged_onset.onset.parameters = case_parameters(&[("k", 3.0)]);
        cases.push(CharacterisationCase { name: "onset dragged".into(), trial: "synthetic", request: dragged_onset });

        let mut dragged_takeoff = baseline();
        dragged_takeoff.takeoff.manual_index = Some(2100);
        dragged_takeoff.takeoff.parameters = case_parameters(&[("threshold_newtons", 25.0)]);
        cases.push(CharacterisationCase { name: "takeoff dragged".into(), trial: "synthetic", request: dragged_takeoff });

        let mut both_dragged = baseline();
        both_dragged.onset.manual_index = Some(1150);
        both_dragged.takeoff.manual_index = Some(2050);
        cases.push(CharacterisationCase { name: "both dragged".into(), trial: "synthetic", request: both_dragged });

        let mut inverted = baseline();
        inverted.onset.manual_index = Some(2200);
        inverted.takeoff.manual_index = Some(1300);
        cases.push(CharacterisationCase { name: "onset after takeoff".into(), trial: "synthetic", request: inverted });

        let mut unbacked = baseline();
        unbacked.registry_backed_ids = Vec::new();
        cases.push(CharacterisationCase { name: "nothing registry backed".into(), trial: "synthetic", request: unbacked });

        let mut everything_backed = baseline();
        everything_backed.registry_backed_ids = BINDINGS.iter().map(|binding| binding.id.to_string()).collect();
        cases.push(CharacterisationCase {
            name: "everything registry backed".into(),
            trial: "synthetic",
            request: everything_backed,
        });

        for (name, method_id, parameters, options) in BUMP_ONSET_CASES {
            let mut candidate = baseline();
            candidate.onset = MethodChoice {
                method_id: (*method_id).into(),
                parameters: case_parameters(parameters),
                options: case_options(options),
                manual_index: None,
            };
            cases.push(CharacterisationCase {
                name: format!("bump onset {name}"),
                trial: "pre_movement_bump",
                request: candidate,
            });
        }

        for (name, takeoff_id) in [
            ("first sustained run", "takeoff.threshold.absolute_force"),
            ("longest run", "takeoff.threshold.longest_run"),
            ("descending crossing", "takeoff.threshold.descending_crossing"),
            ("flight noise", "takeoff.threshold.flight_noise_k_sd"),
        ] {
            let mut candidate = baseline();
            candidate.weighing.duration_seconds = 0.5;
            candidate.takeoff.method_id = takeoff_id.into();
            cases.push(CharacterisationCase {
                name: format!("two flight phases {name}"),
                trial: "two_flight_phases",
                request: candidate,
            });
        }

        cases
    }

    fn characterisation_report() -> String {
        let synthetic_trial = synthetic();
        let two_phase_trial = two_flight_phases();
        let bump_trial = pre_movement_bump();
        let mut report = String::new();

        for case in characterisation_cases() {
            let trial = match case.trial {
                "two_flight_phases" => &two_phase_trial,
                "pre_movement_bump" => &bump_trial,
                _ => &synthetic_trial,
            };
            report.push_str(&format!("case {} on {}\n", case.name, case.trial));
            match run(trial, &case.request) {
                Ok(response) => {
                    report.push_str(&format!(
                        "  window {}..{}\n  onset {:?}\n  takeoff {:?}\n  touchdown {:?}\n",
                        response.weighing_start_index,
                        response.weighing_end_index,
                        response.onset_index,
                        response.takeoff_index,
                        response.touchdown_index
                    ));
                    report.push_str(&format!(
                        "  levels {:?} {:?} {:?} {:?} {:?}\n",
                        response.levels.system_weight_newtons,
                        response.levels.weighing_standard_deviation_newtons,
                        response.levels.onset_band_lower_newtons,
                        response.levels.onset_band_upper_newtons,
                        response.levels.takeoff_threshold_newtons
                    ));
                    for metric in &response.metrics {
                        report.push_str(&format!("  metric {} {:?}\n", metric.key, metric.value));
                    }
                    for method in &response.bound_methods {
                        report.push_str(&format!(
                            "  method {} backed {} override {}\n",
                            method.method_id, method.registry_backed, method.manual_override
                        ));
                    }
                    for warning in &response.warnings {
                        report.push_str(&format!("  warning {warning}\n"));
                    }
                }
                Err(error) => report.push_str(&format!("  error {error}\n")),
            }
        }
        report
    }

    /// The file this compares against was recorded before the fingerprint was built from
    /// the resolution, so a number that moved shows up as a changed line.
    #[test]
    fn recording_what_each_rule_resolved_moved_no_number() {
        let expected = include_str!("../tests/resolved-values-baseline.txt");
        let found = characterisation_report();
        for (offset, (want, got)) in expected.lines().zip(found.lines()).enumerate() {
            assert_eq!(want, got, "baseline line {} moved", offset + 1);
        }
        assert_eq!(
            expected.lines().count(),
            found.lines().count(),
            "the case list is no longer the one the baseline was recorded from"
        );
    }
}
