//! The parameter set that binds each registry method to the values one reference
//! implementation used, and the analysis that runs all of them over one trial.
//!
//! Nothing here is a second implementation of any quantity. Every value is produced by
//! calling `plateforce-core`; this file only says which numbers to pass. The reference
//! implementation is a frozen numpy run over 244 trials whose rules are each cited to
//! the upstream file and line they reproduce.

use plateforce_core::onset::{
    backtrack, onset_adaptive_trailing_window, onset_last_sample_within_noise_band,
    onset_noise_relative, onset_relative_to_reference, BandSides, CrossingSearch,
    CrossingSelection, DegenerateBandPolicy, PostCrossingRule,
};
use plateforce_core::phases::{
    braking_start_by_force_minimum, braking_start_by_force_return,
    braking_start_by_velocity_minimum, velocity_zero_crossing,
};
use plateforce_core::series::{
    centre_of_mass_velocity_meters_per_second, IntegrationAnchor, IntegrationDirection,
    IntegrationSpec, IntegrationStart, QuadratureRule,
};
use plateforce_core::smoothing::savitzky_golay_interpolated_edges;
use plateforce_core::statistics::{
    index_of_maximum, mean, median, standard_deviation, DispersionEstimator, DurationRounding,
    VarianceAccumulation,
};
use plateforce_core::takeoff::{
    force_minimum_index, takeoff_descending_crossing, takeoff_first_sustained_run,
    takeoff_longest_run, takeoff_reestimated_flight_threshold, ResidualComparison,
    ShortRunHandling,
};
use plateforce_core::trial::{
    jump_height_from_takeoff_velocity, takeoff_velocity_meters_per_second, CentralTendency,
    Landmarks, WeighingEpoch,
};
use plateforce_core::{statistics::samples_for_duration, Trial};

/// The value the reference implementation writes when a rule found nothing. Reading it
/// as an index would put movement onset a twelfth of a second before the recording.
pub const REFERENCE_NOT_FOUND: f64 = -100.0;

/// Which `integration.start.*` rule this pipeline integrates the velocity series under.
///
/// Named here rather than taken from `IntegrationStart` directly, because that enum carries
/// the onset index, which is a property of the trial rather than of a binding set.
/// `integration.start.trial_start` is `deprecated` and `integration.start.detected_onset` is
/// `recommended`, and both force a decision, so a harness reproducing a pipeline that
/// integrates from sample zero says so rather than inheriting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationStartRule {
    TrialStart,
    DetectedOnset,
}

/// Which force level the braking-start search returns to.
///
/// The reference emits this column as `f_cross_bw` and computes it against the force at
/// movement onset, which is system weight less whatever threshold the bound onset rule
/// used, so its braking boundary moves when the onset rule moves.
/// `phase.braking_start.zero_net_force` states the level as system weight. Measured on
/// the six committed subject-01 trials, the two land 1 to 8 samples apart on 6 of 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrakingStartReference {
    ForceAtOnset,
    SystemWeight,
}

/// Every parameter the reference implementation bound, in one place.
///
/// The reference runs nine onset rules, five takeoff rules and four phase-boundary
/// rules over the same trace, so a single value here can move a dozen output columns.
#[derive(Debug, Clone, Copy)]
pub struct ReferenceBindings {
    pub sample_rate_hz: f64,
    pub gravity_meters_per_second_squared: f64,
    pub noise_multiplier: f64,
    pub quiet_window_seconds: [f64; 3],
    pub median_window_seconds: f64,
    pub lowest_variance_window_seconds: f64,
    pub lowest_variance_floor_newtons: f64,
    pub smoothing_window_seconds: f64,
    pub smoothing_polynomial_order: usize,
    pub smoothed_persistence_seconds: f64,
    pub search_floor_seconds: f64,
    pub sustain_seconds: f64,
    pub backtrack_seconds: f64,
    pub inverse_lookback_seconds: f64,
    pub percent_bodyweight_fraction: f64,
    pub adaptive_window_seconds: f64,
    pub flight_residual_newtons: f64,
    pub flight_hold_seconds: f64,
    pub flight_bodyweight_fraction: f64,
    pub flight_fraction_minimum_seconds: f64,
    pub descending_floor_newtons: f64,
    pub descending_bodyweight_fraction: f64,
    pub descending_confirm_samples: usize,
    pub laboratory_residual_newtons: f64,
    pub laboratory_minimum_seconds: f64,
    pub reestimation_trim_fraction: f64,
    pub whole_window_seconds: f64,
    /// Fraction of standing weight one rule falls back to when its own band collapses.
    pub collapsed_band_fraction: f64,
    /// The reference reaches numpy for every dispersion, including for the two rules it
    /// reproduces from R sources where the original calls `sd` and divides by n-1.
    pub dispersion: DispersionEstimator,
    pub variance_accumulation: VarianceAccumulation,
    pub duration_rounding: DurationRounding,
    pub residual_comparison: ResidualComparison,
    pub braking_start_reference: BrakingStartReference,
    pub integration_start: IntegrationStartRule,
}

impl Default for ReferenceBindings {
    fn default() -> Self {
        Self {
            sample_rate_hz: 1200.0,
            // The reference's own value. The software defaults to standard gravity,
            // 9.80665, and the two differ by 342 parts per million on every height.
            gravity_meters_per_second_squared: 9.81,
            noise_multiplier: 5.0,
            quiet_window_seconds: [0.25, 0.40, 1.00],
            median_window_seconds: 0.5,
            lowest_variance_window_seconds: 1.0,
            lowest_variance_floor_newtons: 10.0,
            smoothing_window_seconds: 0.2,
            smoothing_polynomial_order: 3,
            smoothed_persistence_seconds: 0.175,
            search_floor_seconds: 0.5,
            sustain_seconds: 0.03,
            backtrack_seconds: 0.03,
            inverse_lookback_seconds: 0.1,
            percent_bodyweight_fraction: 0.10,
            adaptive_window_seconds: 1.0,
            flight_residual_newtons: 10.0,
            flight_hold_seconds: 0.25,
            flight_bodyweight_fraction: 0.2,
            flight_fraction_minimum_seconds: 0.15,
            descending_floor_newtons: 20.0,
            descending_bodyweight_fraction: 0.05,
            descending_confirm_samples: 4,
            laboratory_residual_newtons: 50.0,
            laboratory_minimum_seconds: 0.1,
            reestimation_trim_fraction: 0.25,
            whole_window_seconds: 2.0,
            collapsed_band_fraction: 0.95,
            dispersion: DispersionEstimator::Population,
            variance_accumulation: VarianceAccumulation::CumulativeSumOfSquares,
            duration_rounding: DurationRounding::Truncate,
            residual_comparison: ResidualComparison::Magnitude,
            braking_start_reference: BrakingStartReference::ForceAtOnset,
            integration_start: IntegrationStartRule::TrialStart,
        }
    }
}

/// Why a trial produced no row at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrialRejection {
    TooShort { samples: usize, required: usize },
    NoTakeoff,
}

/// The nine onset rules, in the order the reference emits them.
pub const ONSET_RULES: [&str; 9] = [
    "jm",
    "jm_nosmooth",
    "bilalovski",
    "jumptest",
    "sams_bw",
    "sams_30ms",
    "pct10bw",
    "sd_minus_30ms",
    "chronojump",
];

/// The five takeoff rules, in the order the reference emits them.
pub const TAKEOFF_RULES: [&str; 5] = ["jm", "bilalovski", "jumptest", "labanalysis", "sams"];

/// The four unweighting-end rules, in the order the reference emits them.
pub const UNWEIGHTING_END_RULES: [&str; 4] =
    ["argmin_v", "f_cross_bw", "argmin_f", "argmin_f_to_takeoff"];

/// One trial run through every bound rule.
///
/// Indices are `Option` because a rule that found nothing is a different state from a
/// rule that found sample zero, and the reference conflates the two.
#[derive(Debug, Clone)]
pub struct TrialAnalysis {
    pub system_weight_newtons: [f64; 3],
    pub median_weight_newtons: f64,
    pub lowest_variance_weight_newtons: f64,
    pub lowest_variance_tied_window_count: usize,
    /// Heaviest minus lightest weight the weighing rule could have returned.
    pub lowest_variance_tied_weight_span_newtons: f64,
    /// Jump height computed at the lightest and at the heaviest weight the weighing
    /// rule could have returned, differenced. Zero when the rule found one window.
    pub jump_height_span_across_tied_weighing_windows_cm: f64,
    pub standard_deviation_newtons: [f64; 2],
    pub onset_index: [Option<usize>; 9],
    pub takeoff_index: [Option<usize>; 5],
    pub unweighting_end_index: [Option<usize>; 4],
    pub qualifying_low_force_run_count: usize,
    pub longest_run_is_first_qualifying: bool,
    /// How far past the first flight phase the longest-run rule placed takeoff.
    pub takeoff_lateness_seconds: f64,
    pub velocity_zero_index: Option<usize>,
    pub velocity_zero_is_true_crossing: bool,
    pub force_minimum_before_peak: Option<usize>,
    pub force_minimum_before_takeoff: Option<usize>,
    pub jump_height_centimetres: [f64; 9],
    pub whole_window_jump_height_centimetres: f64,
}

impl TrialAnalysis {
    pub fn time_to_takeoff_seconds(&self, rule: usize, sample_rate_hz: f64) -> f64 {
        match (self.onset_index[rule], self.takeoff_index[0]) {
            (Some(onset), Some(takeoff)) if onset > 0 => {
                (takeoff as f64 - onset as f64) / sample_rate_hz
            }
            _ => f64::NAN,
        }
    }
}

fn samples(seconds: f64, bindings: &ReferenceBindings) -> usize {
    samples_for_duration(seconds, bindings.sample_rate_hz, bindings.duration_rounding)
}

/// Run every bound rule over one trial.
pub fn analyse(
    trial: &Trial,
    bindings: &ReferenceBindings,
) -> Result<TrialAnalysis, TrialRejection> {
    let force = trial.force();
    let minimum_samples = samples(2.0, bindings);
    if force.len() < minimum_samples {
        return Err(TrialRejection::TooShort {
            samples: force.len(),
            required: minimum_samples,
        });
    }

    let takeoff_jm = takeoff_first_sustained_run(
        force,
        bindings.flight_residual_newtons,
        samples(bindings.flight_hold_seconds, bindings),
        bindings.residual_comparison,
        0,
        bindings.sample_rate_hz,
    )
    .map_err(|_| TrialRejection::NoTakeoff)?;
    if takeoff_jm == 0 {
        return Err(TrialRejection::NoTakeoff);
    }

    let weight: Vec<f64> = bindings
        .quiet_window_seconds
        .iter()
        .map(|&seconds| mean(&force[..samples(seconds, bindings)]).unwrap_or(f64::NAN))
        .collect();
    let median_weight =
        median(&force[..samples(bindings.median_window_seconds, bindings)]).unwrap_or(f64::NAN);

    let (lowest_variance, _) = WeighingEpoch::lowest_variance(
        trial,
        samples(bindings.lowest_variance_window_seconds, bindings),
        takeoff_jm,
        Some(bindings.lowest_variance_floor_newtons),
        bindings.variance_accumulation,
        bindings.dispersion,
    )
    .map_err(|_| TrialRejection::NoTakeoff)?;

    let deviation: Vec<f64> = [bindings.quiet_window_seconds[0], 0.5]
        .iter()
        .map(|&seconds| {
            standard_deviation(&force[..samples(seconds, bindings)], bindings.dispersion)
                .unwrap_or(f64::NAN)
        })
        .collect();
    let deviation_full_second = standard_deviation(
        &force[..samples(bindings.quiet_window_seconds[2], bindings)],
        bindings.dispersion,
    )
    .unwrap_or(f64::NAN);

    let peak_index = index_of_maximum(&force[..takeoff_jm]).unwrap_or_default();
    let force_minimum_before_peak = force_minimum_index(force, peak_index);
    let force_minimum_before_takeoff = force_minimum_index(force, takeoff_jm);

    let onset = onset_family(
        trial,
        bindings,
        takeoff_jm,
        &weight,
        median_weight,
        &deviation,
        deviation_full_second,
        &lowest_variance,
        force_minimum_before_peak.unwrap_or_default(),
    );

    let flight_fraction = takeoff_longest_run(
        force,
        bindings.flight_bodyweight_fraction * median_weight,
        samples(bindings.flight_fraction_minimum_seconds, bindings),
        ResidualComparison::SignedValue,
        ShortRunHandling::RankThenFilter,
        bindings.sample_rate_hz,
    );
    let takeoff = takeoff_family(force, bindings, peak_index, &weight, &flight_fraction);

    // The reference integrates the whole recording from sample zero with velocity zero
    // there, which is `integration.start.trial_start`. The binding set names the start
    // rather than leaving it to whatever a caller inherits.
    let onset_for_phases = onset[0].filter(|&index| index > 0).unwrap_or(0);
    let weighing = WeighingEpoch {
        start_index: 0,
        end_index: samples(bindings.quiet_window_seconds[0], bindings),
        system_weight_newtons: weight[0],
        standard_deviation_newtons: deviation[0],
        tied_window_count: 1,
        tied_weight_low_newtons: weight[0],
        tied_weight_high_newtons: weight[0],
    };
    let integration = IntegrationSpec {
        quadrature: QuadratureRule::Trapezoid,
        direction: IntegrationDirection::Forward,
        start: match bindings.integration_start {
            IntegrationStartRule::TrialStart => IntegrationStart::TrialStart,
            IntegrationStartRule::DetectedOnset => IntegrationStart::DetectedOnset {
                index: onset_for_phases,
            },
        },
        anchor: IntegrationAnchor::SinglePoint { index: 0 },
    };
    let velocity_series = centre_of_mass_velocity_meters_per_second(
        trial,
        &weighing,
        &integration,
        bindings.gravity_meters_per_second_squared,
    );
    let velocity_zero = velocity_zero_crossing(&velocity_series, onset_for_phases, takeoff_jm);

    let braking_reference_newtons = match bindings.braking_start_reference {
        BrakingStartReference::ForceAtOnset => force[onset_for_phases],
        BrakingStartReference::SystemWeight => weight[0],
    };
    let unweighting_end = [
        braking_start_by_velocity_minimum(&velocity_series, onset_for_phases, takeoff_jm),
        // The reference tool publishes whatever the search returns, its own bound included,
        // so the harness reads the index the way that tool does.
        braking_start_by_force_return(
            force,
            onset_for_phases,
            braking_reference_newtons,
            peak_index,
        )
        .map(|crossing| crossing.index),
        braking_start_by_force_minimum(
            force,
            onset_for_phases,
            velocity_zero
                .map(|crossing| crossing.index)
                .filter(|&index| index > 0)
                .unwrap_or(peak_index),
        ),
        braking_start_by_force_minimum(force, onset_for_phases, takeoff_jm),
    ];

    let jump_height = jump_height_family(trial, bindings, &onset, takeoff_jm, weight[0]);
    let whole_window = whole_window_jump_height(trial, bindings, takeoff_jm);

    let (qualifying_low_force_run_count, longest_run_is_first_qualifying, takeoff_lateness_seconds) =
        flight_fraction
            .as_ref()
            .map(|selection| {
                (
                    selection.qualifying_run_count,
                    selection.selected_is_first_qualifying,
                    (selection.start_index as f64 - selection.first_qualifying_start_index as f64)
                        * trial.sample_interval_seconds(),
                )
            })
            .unwrap_or((0, false, 0.0));

    Ok(TrialAnalysis {
        system_weight_newtons: [weight[0], weight[1], weight[2]],
        median_weight_newtons: median_weight,
        lowest_variance_weight_newtons: lowest_variance.system_weight_newtons,
        lowest_variance_tied_window_count: lowest_variance.tied_window_count,
        lowest_variance_tied_weight_span_newtons: lowest_variance.tied_weight_high_newtons
            - lowest_variance.tied_weight_low_newtons,
        jump_height_span_across_tied_weighing_windows_cm: tied_weighing_window_height_span(
            trial,
            bindings,
            &lowest_variance,
            onset[4],
            takeoff_jm,
        ),
        standard_deviation_newtons: [deviation[0], deviation[1]],
        onset_index: onset,
        takeoff_index: takeoff,
        unweighting_end_index: unweighting_end,
        qualifying_low_force_run_count,
        longest_run_is_first_qualifying,
        takeoff_lateness_seconds,
        velocity_zero_index: velocity_zero.map(|crossing| crossing.index),
        velocity_zero_is_true_crossing: velocity_zero
            .map(|crossing| crossing.is_true_crossing)
            .unwrap_or(false),
        force_minimum_before_peak,
        force_minimum_before_takeoff,
        jump_height_centimetres: jump_height,
        whole_window_jump_height_centimetres: whole_window,
    })
}

/// How far jump height moves across the weights the weighing rule could not choose
/// between, holding the landmarks fixed.
///
/// Scoped to the rule that consumes the searched window: its own onset, and the weight
/// it derived from that window. A pipeline anchored on a fixed window is unaffected.
fn tied_weighing_window_height_span(
    trial: &Trial,
    bindings: &ReferenceBindings,
    epoch: &WeighingEpoch,
    onset_index: Option<usize>,
    takeoff_index: usize,
) -> f64 {
    let Some(onset_index) = onset_index.filter(|&index| index > 0) else {
        return 0.0;
    };
    let gravity = bindings.gravity_meters_per_second_squared;
    let landmarks = Landmarks {
        onset_index,
        takeoff_index,
        touchdown_index: trial.len() - 1,
    };
    let height_at = |weight_newtons: f64| {
        let bound = WeighingEpoch {
            system_weight_newtons: weight_newtons,
            ..*epoch
        };
        jump_height_from_takeoff_velocity(
            takeoff_velocity_meters_per_second(trial, &bound, &landmarks, gravity),
            gravity,
        ) * 100.0
    };
    (height_at(epoch.tied_weight_high_newtons) - height_at(epoch.tied_weight_low_newtons)).abs()
}

#[allow(clippy::too_many_arguments)]
fn onset_family(
    trial: &Trial,
    bindings: &ReferenceBindings,
    takeoff_jm: usize,
    weight: &[f64],
    median_weight: f64,
    deviation: &[f64],
    deviation_full_second: f64,
    lowest_variance: &WeighingEpoch,
    force_minimum_before_peak: usize,
) -> [Option<usize>; 9] {
    let force = trial.force();
    let rate = bindings.sample_rate_hz;
    let quiet_samples = samples(bindings.search_floor_seconds, bindings);
    let persistence = samples(bindings.smoothed_persistence_seconds, bindings);
    let backtrack_samples = samples(bindings.backtrack_seconds, bindings);

    let quiet_reference = mean(&force[..quiet_samples]).unwrap_or(f64::NAN);
    let quiet_deviation =
        standard_deviation(&force[..quiet_samples], bindings.dispersion).unwrap_or(f64::NAN);

    let smoothed = savitzky_golay_interpolated_edges(
        force,
        samples(bindings.smoothing_window_seconds, bindings),
        bindings.smoothing_polynomial_order,
    )
    .unwrap_or_else(|_| force.to_vec());

    let sustained_search = CrossingSearch {
        start_index: quiet_samples,
        end_index: force.len().saturating_sub(persistence),
        persistence_samples: persistence,
        selection: CrossingSelection::First,
    };
    let immediate_search = |end: usize| CrossingSearch {
        start_index: 0,
        end_index: end,
        persistence_samples: 1,
        selection: CrossingSelection::First,
    };

    let smoothed_onset = onset_noise_relative(
        &smoothed,
        quiet_reference,
        quiet_deviation,
        bindings.noise_multiplier,
        BandSides::BelowOnly,
        DegenerateBandPolicy::UseAsStated,
        &sustained_search,
        rate,
    )
    .ok();
    let raw_onset = onset_noise_relative(
        force,
        quiet_reference,
        quiet_deviation,
        bindings.noise_multiplier,
        BandSides::BelowOnly,
        DegenerateBandPolicy::UseAsStated,
        &sustained_search,
        rate,
    )
    .ok();

    // The two-sided rule has no persistence and no search floor, and falls back to
    // sample zero rather than reporting that it found nothing.
    let two_sided = onset_noise_relative(
        force,
        median_weight,
        standard_deviation(&force[..quiet_samples], bindings.dispersion).unwrap_or(f64::NAN),
        bindings.noise_multiplier,
        BandSides::BothSides,
        DegenerateBandPolicy::UseAsStated,
        &immediate_search(takeoff_jm),
        rate,
    )
    .ok()
    .or(Some(0));

    let sustain_samples = samples_for_duration(
        bindings.sustain_seconds,
        bindings.sample_rate_hz,
        DurationRounding::Ceiling,
    )
    .max(1);
    let sustained_below = onset_noise_relative(
        force,
        weight[2],
        deviation_full_second,
        bindings.noise_multiplier,
        BandSides::BelowOnly,
        DegenerateBandPolicy::FractionOfReference(bindings.collapsed_band_fraction),
        &CrossingSearch {
            start_index: quiet_samples,
            end_index: takeoff_jm.min(force.len().saturating_sub(sustain_samples)),
            persistence_samples: sustain_samples,
            selection: CrossingSelection::First,
        },
        rate,
    )
    .ok();

    let last_within_band = |rule: PostCrossingRule| {
        onset_last_sample_within_noise_band(
            force,
            lowest_variance.system_weight_newtons,
            lowest_variance.standard_deviation_newtons,
            bindings.noise_multiplier,
            force_minimum_before_peak,
            samples(bindings.inverse_lookback_seconds, bindings),
            rule,
            rate,
        )
        .ok()
        .map(|outcome| outcome.index)
    };

    let percent_bodyweight = onset_relative_to_reference(
        force,
        weight[0],
        bindings.percent_bodyweight_fraction,
        &immediate_search(takeoff_jm),
        rate,
    )
    .ok();

    let noise_then_backtrack = onset_noise_relative(
        force,
        weight[0],
        deviation[0],
        bindings.noise_multiplier,
        BandSides::BelowOnly,
        DegenerateBandPolicy::UseAsStated,
        &immediate_search(takeoff_jm),
        rate,
    )
    .ok()
    .map(|index| backtrack(index, backtrack_samples).index);

    let adaptive = onset_adaptive_trailing_window(
        force,
        samples(bindings.adaptive_window_seconds, bindings),
        bindings.noise_multiplier,
        bindings.dispersion,
        rate,
    )
    .ok();

    [
        smoothed_onset,
        raw_onset,
        two_sided,
        sustained_below,
        last_within_band(PostCrossingRule::ToReferenceCrossing),
        last_within_band(PostCrossingRule::FixedOffset(backtrack_samples)),
        percent_bodyweight,
        noise_then_backtrack,
        adaptive,
    ]
}

fn takeoff_family(
    force: &[f64],
    bindings: &ReferenceBindings,
    peak_index: usize,
    weight: &[f64],
    flight_fraction: &Result<
        plateforce_core::takeoff::FlightPhaseSelection,
        plateforce_core::TrialError,
    >,
) -> [Option<usize>; 5] {
    let rate = bindings.sample_rate_hz;
    let absolute = takeoff_first_sustained_run(
        force,
        bindings.flight_residual_newtons,
        samples(bindings.flight_hold_seconds, bindings),
        bindings.residual_comparison,
        0,
        rate,
    )
    .ok();

    let descending = takeoff_descending_crossing(
        force,
        bindings
            .descending_floor_newtons
            .max(bindings.descending_bodyweight_fraction * weight[0]),
        bindings.descending_confirm_samples,
        rate,
    )
    .ok();

    let laboratory = takeoff_longest_run(
        force,
        bindings.laboratory_residual_newtons,
        samples(bindings.laboratory_minimum_seconds, bindings),
        ResidualComparison::SignedValue,
        ShortRunHandling::FilterThenRank,
        rate,
    )
    .ok()
    .map(|selection| selection.start_index);

    let reestimated = takeoff_reestimated_flight_threshold(
        force,
        peak_index,
        bindings.flight_residual_newtons,
        bindings.reestimation_trim_fraction,
        bindings.noise_multiplier,
        bindings.dispersion,
        rate,
    )
    .ok()
    .map(|flight| flight.takeoff_index);

    [
        absolute,
        flight_fraction
            .as_ref()
            .ok()
            .map(|selection| selection.start_index),
        descending,
        laboratory,
        reestimated,
    ]
}

fn jump_height_family(
    trial: &Trial,
    bindings: &ReferenceBindings,
    onset: &[Option<usize>; 9],
    takeoff_jm: usize,
    weight_newtons: f64,
) -> [f64; 9] {
    let epoch = WeighingEpoch {
        start_index: 0,
        end_index: samples(bindings.quiet_window_seconds[0], bindings),
        system_weight_newtons: weight_newtons,
        standard_deviation_newtons: f64::NAN,
        tied_window_count: 1,
        tied_weight_low_newtons: weight_newtons,
        tied_weight_high_newtons: weight_newtons,
    };
    let mut heights = [f64::NAN; 9];
    for (slot, index) in heights.iter_mut().zip(onset) {
        // A rule that landed on sample zero is treated as a rule that failed, which is
        // the reference's own conflation and is reported as a divergence rather than
        // silently corrected here.
        let Some(onset_index) = index.filter(|&index| index > 0) else {
            continue;
        };
        let velocity = takeoff_velocity_meters_per_second(
            trial,
            &epoch,
            &Landmarks {
                onset_index,
                takeoff_index: takeoff_jm,
                touchdown_index: trial.len() - 1,
            },
            bindings.gravity_meters_per_second_squared,
        );
        *slot =
            jump_height_from_takeoff_velocity(velocity, bindings.gravity_meters_per_second_squared)
                * 100.0;
    }
    heights
}

/// Jump height integrated over a fixed window ending at takeoff rather than from a
/// detected onset, which is what one tool's headline wrapper does.
fn whole_window_jump_height(trial: &Trial, bindings: &ReferenceBindings, takeoff_jm: usize) -> f64 {
    let window = samples(bindings.whole_window_seconds, bindings);
    let start = takeoff_jm.saturating_sub(window);
    let quiet = samples(bindings.quiet_window_seconds[0], bindings);
    let Some(crop_weight) = mean(&trial.force()[start..(start + quiet).min(takeoff_jm)]) else {
        return f64::NAN;
    };
    let epoch = WeighingEpoch {
        start_index: start,
        end_index: start + quiet,
        system_weight_newtons: crop_weight,
        standard_deviation_newtons: f64::NAN,
        tied_window_count: 1,
        tied_weight_low_newtons: crop_weight,
        tied_weight_high_newtons: crop_weight,
    };
    let velocity = takeoff_velocity_meters_per_second(
        trial,
        &epoch,
        &Landmarks {
            onset_index: start,
            takeoff_index: takeoff_jm,
            touchdown_index: trial.len() - 1,
        },
        bindings.gravity_meters_per_second_squared,
    );
    jump_height_from_takeoff_velocity(velocity, bindings.gravity_meters_per_second_squared) * 100.0
}

/// Weighing epoch used only by the fixed window rules, exposed so a caller can see
/// which window a reported weight came from.
pub fn fixed_epoch(
    trial: &Trial,
    duration_seconds: f64,
    bindings: &ReferenceBindings,
) -> Result<WeighingEpoch, plateforce_core::TrialError> {
    WeighingEpoch::fixed_window(
        trial,
        duration_seconds,
        CentralTendency::Mean,
        bindings.dispersion,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use plateforce_core::read_trial_from_path;

    fn committed_trial(trial_number: u32) -> Trial {
        let path =
            crate::fixtures_directory().join(format!("subject01_trial{trial_number}.force.txt"));
        read_trial_from_path(&path, '\t', 0, 1200.0)
            .expect("committed fixture reads")
            .0
    }

    /// The reference level and the level the registry entry states are two rules, and a
    /// binding that ignored the choice would leave every column identical.
    #[test]
    fn the_braking_start_reference_moves_the_boundary_on_every_committed_trial() {
        let at_system_weight = ReferenceBindings {
            braking_start_reference: BrakingStartReference::SystemWeight,
            ..ReferenceBindings::default()
        };
        let mut moved = 0;
        for trial_number in 1..=6 {
            let trial = committed_trial(trial_number);
            let reference = analyse(&trial, &ReferenceBindings::default())
                .expect("the reference binding analyses")
                .unweighting_end_index[1];
            let weight = analyse(&trial, &at_system_weight)
                .expect("the system weight binding analyses")
                .unweighting_end_index[1];
            assert!(reference.is_some() && weight.is_some());
            if reference != weight {
                moved += 1;
            }
        }
        assert_eq!(moved, 6, "the braking reference moved {moved} of 6 trials");
    }

    /// A field nobody can flip is a note rather than a choice. `integration.start` forces a
    /// decision in the registry, so the binding set has to be able to state either side and
    /// the two have to give different numbers.
    #[test]
    fn the_integration_start_moves_the_velocity_zero_it_is_read_from() {
        let at_detected_onset = ReferenceBindings {
            integration_start: IntegrationStartRule::DetectedOnset,
            ..ReferenceBindings::default()
        };
        let mut moved = 0;
        for trial_number in 1..=6 {
            let trial = committed_trial(trial_number);
            let reference = analyse(&trial, &ReferenceBindings::default())
                .expect("the reference binding analyses")
                .velocity_zero_index;
            let onset = analyse(&trial, &at_detected_onset)
                .expect("the detected-onset binding analyses")
                .velocity_zero_index;
            assert!(reference.is_some() && onset.is_some());
            if reference != onset {
                moved += 1;
            }
        }
        // Four rather than six: on two of the committed trials the onset falls where the
        // two rules happen to place the crossing at the same sample, so a set that only
        // ever ran those two would read the choice as making no difference.
        assert_eq!(moved, 4, "the integration start moved {moved} of 6 trials");
    }
}
