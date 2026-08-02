//! Running every defensible alternative for one quantity and reporting how far the
//! number moves.
//!
//! The alternatives are the values the literature actually contains, which the registry
//! carries per parameter. Nothing here invents a variant, and a variant that fails is
//! listed with its reason rather than dropped from the denominator.

use serde::{Deserialize, Serialize};

use plateforce_core::Trial;

use crate::AnalysisRequest;

/// One dimension of the sweep. Either the bound method for a slot changes, or one of its
/// parameters does.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub slot: String,
    #[serde(default)]
    pub parameter: Option<String>,
    #[serde(default)]
    pub values: Vec<f64>,
    #[serde(default)]
    pub method_ids: Vec<String>,
}

impl Axis {
    fn len(&self) -> usize {
        if self.method_ids.is_empty() {
            self.values.len()
        } else {
            self.method_ids.len()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpreadRequest {
    pub base: AnalysisRequest,
    pub axes: Vec<Axis>,
    pub quantity_key: String,
    #[serde(default = "default_cap")]
    pub maximum_combinations: usize,
}

fn default_cap() -> usize {
    512
}

#[derive(Debug, Clone, Serialize)]
pub struct Variant {
    pub label: String,
    pub settings: Vec<(String, String)>,
    pub value: Option<f64>,
    pub method_ids: Vec<String>,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadResponse {
    pub quantity_key: String,
    pub unit: String,
    pub unit_symbol: String,
    pub combinations_requested: usize,
    pub combinations_run: usize,
    pub capped: bool,
    pub succeeded: usize,
    pub failed: usize,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub median: Option<f64>,
    pub spread_absolute: Option<f64>,
    /// The headline figure. On the 244-trial corpus this reads 38.9 percent for time to
    /// takeoff, which is the whole argument for the registry in one number.
    pub spread_percent_of_median: Option<f64>,
    pub baseline_value: Option<f64>,
    pub variants: Vec<Variant>,
}

pub fn run(trial: &Trial, request: &SpreadRequest) -> Result<SpreadResponse, String> {
    let combinations_requested: usize =
        request.axes.iter().map(Axis::len).product::<usize>().max(1);
    let cap = request.maximum_combinations.max(1);
    let combinations_run = combinations_requested.min(cap);
    let capped = combinations_requested > cap;

    let baseline = crate::run(trial, &request.base)
        .ok()
        .and_then(|response| extract(&response, &request.quantity_key).0);

    let (unit, unit_symbol) = crate::run(trial, &request.base)
        .ok()
        .and_then(|response| {
            response
                .metrics
                .iter()
                .find(|m| m.key == request.quantity_key)
                .map(|m| (m.unit.to_string(), m.unit_symbol.to_string()))
        })
        .unwrap_or_default();

    let mut variants = Vec::with_capacity(combinations_run);
    for index in 0..combinations_run {
        let (candidate, settings) = materialise(&request.base, &request.axes, index)?;
        let method_ids = vec![
            candidate.weighing.method_id.clone(),
            candidate.onset.method_id.clone(),
            candidate.takeoff.method_id.clone(),
        ];
        let label = settings
            .iter()
            .map(|(name, value)| format!("{name} {value}"))
            .collect::<Vec<_>>()
            .join(", ");

        match crate::run(trial, &candidate) {
            Ok(response) => {
                let (value, warning) = extract(&response, &request.quantity_key);
                variants.push(Variant {
                    label: if label.is_empty() {
                        "baseline".into()
                    } else {
                        label
                    },
                    settings,
                    value,
                    method_ids,
                    failure_reason: value.is_none().then(|| {
                        warning.unwrap_or_else(|| {
                            "the rule found no crossing, so this quantity has no value".into()
                        })
                    }),
                });
            }
            Err(error) => variants.push(Variant {
                label: if label.is_empty() {
                    "baseline".into()
                } else {
                    label
                },
                settings,
                value: None,
                method_ids,
                failure_reason: Some(error),
            }),
        }
    }

    let mut values: Vec<f64> = variants.iter().filter_map(|v| v.value).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = values.get(values.len() / 2).copied();
    let minimum = values.first().copied();
    let maximum = values.last().copied();
    let spread_absolute = match (minimum, maximum) {
        (Some(low), Some(high)) => Some(high - low),
        _ => None,
    };

    Ok(SpreadResponse {
        quantity_key: request.quantity_key.clone(),
        unit,
        unit_symbol,
        combinations_requested,
        combinations_run,
        capped,
        succeeded: values.len(),
        failed: variants.len() - values.len(),
        minimum,
        maximum,
        median,
        spread_absolute,
        spread_percent_of_median: match (spread_absolute, median) {
            (Some(spread), Some(mid)) if mid.abs() > f64::EPSILON => {
                Some(100.0 * spread / mid.abs())
            }
            _ => None,
        },
        baseline_value: baseline,
        variants,
    })
}

fn extract(
    response: &crate::AnalysisResponse,
    quantity_key: &str,
) -> (Option<f64>, Option<String>) {
    let value = response
        .metrics
        .iter()
        .find(|m| m.key == quantity_key)
        .and_then(|m| m.value)
        .filter(|v| v.is_finite());
    (value, response.warnings.first().cloned())
}

/// The slots this sweep can vary, and the request field that is not a slot. Named here so a
/// refusal can print what the caller could have asked for.
const SWEEPABLE_SLOTS: &[&str] = &["weighing", "onset", "takeoff"];
const GRAVITY_FIELD: &str = "gravity_meters_per_second_squared";

/// A slot the sweep does not recognise, refused rather than skipped.
///
/// Skipping produced a run in which every variant was the baseline, so the panel printed a
/// spread of zero over knobs that had not moved and reported success. A caller reading that
/// cannot tell a method that agrees with itself from a name this function did not know.
fn unsweepable(slot: &str, parameter: Option<&str>) -> String {
    let named = match parameter {
        Some(parameter) => format!("'{slot}.{parameter}'"),
        None => format!("'{slot}'"),
    };
    format!(
        "{named} was passed as a sweep axis, and the axes this sweep can vary are the slots \
         {SWEEPABLE_SLOTS:?} and the request field '{GRAVITY_FIELD}'"
    )
}

/// Mixed-radix decode of the flat index into one point of the cartesian product.
fn materialise(
    base: &AnalysisRequest,
    axes: &[Axis],
    flat_index: usize,
) -> Result<(AnalysisRequest, Vec<(String, String)>), String> {
    let mut candidate = base.clone();
    let mut settings = Vec::new();
    let mut remainder = flat_index;

    for axis in axes {
        let width = axis.len().max(1);
        let position = remainder % width;
        remainder /= width;

        if !axis.method_ids.is_empty() {
            let method_id = axis.method_ids[position].clone();
            settings.push((axis.slot.clone(), method_id.clone()));
            // A swept slot cannot also be pinned to a dragged marker: every variant would
            // return the pinned index and the sweep would report a spread of zero.
            match axis.slot.as_str() {
                "onset" => {
                    candidate.onset.method_id = method_id;
                    candidate.onset.manual_index = None;
                }
                "takeoff" => {
                    candidate.takeoff.method_id = method_id;
                    candidate.takeoff.manual_index = None;
                }
                "weighing" => candidate.weighing.method_id = method_id,
                other => return Err(unsweepable(other, None)),
            }
            continue;
        }

        let Some(parameter) = axis.parameter.as_ref() else {
            continue;
        };
        let value = axis.values[position];
        settings.push((parameter.clone(), format_value(value)));

        match (axis.slot.as_str(), parameter.as_str()) {
            ("" | "global", GRAVITY_FIELD) => candidate.gravity_meters_per_second_squared = value,
            ("weighing", name) => {
                candidate
                    .weighing
                    .parameters
                    .insert(name.to_string(), value);
            }
            ("onset", name) => {
                candidate.onset.parameters.insert(name.to_string(), value);
                // A swept parameter has to be able to move the answer, so any marker the
                // user dragged is released for the duration of the sweep.
                candidate.onset.manual_index = None;
            }
            ("takeoff", name) => {
                candidate.takeoff.parameters.insert(name.to_string(), value);
                candidate.takeoff.manual_index = None;
            }
            (other, name) => return Err(unsweepable(other, Some(name))),
        }
    }

    Ok((candidate, settings))
}

fn format_value(value: f64) -> String {
    if (value.fract()).abs() < 1e-9 {
        format!("{value:.0}")
    } else if value.abs() < 0.1 {
        format!("{value:.3}")
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{MethodChoice, WeighingChoice};
    use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

    pub(super) fn synthetic() -> Trial {
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

    pub(super) fn base() -> AnalysisRequest {
        AnalysisRequest {
            weighing: WeighingChoice {
                method_id: "bwepoch.fixed_window".into(),
                start_index: None,
                parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
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

    #[test]
    fn the_published_values_of_one_parameter_move_the_number() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base(),
                axes: vec![Axis {
                    slot: "onset".into(),
                    parameter: Some("k".into()),
                    values: vec![2.0, 3.0, 5.0, 10.0],
                    method_ids: Vec::new(),
                }],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 512,
            },
        )
        .unwrap();

        assert_eq!(response.combinations_run, 4);
        assert_eq!(response.succeeded, 4);
        assert!(
            response.spread_absolute.unwrap() > 0.0,
            "k did not move the answer"
        );
        assert_eq!(response.unit, "seconds");
    }

    #[test]
    fn two_axes_produce_their_cartesian_product() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base(),
                axes: vec![
                    Axis {
                        slot: "onset".into(),
                        parameter: Some("k".into()),
                        values: vec![2.0, 3.0, 5.0, 10.0],
                        method_ids: Vec::new(),
                    },
                    Axis {
                        slot: "onset".into(),
                        parameter: Some("back_offset".into()),
                        values: vec![0.010, 0.030, 0.040, 0.050],
                        method_ids: Vec::new(),
                    },
                ],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 512,
            },
        )
        .unwrap();
        assert_eq!(response.combinations_requested, 16);
        assert_eq!(response.combinations_run, 16);
        assert!(!response.capped);
    }

    /// Sweeping a slot whose marker the user dragged has to release that marker, or every
    /// variant returns the pinned index and the headline figure reads zero.
    #[test]
    fn sweeping_a_slot_releases_its_dragged_marker() {
        let mut base = base();
        base.onset.manual_index = Some(1300);
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base,
                axes: vec![Axis {
                    slot: "onset".into(),
                    parameter: None,
                    values: Vec::new(),
                    method_ids: vec![
                        "onset.threshold.noise_relative".into(),
                        "onset.threshold.relative_to_system_weight".into(),
                        "onset.threshold.absolute_force".into(),
                    ],
                }],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 512,
            },
        )
        .unwrap();
        assert_eq!(response.succeeded, 3);
        assert!(
            response.spread_absolute.unwrap() > 0.0,
            "the dragged marker pinned every variant to the same answer"
        );
    }

    #[test]
    fn a_capped_sweep_says_it_was_capped_rather_than_reporting_a_short_denominator() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base(),
                axes: vec![Axis {
                    slot: "onset".into(),
                    parameter: Some("k".into()),
                    values: (1..40).map(f64::from).collect(),
                    method_ids: Vec::new(),
                }],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 10,
            },
        )
        .unwrap();
        assert!(response.capped);
        assert_eq!(response.combinations_requested, 39);
        assert_eq!(response.combinations_run, 10);
    }

    #[test]
    fn a_variant_that_fails_is_listed_with_a_reason_and_not_dropped() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base(),
                axes: vec![Axis {
                    slot: "onset".into(),
                    parameter: Some("k".into()),
                    values: vec![5.0, 100_000.0],
                    method_ids: Vec::new(),
                }],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 512,
            },
        )
        .unwrap();
        assert_eq!(response.variants.len(), 2);
        assert_eq!(response.failed, 1);
        let failed = response
            .variants
            .iter()
            .find(|v| v.value.is_none())
            .unwrap();
        assert!(failed.failure_reason.is_some());
    }
}

#[cfg(test)]
mod a_slot_the_sweep_cannot_vary {
    use std::collections::BTreeMap;

    use super::tests::{base, synthetic};
    use super::*;

    fn sweep_over(slot: &str, parameter: Option<&str>, values: Vec<f64>) -> SpreadRequest {
        SpreadRequest {
            base: base(),
            axes: vec![Axis {
                slot: slot.to_string(),
                parameter: parameter.map(str::to_string),
                values,
                method_ids: Vec::new(),
            }],
            quantity_key: "jump_height_from_takeoff_meters".into(),
            maximum_combinations: 512,
        }
    }

    /// The name the browser posts once slot identity is the construct id. Under the arm that
    /// swallowed it, every variant was the baseline and the panel printed a spread of zero.
    #[test]
    fn a_construct_id_the_sweep_does_not_know_is_refused_by_name() {
        let request = sweep_over("movement_onset", Some("k"), vec![2.0, 5.0, 8.0]);
        let refusal = run(&synthetic(), &request).expect_err("an unknown axis is refused");
        println!("{refusal}");
        assert!(refusal.contains("movement_onset.k"), "{refusal}");
        assert!(refusal.contains("onset"), "{refusal}");
    }

    #[test]
    fn an_unknown_method_axis_names_the_slot_rather_than_sweeping_nothing() {
        let mut request = sweep_over("movement_onset", None, Vec::new());
        request.axes[0].method_ids = vec![
            "onset.threshold.noise_relative".into(),
            "onset.threshold.absolute_force".into(),
        ];
        let refusal = run(&synthetic(), &request).expect_err("an unknown axis is refused");
        println!("{refusal}");
        assert!(refusal.contains("'movement_onset'"), "{refusal}");
    }

    /// The control. A knob the sweep does know still moves the number, so the refusal above
    /// is a rejected name rather than a sweep that refuses everything.
    #[test]
    fn a_slot_the_sweep_does_know_still_moves_the_number() {
        let request = sweep_over("onset", Some("k"), vec![2.0, 5.0, 8.0]);
        let response = run(&synthetic(), &request).expect("a known axis sweeps");
        println!(
            "{} of {} variants succeeded, spread {:?}",
            response.succeeded, response.combinations_run, response.spread_absolute
        );
        assert_eq!(response.succeeded, 3);
        assert!(
            response.spread_absolute.is_some_and(|spread| spread > 0.0),
            "a swept knob has to move the number: {:?}",
            response.spread_absolute
        );
    }

    #[test]
    fn the_gravity_field_is_not_a_slot_and_still_sweeps() {
        let request = sweep_over("global", Some(GRAVITY_FIELD), vec![9.8, 9.80665, 9.81]);
        let response = run(&synthetic(), &request).expect("gravity sweeps");
        assert_eq!(response.succeeded, 3);
        assert!(response.spread_absolute.is_some_and(|spread| spread > 0.0));
        let _ = BTreeMap::<String, f64>::new();
    }
}
