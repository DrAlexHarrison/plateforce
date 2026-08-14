//! Running every defensible alternative for one quantity and reporting how far the
//! number moves.
//!
//! The alternatives are the values the literature actually contains, which the registry
//! carries per parameter. Nothing here invents a variant, and a variant that fails is
//! listed with its reason rather than dropped from the denominator.

use serde::{Deserialize, Serialize};

use plateforce_core::{Refusal, Trial};

use crate::AnalysisRequest;

/// One dimension of the sweep: the rule bound to a slot changes, or one of the settings that
/// rule reads does.
///
/// A setting is a number or a name, and both are choices in the same sense. The six published
/// values of `onset.k` move a jump height 0.01981 m on subject 01 trial 1, against 0.01924 m
/// for the five onset rules, so a value inside a rule moves the number as far as the choice
/// of rule does. An enumerated value is a value, which is why `options` is here beside
/// `values` rather than the axis being numeric.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub slot: String,
    #[serde(default)]
    pub parameter: Option<String>,
    #[serde(default)]
    pub values: Vec<f64>,
    /// Names the parameter takes, for a setting whose alternatives are enumerated rather
    /// than numeric: which divisor a standard deviation uses, which end of a window a
    /// landmark is read from.
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub method_ids: Vec<String>,
}

/// What one axis compares, in the order `materialise` reads them.
const KINDS: [&str; 3] = ["rules", "numbers", "names"];

impl Axis {
    /// Which of the three sets this axis states, as the words the refusal quotes.
    fn kinds_stated(&self) -> Vec<String> {
        [
            !self.method_ids.is_empty(),
            !self.values.is_empty(),
            !self.options.is_empty(),
        ]
        .iter()
        .zip(KINDS)
        .filter(|(stated, _)| **stated)
        .map(|(_, kind)| kind.to_string())
        .collect()
    }

    fn len(&self) -> usize {
        if !self.method_ids.is_empty() {
            self.method_ids.len()
        } else if !self.options.is_empty() {
            self.options.len()
        } else {
            self.values.len()
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
    /// Why this combination produced no number, as the record of the rule that declined.
    ///
    /// `None` where no rule on the quantity's chain declined, which is a different state
    /// from a rule declining and is reported as one. The variant stays in the denominator
    /// either way.
    #[serde(default)]
    pub failure_reason: Option<Refusal>,
    /// What the software knows about this variant's own result, read while its response is
    /// in hand. A variant carries the one swept quantity and a signal compares two, so
    /// nothing outside this loop could recover it.
    #[serde(default)]
    pub signals: Vec<crate::quality::QualitySignal>,
}

/// One population of variants summarised. Computed by one function over both populations
/// rather than by two functions, so the counted figures and the whole-sweep figures cannot
/// drift.
#[derive(Debug, Clone, Serialize)]
pub struct Headline {
    pub counted: usize,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub median: Option<f64>,
    pub spread_absolute: Option<f64>,
    pub spread_percent_of_median: Option<f64>,
}

fn summarise(values: &mut Vec<f64>) -> Headline {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = values.get(values.len() / 2).copied();
    let minimum = values.first().copied();
    let maximum = values.last().copied();
    // A spread is the distance between two numbers, so a set holding one holds no spread.
    // Taken over a single combination it read 0.0, which on the figure this product leads
    // with says every published alternative agreed, and one number nobody compared cannot
    // say that. The set decides it rather than the request: a sweep whose other combinations
    // all declined reaches the same single number and reported the same 0.0 with the
    // failures counted in its denominator.
    //
    // The minimum, the maximum and the median stay, because one number has all three, the
    // way a sweep that produced no number at all publishes none of them.
    let spread_absolute = match values.as_slice() {
        [] | [_] => None,
        [low, .., high] => Some(high - low),
    };
    Headline {
        counted: values.len(),
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
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SpreadResponse {
    pub quantity_key: String,
    /// What this sweep actually varied, one entry per axis.
    ///
    /// A spread is a number over a set of choices, and a reader cannot judge it without
    /// knowing which choices were in the set. This surface reported `combinations_run` and no
    /// account of what was combined, so a spread taken over the three landmark rules while the
    /// rule that computes the quantity stood still read exactly like a spread over everything.
    pub axes_varied: Vec<AxisRecord>,
    /// Rules this request bound that no axis varied, with the rule each was pinned to.
    ///
    /// The other half of the same question. A reader holding a figure can see both what moved
    /// and what did not, so the figure cannot be read as wider than the set it came from.
    pub held_fixed: Vec<HeldRule>,
    /// Landmarks a reader placed by hand that the sweep set aside, named by their slot.
    ///
    /// A swept slot cannot also be pinned, so the figures here answer for the published rules
    /// rather than for the analysis on screen, and the two are different claims. The reader is
    /// told which, from the record, because a surface working it out for itself would be a
    /// second place that knows what the sweep did.
    pub released_landmarks: Vec<String>,
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
    /// The same summary over the variants whose own signals do not fire. A rule that
    /// disagrees with itself contributes the size of its own defect to the whole-sweep
    /// figures, so a panel answering how much the method choice moves a number reads both
    /// and states both counts.
    pub over_rules_without_signals: Headline,
    /// How many valued variants the summary above set aside, so the two populations always
    /// reconcile against `succeeded`.
    pub excluded_by_a_signal: usize,
    pub baseline_value: Option<f64>,
    pub variants: Vec<Variant>,
}

/// One axis of a sweep, as the record of what was varied rather than as the request for it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AxisRecord {
    pub slot: String,
    /// The registry construct the slot names, so a reader looks up what varied rather than a
    /// word that appears in no registry file.
    pub construct: String,
    /// Rules compared along this axis, or 0 where the axis varied a value instead.
    pub rules_varied: usize,
    /// Values compared along this axis, with the parameter they were written against.
    ///
    /// Numbers and names alike, because an axis compares one or the other and the terminal's
    /// own grammar calls both a value: `--choose weighing.centre=mean` writes one exactly as
    /// `--set onset.k=5` writes the other. Which kind it was is on each variant's `settings`,
    /// under the name the registry publishes.
    pub values_varied: usize,
    pub parameter: Option<String>,
}

/// A rule the request bound and the sweep did not vary.
///
/// Read back as well as written, because a folder comparison carries this on the record it
/// leaves beside its tables and a reader of that file loads it as this type rather than as
/// a second declaration of the same two fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeldRule {
    pub construct: String,
    pub method_id: String,
}

/// The construct a slot word names, so the record carries a name the registry answers to.
fn construct_named(slot: &str) -> String {
    crate::binding::construct_for_slot(slot)
        .unwrap_or(slot)
        .to_string()
}

/// Every rule the request bound, with the constructs the axes varied removed.
///
/// Read off the request rather than from a list beside it, so a construct that becomes
/// bindable reaches this record without an edit here.
fn held_fixed(base: &AnalysisRequest, axes: &[Axis], computed_by: Option<&str>) -> Vec<HeldRule> {
    let varied: std::collections::BTreeSet<String> = axes
        .iter()
        .filter(|axis| !axis.method_ids.is_empty())
        .map(|axis| construct_named(&axis.slot))
        .collect();
    let mut held = Vec::new();
    for (construct, method_id) in [
        (crate::WEIGHING_CONSTRUCT, &base.weighing.method_id),
        (crate::ONSET_CONSTRUCT, &base.onset.method_id),
        (crate::TAKEOFF_CONSTRUCT, &base.takeoff.method_id),
    ] {
        if !method_id.is_empty() && !varied.contains(construct) {
            held.push(HeldRule {
                construct: construct.to_string(),
                method_id: method_id.clone(),
            });
        }
    }
    // Both maps, because a rule that conditions the signal the landmark rules read is a
    // choice that stood still while the figure was taken, exactly as a derived rule is. Read
    // off `derived` alone, a sweep run under a stated conditioning rule reported every axis
    // it varied and said nothing about the rule that shaped the trace underneath them.
    for (construct, choice) in base.derived.iter().chain(base.conditioning.iter()) {
        if !choice.method_id.is_empty() && !varied.contains(construct.as_str()) {
            held.push(HeldRule {
                construct: construct.clone(),
                method_id: choice.method_id.clone(),
            });
        }
    }
    // The arithmetic the spine ran for itself, which the request names nowhere. Without this a
    // sweep whose quantity came from an unchosen default reports every axis it varied and says
    // nothing about the rule that made the number.
    if let Some(id) = computed_by {
        if let Some(construct) = crate::binding::BINDINGS
            .iter()
            .find(|binding| binding.id == id)
            .map(|binding| binding.construct)
        {
            let already = held.iter().any(|rule| rule.construct == construct);
            if !already && !varied.contains(construct) {
                held.push(HeldRule {
                    construct: construct.to_string(),
                    method_id: id.to_string(),
                });
            }
        }
    }
    held
}

/// Where the binding table declares a construct, and the far end for one it declares
/// nowhere, so an axis this build runs no rule for still lands somewhere fixed.
fn table_rank_of_construct(construct: &str) -> usize {
    crate::binding::BINDINGS
        .iter()
        .position(|binding| binding.construct == construct)
        .unwrap_or(usize::MAX)
}

/// Where the binding table declares a rule, on the same terms.
fn table_rank_of_rule(method_id: &str) -> usize {
    crate::binding::BINDINGS
        .iter()
        .position(|binding| binding.id == method_id)
        .unwrap_or(usize::MAX)
}

/// What an axis sorts by: the construct's place in the table, then the name it varies, then
/// the rules along it. Enough to put two axes over one construct in a fixed order without
/// reading the order the caller wrote them in.
fn axis_order(axis: &Axis) -> (usize, String, String, Vec<usize>) {
    let construct = construct_named(&axis.slot);
    (
        table_rank_of_construct(&construct),
        construct,
        axis.parameter.clone().unwrap_or_default(),
        axis.method_ids
            .iter()
            .map(|id| table_rank_of_rule(id))
            .collect(),
    )
}

/// The last tiebreak, for two axes over one parameter of one construct carrying different
/// value lists. Lexicographic over the values, which are already in ascending order.
fn values_order(left: &Axis, right: &Axis) -> std::cmp::Ordering {
    left.values
        .iter()
        .zip(right.values.iter())
        .map(|(left, right)| left.total_cmp(right))
        .find(|order| order.is_ne())
        .unwrap_or_else(|| left.values.len().cmp(&right.values.len()))
}

/// One sweep, one document, whichever surface asked for it.
///
/// The axes and the rules along them are a set of choices, and the order a caller listed
/// them in is a fact about that caller rather than about the sweep. Reported as written, the
/// same sweep left the terminal and the browser tab differing in 520 paths of `variants`
/// while all 17 other compared fields agreed and the 75 labels matched as a set, because the
/// tab sends rules in the order it ranks them for a reader and the terminal reads the
/// binding table. So the record is ordered here, once, and a surface wanting a reader's
/// ranking re-ranks what it renders.
///
/// This also fixes which combinations a capped sweep runs, which was otherwise the caller's
/// list order deciding what the cap cut.
fn ordered_by_the_binding_table(axes: &[Axis]) -> Vec<Axis> {
    let mut ordered: Vec<Axis> = axes
        .iter()
        .map(|axis| {
            let mut axis = axis.clone();
            axis.method_ids.sort_by(|left, right| {
                table_rank_of_rule(left)
                    .cmp(&table_rank_of_rule(right))
                    .then_with(|| left.cmp(right))
            });
            axis.values.sort_by(f64::total_cmp);
            axis.options.sort();
            axis
        })
        .collect();
    ordered.sort_by(|left, right| {
        axis_order(left)
            .cmp(&axis_order(right))
            .then_with(|| values_order(left, right))
            .then_with(|| left.options.cmp(&right.options))
    });
    ordered
}

/// An axis naming a step and nothing to vary along it, refused rather than run.
///
/// Both shapes that reach this were measured on this file. An axis carrying neither rules
/// nor values has a width of zero, and the product below multiplied it away: a four-value
/// axis that alone ran 4 combinations and reported 0.0158 seconds ran 1 and reported a
/// spread of zero with an empty axis beside it. An axis naming a parameter and no values
/// indexed an empty list and brought the library down, which `pf.spread(parameter="k")`
/// with no `values` reached on the shipped Python surface.
///
/// The code is the one for a name that is known with its value missing, because that is the
/// fault here: the step is one this request carries, and what to compare along it was never
/// said.
fn nothing_to_vary(axis: &Axis) -> Box<Refusal> {
    Box::new(Refusal::sweep_axis_states_no_alternative(named_axis(axis)))
}

/// What a refusal calls one axis: the step alone where the rule varies, the step and the
/// setting where a value does.
fn named_axis(axis: &Axis) -> String {
    match axis.parameter.as_deref() {
        Some(parameter) => format!("{}.{parameter}", axis.slot),
        None => axis.slot.clone(),
    }
}

pub fn run(trial: &Trial, request: &SpreadRequest) -> Result<SpreadResponse, Box<Refusal>> {
    let axes = ordered_by_the_binding_table(&request.axes);
    // Both faults are in the width of one axis, and the over-stated one is checked first
    // because an axis stating two sets has a width under either reading and would sweep.
    if let Some(crowded) = axes.iter().find(|axis| axis.kinds_stated().len() > 1) {
        return Err(Box::new(Refusal::sweep_axis_compares_more_than_one_kind(
            named_axis(crowded),
            crowded.kinds_stated(),
        )));
    }
    if let Some(empty) = axes.iter().find(|axis| axis.len() == 0) {
        return Err(nothing_to_vary(empty));
    }
    let combinations_requested: usize = axes.iter().map(Axis::len).product::<usize>().max(1);
    let cap = request.maximum_combinations.max(1);
    let combinations_run = combinations_requested.min(cap);
    let capped = combinations_requested > cap;

    let baseline = crate::run(trial, &request.base)
        .ok()
        .and_then(|response| extract(&response, &request.quantity_key));

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

    // The rule that computed the quantity, when the spine ran it under its own default and the
    // request therefore names it nowhere. It is still a choice that stood still while the
    // figure was taken, and a reader who cannot see it reads the spread as wider than its set.
    let computed_by = crate::run(trial, &request.base).ok().and_then(|response| {
        response
            .metrics
            .iter()
            .find(|metric| metric.key == request.quantity_key)
            .and_then(|metric| metric.computed_by.clone())
    });

    let mut variants = Vec::with_capacity(combinations_run);
    for index in 0..combinations_run {
        let (candidate, settings) = materialise(&request.base, &axes, index)?;
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
                let value = extract(&response, &request.quantity_key);
                variants.push(Variant {
                    label: if label.is_empty() {
                        "baseline".into()
                    } else {
                        label
                    },
                    settings,
                    value,
                    method_ids,
                    failure_reason: value
                        .is_none()
                        .then(|| declined_for(&response, &request.quantity_key))
                        .flatten(),
                    signals: crate::quality::signals(&response),
                });
            }
            Err(refusal) => variants.push(Variant {
                label: if label.is_empty() {
                    "baseline".into()
                } else {
                    label
                },
                settings,
                value: None,
                method_ids,
                failure_reason: Some(*refusal),
                signals: Vec::new(),
            }),
        }
    }

    let mut values: Vec<f64> = variants.iter().filter_map(|v| v.value).collect();
    let whole = summarise(&mut values);

    let mut without_signals: Vec<f64> = variants
        .iter()
        .filter(|variant| !crate::quality::distrusted(&variant.signals))
        .filter_map(|variant| variant.value)
        .collect();
    let over_rules_without_signals = summarise(&mut without_signals);
    let excluded_by_a_signal = whole.counted - over_rules_without_signals.counted;

    Ok(SpreadResponse {
        quantity_key: request.quantity_key.clone(),
        axes_varied: axes
            .iter()
            .map(|axis| AxisRecord {
                slot: axis.slot.clone(),
                construct: construct_named(&axis.slot),
                rules_varied: axis.method_ids.len(),
                values_varied: axis.values.len() + axis.options.len(),
                parameter: axis.parameter.clone(),
            })
            .collect(),
        held_fixed: held_fixed(&request.base, &axes, computed_by.as_deref()),
        released_landmarks: released_landmarks(&request.base, &axes),
        unit,
        unit_symbol,
        combinations_requested,
        combinations_run,
        capped,
        succeeded: whole.counted,
        failed: variants.len() - whole.counted,
        minimum: whole.minimum,
        maximum: whole.maximum,
        median: whole.median,
        spread_absolute: whole.spread_absolute,
        spread_percent_of_median: whole.spread_percent_of_median,
        over_rules_without_signals,
        excluded_by_a_signal,
        baseline_value: baseline,
        variants,
    })
}

fn extract(response: &crate::AnalysisResponse, quantity_key: &str) -> Option<f64> {
    response
        .metrics
        .iter()
        .find(|m| m.key == quantity_key)
        .and_then(|m| m.value)
        .filter(|v| v.is_finite())
}

/// Why this quantity has no value on this variant, taken from a rule that declined on its
/// own chain.
///
/// Attributed rather than assumed: the refusal has to name a rule the quantity itself says
/// produced it or fed it, so a rule declining elsewhere in the analysis is not written against
/// a number it had no part in. Where nothing on the chain declined this is `None`, because a
/// cause nobody recorded is not a cause to report.
///
/// The chain is `chain::accounts_for`'s, which is the one the account under a blank cell is
/// written from. Read here against the contributing rules alone, it could not see the
/// arithmetic that computed the quantity, and 60 of 75 variants of the committed sweep came
/// back with no value and no reason.
fn declined_for(response: &crate::AnalysisResponse, quantity_key: &str) -> Option<Refusal> {
    let metric = response
        .metrics
        .iter()
        .find(|metric| metric.key == quantity_key)?;
    crate::chain::refusal_accounting_for(response, metric)
}

/// The three landmark slots, which are reached by their own names on the request.
const LANDMARK_SLOTS: &[&str] = &["weighing", "onset", "takeoff"];
const GRAVITY_FIELD: &str = "gravity_meters_per_second_squared";

/// An axis the sweep cannot vary, refused rather than skipped.
///
/// Skipping produced a run in which every variant was the baseline, so the panel printed a
/// spread of zero over knobs that had not moved and reported success. A caller reading that
/// cannot tell a method that agrees with itself from a name this function did not know.
///
/// The list of what could have been asked for is the three landmark slots plus the
/// constructs this request actually carries and the gravity field, not a fixed three. A
/// construct the build runs a rule for and this request did not name is not an axis:
/// sweeping it would run a rule nobody chose.
fn unsweepable(base: &AnalysisRequest, slot: &str, parameter: Option<&str>) -> Box<Refusal> {
    let axis = match parameter {
        Some(parameter) => format!("{slot}.{parameter}"),
        None => slot.to_string(),
    };
    let mut offered: Vec<String> = LANDMARK_SLOTS.iter().map(|s| (*s).to_string()).collect();
    offered.extend(base.derived.keys().cloned());
    offered.extend(base.conditioning.keys().cloned());
    offered.push(format!("global.{GRAVITY_FIELD}"));
    Box::new(Refusal::axis_not_in_this_request(axis, offered))
}

/// One point of the cartesian product: the request to run, and the settings naming it.
type SweepPoint = (AnalysisRequest, Vec<(String, String)>);

/// Mixed-radix decode of the flat index into one point of the cartesian product.
fn materialise(
    base: &AnalysisRequest,
    axes: &[Axis],
    flat_index: usize,
) -> Result<SweepPoint, Box<Refusal>> {
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
                construct => match rule_bound_for(&mut candidate, construct) {
                    Some(choice) => {
                        choice.method_id = method_id;
                        choice.manual_index = None;
                    }
                    None => return Err(unsweepable(base, construct, None)),
                },
            }
            continue;
        }

        let Some(parameter) = axis.parameter.as_ref() else {
            continue;
        };

        if !axis.options.is_empty() {
            let chosen = axis.options[position].clone();
            settings.push((parameter.clone(), chosen.clone()));
            // Gravity is a number the run carries and no rule reads, so it takes no name.
            if matches!(axis.slot.as_str(), "" | "global") {
                return Err(unsweepable(base, &axis.slot, Some(parameter)));
            }
            match options_bound_for(&mut candidate, &axis.slot) {
                Some(options) => {
                    options.insert(parameter.clone(), chosen);
                }
                None => return Err(unsweepable(base, &axis.slot, Some(parameter))),
            }
            release_dragged_marker(&mut candidate, &axis.slot);
            continue;
        }

        let value = axis.values[position];
        // The spelling every other record uses. Written at two decimals here, a sweep over
        // 9.80665 and 9.8070 labelled both variants `9.81` and left two different numbers
        // resting on one account of what produced them.
        settings.push((parameter.clone(), crate::recorded_number_text(value)));

        match (axis.slot.as_str(), parameter.as_str()) {
            // Stated, because naming an axis and the values along it is the caller choosing
            // every one of them. Left as the request's own default the rule that publishes a
            // gravity would keep publishing it, and the panel would print a spread of zero
            // over a knob that moved.
            ("" | "global", GRAVITY_FIELD) => candidate.state_gravity(Some(value)),
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
            (construct, name) => match rule_bound_for(&mut candidate, construct) {
                Some(choice) => {
                    choice.parameters.insert(name.to_string(), value);
                    choice.manual_index = None;
                }
                None => return Err(unsweepable(base, construct, Some(name))),
            },
        }
    }

    Ok((candidate, settings))
}

/// The choice a request carries for a construct it bound, wherever it carries it.
///
/// A construct is in one of the two maps or in neither, so the phase it runs in is not
/// something a caller naming an axis has to know. Reached through `derived` alone, a sweep
/// over a conditioning rule was refused as a name the request did not carry, on a request
/// that carried it.
/// The same lookup without taking the request mutably, for reading what a sweep will set aside.
/// One shape, so a construct found by one is found by the other.
fn rule_bound_for_read<'a>(
    request: &'a AnalysisRequest,
    construct: &str,
) -> Option<&'a crate::MethodChoice> {
    if request.derived.contains_key(construct) {
        return request.derived.get(construct);
    }
    request.conditioning.get(construct)
}

fn rule_bound_for<'a>(
    request: &'a mut AnalysisRequest,
    construct: &str,
) -> Option<&'a mut crate::MethodChoice> {
    if request.derived.contains_key(construct) {
        return request.derived.get_mut(construct);
    }
    request.conditioning.get_mut(construct)
}

/// Where a step's enumerated settings are written. `weighing` carries its own choice type,
/// which is why this is not `rule_bound_for` with a field access on the end.
fn options_bound_for<'a>(
    request: &'a mut AnalysisRequest,
    slot: &str,
) -> Option<&'a mut std::collections::BTreeMap<String, String>> {
    match slot {
        "weighing" => Some(&mut request.weighing.options),
        "onset" => Some(&mut request.onset.options),
        "takeoff" => Some(&mut request.takeoff.options),
        construct => rule_bound_for(request, construct).map(|choice| &mut choice.options),
    }
}

/// Which of the swept slots the reader had placed by hand, so the answer says whose analysis it
/// is describing.
///
/// Read from the same request and the same axes that `release_dragged_marker` acts on, so the
/// two cannot drift: a slot appears here exactly when the sweep set its marker aside.
fn released_landmarks(base: &AnalysisRequest, axes: &[Axis]) -> Vec<String> {
    axes.iter()
        .map(|axis| axis.slot.as_str())
        .filter(|slot| match *slot {
            "weighing" => false,
            "onset" => base.onset.manual_index.is_some(),
            "takeoff" => base.takeoff.manual_index.is_some(),
            construct => rule_bound_for_read(base, construct)
                .is_some_and(|choice| choice.manual_index.is_some()),
        })
        .map(str::to_string)
        .collect()
}

/// A swept setting has to be able to move the answer, so any marker the user dragged on that
/// step is released for the duration of the sweep. Weighing carries a start index rather than
/// a marker and keeps it.
fn release_dragged_marker(request: &mut AnalysisRequest, slot: &str) {
    match slot {
        "weighing" => {}
        "onset" => request.onset.manual_index = None,
        "takeoff" => request.takeoff.manual_index = None,
        construct => {
            if let Some(choice) = rule_bound_for(request, construct) {
                choice.manual_index = None;
            }
        }
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

    /// The gradual jump above, with its flight sized to its own impulse so the two height
    /// routes agree when the start of the jump is placed correctly.
    ///
    /// The unweighting descends over 360 samples rather than stepping, so an onset threshold
    /// swept across that ramp genuinely moves the landmark: a small threshold catches the
    /// start of the descent and a large one catches it 0.28 s late, missing real negative
    /// impulse. The late variant then reads roughly triple the flight route's height while
    /// the correct one sits within a percent, which is what a sweep needs before it can
    /// report two populations rather than one.
    pub(super) fn jump_whose_two_routes_agree() -> Trial {
        let mut force = vec![600.0; 1200];
        for (index, sample) in force.iter_mut().enumerate() {
            *sample += ((index % 17) as f64 - 8.0) * 0.4;
        }
        force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
        force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
        force.extend(std::iter::repeat_n(0.0, 180));
        force.extend(std::iter::repeat_n(1400.0, 240));
        Trial::new(force, 1200.0).unwrap()
    }

    pub(super) fn base() -> AnalysisRequest {
        crate::request::prepared(AnalysisRequest {
            weighing: WeighingChoice {
                method_id: "bwepoch.fixed_window".into(),
                start_index: None,
                parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
                options: BTreeMap::new(),
                ..Default::default()
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
            ..Default::default()
        })
    }

    /// Two swept values a reader can tell apart are two variants a reader can tell apart.
    ///
    /// The settings were written at two decimals, so `k` swept over 2.001 and 2.002 labelled
    /// both variants `k 2.00` while the two produced different numbers, and the panel showed
    /// a spread over an axis whose own record said it had not moved. Asserted on the settings
    /// rather than on the label, because the label is assembled from them and a guard reading
    /// the assembled string cannot say which half lost the digits.
    #[test]
    fn two_swept_values_that_differ_late_are_two_settings_a_reader_can_tell_apart() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base(),
                axes: vec![Axis {
                    slot: "onset".into(),
                    parameter: Some("k".into()),
                    values: vec![2.001, 2.002],
                    options: Vec::new(),
                    method_ids: Vec::new(),
                }],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 512,
            },
        )
        .unwrap();

        let written: Vec<String> = response
            .variants
            .iter()
            .flat_map(|variant| variant.settings.iter())
            .filter(|(name, _)| name == "k")
            .map(|(_, value)| value.clone())
            .collect();
        assert_eq!(
            written,
            vec!["2.001".to_string(), "2.002".to_string()],
            "the sweep ran two values of k and wrote {written:?}"
        );
    }

    /// The spelling a swept setting is written in is the one every other record uses, so a
    /// reader comparing a swept `k` against the `k` an analysed record names is comparing two
    /// spellings of one number rather than two numbers.
    #[test]
    fn a_swept_setting_is_written_the_way_every_other_record_writes_it() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base(),
                axes: vec![Axis {
                    slot: "onset".into(),
                    parameter: Some("k".into()),
                    values: vec![5.0, 2.5],
                    options: Vec::new(),
                    method_ids: Vec::new(),
                }],
                quantity_key: "time_to_takeoff_seconds".into(),
                maximum_combinations: 512,
            },
        )
        .unwrap();

        // The sweep orders its own axis, which `the_order_the_caller_typed_the_values_in_does
        // _not_reach_the_document` pins, so the set is compared rather than the sequence.
        let written: Vec<String> = response
            .variants
            .iter()
            .flat_map(|variant| variant.settings.iter())
            .filter(|(name, _)| name == "k")
            .map(|(_, value)| value.clone())
            .collect();
        let through_the_one_home: Vec<String> = [2.5_f64, 5.0]
            .iter()
            .map(|value| crate::recorded_number_text(*value))
            .collect();
        assert_eq!(written, through_the_one_home);
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
                    options: Vec::new(),
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
                        options: Vec::new(),
                        method_ids: Vec::new(),
                    },
                    Axis {
                        slot: "onset".into(),
                        parameter: Some("back_offset".into()),
                        values: vec![0.010, 0.030, 0.040, 0.050],
                        options: Vec::new(),
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
                    options: Vec::new(),
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
                    options: Vec::new(),
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
                    options: Vec::new(),
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
        let reason = failed
            .failure_reason
            .as_ref()
            .expect("a variant that failed says why");
        // The record the rule built, so a caller reads the code and the value it declined on
        // rather than parsing a sentence. On this variant the band is wider than the trace,
        // which is a collapsed band and not a crossing nobody found: the written sentence
        // this replaced named the wrong one of the two.
        assert_eq!(reason.code, plateforce_core::RefusalCode::CollapsedBand);
        assert_eq!(reason.parameter.as_deref(), Some("k"));
        assert_eq!(reason.value, Some(100_000.0));
        assert_eq!(reason.slot.as_deref(), Some("movement_onset"));
    }

    /// A request binding the one construct whose alternatives are names rather than numbers,
    /// so the sweeps below vary an enumerated setting on a rule that reads one.
    fn base_over_an_epoch_impulse(convention: &str) -> AnalysisRequest {
        let mut base = base();
        base.derived.insert(
            crate::slots::epoch_impulse::CONSTRUCT.to_string(),
            MethodChoice {
                method_id: crate::slots::epoch_impulse::epoch_from_onset::ID.to_string(),
                options: BTreeMap::from([(
                    crate::slots::epoch_impulse::CONVENTION_PARAMETER.to_string(),
                    convention.to_string(),
                )]),
                ..Default::default()
            },
        );
        base
    }

    fn axis_over_the_convention(options: Vec<String>) -> Axis {
        Axis {
            slot: crate::slots::epoch_impulse::CONSTRUCT.to_string(),
            parameter: Some(crate::slots::epoch_impulse::CONVENTION_PARAMETER.to_string()),
            values: Vec::new(),
            options,
            method_ids: Vec::new(),
        }
    }

    /// An enumerated setting is a setting, and the axis carried numbers only.
    ///
    /// The convention an impulse is added up under is a name, and net against gross over one
    /// epoch differ by the system weight across it, so the two names sit as far apart as two
    /// rules do. Nothing on any surface could ask for that comparison.
    #[test]
    fn the_names_a_rule_takes_sweep_the_way_its_numbers_do() {
        let response = run(
            &synthetic(),
            &SpreadRequest {
                base: base_over_an_epoch_impulse(crate::slots::epoch_impulse::NET),
                axes: vec![axis_over_the_convention(vec![
                    crate::slots::epoch_impulse::NET.to_string(),
                    crate::slots::epoch_impulse::GROSS.to_string(),
                ])],
                quantity_key: crate::slots::epoch_impulse::KEY.to_string(),
                maximum_combinations: 512,
            },
        )
        .expect("the convention is a name this rule takes");

        println!(
            "{} of {} succeeded, spread {:?}",
            response.succeeded, response.combinations_run, response.spread_absolute
        );
        assert_eq!(response.combinations_run, 2);
        assert_eq!(response.succeeded, 2);
        assert!(
            response.spread_absolute.is_some_and(|spread| spread > 0.0),
            "the two conventions produced one number: {:?}",
            response.spread_absolute
        );

        // The record names what was compared and what it was compared under, so a reader of
        // the figure can see the set. An axis over names reports its alternatives where an
        // axis over numbers reports its own, because the terminal's grammar calls both a
        // value and a reader of this field is asking how many there were.
        assert_eq!(response.axes_varied.len(), 1);
        assert_eq!(response.axes_varied[0].values_varied, 2);
        assert_eq!(response.axes_varied[0].rules_varied, 0);
        assert_eq!(
            response.axes_varied[0].parameter.as_deref(),
            Some(crate::slots::epoch_impulse::CONVENTION_PARAMETER)
        );
        // The name each variant ran under, so the number carries the choice that produced it.
        assert_eq!(
            response
                .variants
                .iter()
                .map(|variant| variant.settings.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![("convention".to_string(), "gross".to_string())],
                vec![("convention".to_string(), "net".to_string())],
            ]
        );
    }

    /// The names along an axis are a set of choices, and a caller who typed them in another
    /// order was reporting the same sweep. The same property the values already hold.
    #[test]
    fn the_order_the_caller_typed_the_names_in_does_not_reach_the_document() {
        let sweep = |options: Vec<String>| {
            run(
                &synthetic(),
                &SpreadRequest {
                    base: base_over_an_epoch_impulse(crate::slots::epoch_impulse::NET),
                    axes: vec![axis_over_the_convention(options)],
                    quantity_key: crate::slots::epoch_impulse::KEY.to_string(),
                    maximum_combinations: 512,
                },
            )
            .expect("the convention is a name this rule takes")
        };
        let one_way = sweep(vec!["net".to_string(), "gross".to_string()]);
        let the_other = sweep(vec!["gross".to_string(), "net".to_string()]);
        assert!(
            one_way.succeeded > 1,
            "no combination produced a value, so the documents agree about nothing in them"
        );
        assert_eq!(
            serde_json::to_string(&one_way.variants).expect("variants serialise"),
            serde_json::to_string(&the_other.variants).expect("variants serialise"),
        );
    }

    /// An axis stating two sets of alternatives has no width between them, and a precedence
    /// picking one would report every figure over a set the caller did not name.
    #[test]
    fn an_axis_comparing_two_kinds_at_once_is_refused_rather_than_settled_by_precedence() {
        let mut crowded = axis_over_the_convention(vec!["net".to_string(), "gross".to_string()]);
        crowded.values = vec![100.0, 200.0, 300.0];

        let refusal = run(
            &synthetic(),
            &SpreadRequest {
                base: base_over_an_epoch_impulse(crate::slots::epoch_impulse::NET),
                axes: vec![crowded],
                quantity_key: crate::slots::epoch_impulse::KEY.to_string(),
                maximum_combinations: 512,
            },
        )
        .expect_err("an axis comparing two kinds is refused");
        println!("{refusal}");
        assert_eq!(refusal.code, plateforce_core::RefusalCode::ValueNotAccepted);
        assert_eq!(
            refusal.parameter.as_deref(),
            Some("epoch_impulse.convention")
        );
        assert_eq!(refusal.available, vec!["numbers", "names"]);
        // The width is the point: read by a precedence this would have run three
        // combinations or two, and reported its figure over whichever set won.
        assert!(
            refusal.message().contains("one axis compares one of them"),
            "{}",
            refusal.message()
        );
    }

    /// A set of one holds no disagreement, and the two ways a sweep reaches one both used to
    /// publish 0.0 for it.
    ///
    /// The control is the same axis with four values, which has to carry a spread, or a run
    /// that computed nothing at all would satisfy every assertion below.
    #[test]
    fn a_spread_is_withheld_where_one_number_is_all_the_sweep_produced() {
        let sweep = |values: Vec<f64>| {
            run(
                &synthetic(),
                &SpreadRequest {
                    base: base(),
                    axes: vec![Axis {
                        slot: "onset".into(),
                        parameter: Some("k".into()),
                        values,
                        options: Vec::new(),
                        method_ids: Vec::new(),
                    }],
                    quantity_key: "time_to_takeoff_seconds".into(),
                    maximum_combinations: 512,
                },
            )
            .expect("k is a parameter this rule publishes")
        };

        let several = sweep(vec![2.0, 3.0, 5.0, 10.0]);
        assert_eq!(several.succeeded, 4);
        assert!(
            several.spread_absolute.is_some_and(|spread| spread > 0.0),
            "the axis this test compares against moved nothing"
        );
        assert!(several.spread_percent_of_median.is_some());

        let one = sweep(vec![5.0]);
        assert_eq!(one.combinations_run, 1);
        assert_eq!(one.succeeded, 1);
        // The minimum is still a number one combination produced, which is what separates
        // this from a sweep that produced nothing.
        assert!(one.minimum.is_some() && one.median.is_some());
        println!(
            "one combination: spread {:?}, percent {:?}",
            one.spread_absolute, one.spread_percent_of_median
        );
        assert!(
            matches!(
                one,
                SpreadResponse {
                    spread_absolute: None,
                    spread_percent_of_median: None,
                    ..
                }
            ),
            "one combination published a spread over itself"
        );

        // The same set of one reached the other way. Held to the requested width rather than
        // to what was produced, this run would publish 0.0 over a denominator of three.
        let mostly_declined = sweep(vec![5.0, 100_000.0, 200_000.0]);
        assert_eq!(mostly_declined.combinations_run, 3);
        assert_eq!(mostly_declined.succeeded, 1);
        assert_eq!(mostly_declined.failed, 2);
        assert!(
            matches!(
                mostly_declined,
                SpreadResponse {
                    spread_absolute: None,
                    spread_percent_of_median: None,
                    ..
                }
            ),
            "two combinations declined and the third was published as agreeing with itself"
        );
    }

    /// Every rule this build runs for one construct, as one axis, in the order the caller's
    /// own list happens to be in.
    fn axis_over(slot: &str) -> Axis {
        Axis {
            slot: slot.to_string(),
            parameter: None,
            values: Vec::new(),
            options: Vec::new(),
            method_ids: crate::binding::bindings_for(slot)
                .map(|binding| binding.id.to_string())
                .collect(),
        }
    }

    /// What the caller wrote, as the record of the request rather than of the answer, so the
    /// two lists below can be shown to differ before their answers are compared.
    fn as_written(axes: &[Axis]) -> Vec<(String, Vec<String>)> {
        axes.iter()
            .map(|axis| (axis.slot.clone(), axis.method_ids.clone()))
            .collect()
    }

    /// One sweep is one document whichever order the caller listed the rules in, which is
    /// what lets a single record hold the terminal and the browser tab together. The tab
    /// sends rules in the order it ranks them for a reader and the terminal reads the
    /// binding table; on the committed sweep request the two agreed on all 17 other compared
    /// fields and on the 75 labels as a set, and differed in 520 paths of `variants`.
    #[test]
    fn the_order_the_caller_listed_the_rules_in_does_not_reach_the_document() {
        let listed = vec![
            axis_over("weighing"),
            axis_over("onset"),
            axis_over("takeoff"),
        ];
        let mut ranked: Vec<Axis> = listed.iter().rev().cloned().collect();
        for axis in &mut ranked {
            axis.method_ids.reverse();
        }
        assert_ne!(
            as_written(&listed),
            as_written(&ranked),
            "the two callers wrote one list, so what follows compares a request with itself"
        );

        let sweep = |axes: Vec<Axis>| {
            run(
                &synthetic(),
                &SpreadRequest {
                    base: base(),
                    axes,
                    quantity_key: "jump_height_from_takeoff_meters".into(),
                    maximum_combinations: 512,
                },
            )
            .expect("every axis names a construct this request bound")
        };
        let combinations: usize = listed.iter().map(Axis::len).product();
        let from_the_table = sweep(listed);
        let from_a_ranking = sweep(ranked);

        // The population, because two empty lists serialise identically and would satisfy
        // every assertion below without a sweep having run.
        assert!(
            combinations > 1,
            "this build runs one combination, so no order exists to disagree about"
        );
        assert_eq!(from_the_table.variants.len(), combinations);
        assert!(
            from_the_table.succeeded > 0,
            "no combination produced a value, so the documents agree about nothing in them"
        );

        assert_eq!(
            serde_json::to_string(&from_the_table.variants).expect("variants serialise"),
            serde_json::to_string(&from_a_ranking.variants).expect("variants serialise"),
            "one sweep exported from two callers is two documents"
        );
        assert_eq!(from_the_table.axes_varied, from_a_ranking.axes_varied);

        // Ordered by the table rather than merely agreeing with itself. Two callers who
        // reversed each other would agree perfectly on any single fixed order, so what the
        // order IS gets asserted too: the first combination names the first rule the table
        // declares for each construct, and the axes run in the table's order.
        let first_declared: Vec<String> = ["weighing", "onset", "takeoff"]
            .iter()
            .map(|slot| {
                crate::binding::bindings_for(slot)
                    .next()
                    .expect("this build runs a rule for each of the three")
                    .id
                    .to_string()
            })
            .collect();
        assert_eq!(from_the_table.variants[0].method_ids, first_declared);
        assert_eq!(
            from_the_table
                .axes_varied
                .iter()
                .map(|axis| axis.slot.as_str())
                .collect::<Vec<_>>(),
            ["weighing", "onset", "takeoff"]
        );
    }

    /// An axis naming nothing to vary is refused, and the sweep beside it keeps its width.
    ///
    /// Measured on this file rather than supposed. The four-value axis alone ran 4
    /// combinations and reported 0.0158 seconds; with an empty axis beside it the product of
    /// the widths went through zero to one, and the same request ran 1 combination and
    /// reported a spread of zero with no error. The control is that same axis alone, which
    /// has to keep its four, or a refusal that also broke the real sweep would read the same.
    #[test]
    fn an_axis_naming_nothing_to_vary_is_refused_rather_than_collapsing_the_sweep_beside_it() {
        let real = || Axis {
            slot: "onset".into(),
            parameter: Some("k".into()),
            values: vec![2.0, 3.0, 5.0, 10.0],
            options: Vec::new(),
            method_ids: Vec::new(),
        };
        let empty = || Axis {
            slot: "takeoff".into(),
            parameter: None,
            values: Vec::new(),
            options: Vec::new(),
            method_ids: Vec::new(),
        };
        let sweep = |axes: Vec<Axis>| {
            run(
                &synthetic(),
                &SpreadRequest {
                    base: base(),
                    axes,
                    quantity_key: "time_to_takeoff_seconds".into(),
                    maximum_combinations: 512,
                },
            )
        };

        let alone = sweep(vec![real()]).expect("a stated axis sweeps");
        assert_eq!(alone.combinations_run, 4);
        assert!(
            alone.spread_absolute.is_some_and(|spread| spread > 0.0),
            "the axis this test compares against moved nothing"
        );

        let beside =
            sweep(vec![real(), empty()]).expect_err("an axis naming nothing to vary is refused");
        println!("{beside}");
        // The sentence names the axis, so a reader is not sent looking for a rule that
        // published nothing. The generic wording for this code opens on a method id, which
        // is empty here because no rule was asked anything.
        assert!(
            beside
                .message()
                .starts_with("'takeoff' was passed as a sweep axis"),
            "{}",
            beside.message()
        );
        assert_eq!(
            beside.code,
            plateforce_core::RefusalCode::RequiredParameterUnstated
        );
        assert_eq!(beside.parameter.as_deref(), Some("takeoff"));

        // The parameter form, which indexed an empty list rather than returning at all.
        let named = sweep(vec![Axis {
            slot: "onset".into(),
            parameter: Some("k".into()),
            values: Vec::new(),
            options: Vec::new(),
            method_ids: Vec::new(),
        }])
        .expect_err("a parameter named with no values is refused");
        assert_eq!(named.parameter.as_deref(), Some("onset.k"));
    }

    /// The values along a parameter axis are a set of choices too, and a caller who typed
    /// them in another order was reporting the same sweep.
    #[test]
    fn the_order_the_caller_typed_the_values_in_does_not_reach_the_document() {
        let sweep = |values: Vec<f64>| {
            run(
                &synthetic(),
                &SpreadRequest {
                    base: base(),
                    axes: vec![Axis {
                        slot: "onset".into(),
                        parameter: Some("k".into()),
                        values,
                        options: Vec::new(),
                        method_ids: Vec::new(),
                    }],
                    quantity_key: "time_to_takeoff_seconds".into(),
                    maximum_combinations: 512,
                },
            )
            .expect("k is a parameter this rule publishes")
        };
        let ascending = sweep(vec![2.0, 3.0, 5.0, 10.0]);
        let scattered = sweep(vec![5.0, 10.0, 2.0, 3.0]);
        assert_eq!(ascending.variants.len(), 4);
        assert!(
            ascending.succeeded > 0,
            "no combination produced a value, so the documents agree about nothing in them"
        );
        assert_eq!(
            serde_json::to_string(&ascending.variants).expect("variants serialise"),
            serde_json::to_string(&scattered.variants).expect("variants serialise"),
        );
        assert_eq!(ascending.variants[0].label, "k 2");
    }

    /// The reason names a rule the quantity itself says produced it, so a rule declining
    /// elsewhere in the analysis is not written against a number it had no part in.
    ///
    /// One onset rule declines on the run below, and the two quantities differ in whether
    /// they rest on it: the interval is bounded by the onset it did not place, and flight
    /// time is bounded by takeoff and the return to the plate. A field filled from whatever
    /// went wrong anywhere would put the onset refusal on both.
    ///
    /// Flight time answers on that run, so the assertion rests on a quantity that carries a
    /// value rather than on one whose emptiness would satisfy it either way.
    #[test]
    fn a_reason_is_only_written_against_a_quantity_the_declining_rule_produced() {
        let sweep = |quantity: &str| {
            run(
                &synthetic(),
                &SpreadRequest {
                    base: base(),
                    axes: vec![Axis {
                        slot: "onset".into(),
                        parameter: Some("k".into()),
                        values: vec![100_000.0],
                        options: Vec::new(),
                        method_ids: Vec::new(),
                    }],
                    quantity_key: quantity.to_string(),
                    maximum_combinations: 512,
                },
            )
            .unwrap()
        };

        let interval = sweep("time_to_takeoff_seconds");
        assert!(
            interval.variants[0].value.is_none(),
            "this quantity has to be empty here, or the reason below has nothing to explain"
        );
        let reason = interval.variants[0]
            .failure_reason
            .as_ref()
            .expect("the onset rule declined and this interval rests on it");
        assert!(
            reason.method_id.starts_with("onset."),
            "{} is not the rule this quantity rests on",
            reason.method_id
        );

        // The chains are what decide it, so they are what is read, off the same analysis the
        // sweep ran. The interval names the rule that declined and flight time names no onset
        // rule at all, which is why the refusal can reach one and not the other.
        let mut declining = base();
        declining.onset.parameters.insert("k".into(), 100_000.0);
        let response = crate::run(&synthetic(), &declining).expect("the request is well formed");
        let onset_rules_named_by = |key: &str| -> Vec<String> {
            response
                .metrics
                .iter()
                .find(|metric| metric.key == key)
                .map(|metric| {
                    metric
                        .contributing_method_ids
                        .iter()
                        .filter(|id| id.starts_with("onset."))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        assert!(
            !onset_rules_named_by("time_to_takeoff_seconds").is_empty(),
            "the interval names no onset rule, so the pair below is not a comparison"
        );
        assert!(
            onset_rules_named_by("flight_time_seconds").is_empty(),
            "flight time is measured from takeoff to the return to the plate and its chain names {:?}",
            onset_rules_named_by("flight_time_seconds")
        );

        let flight = sweep("flight_time_seconds");
        assert!(
            flight.variants[0].failure_reason.is_none(),
            "the onset rule's refusal was written against a number bounded by takeoff: {:?}",
            flight.variants[0].failure_reason
        );
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
                options: Vec::new(),
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
        assert_eq!(refusal.code, plateforce_core::RefusalCode::UnknownParameter);
        assert_eq!(refusal.parameter.as_deref(), Some("movement_onset.k"));
        assert!(refusal.available.iter().any(|axis| axis == "onset"));
        // The gravity field is an axis a caller can write, so it is listed with the rest
        // rather than mentioned only in the sentence.
        assert!(refusal
            .available
            .iter()
            .any(|axis| axis == "global.gravity_meters_per_second_squared"));
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
        assert_eq!(refusal.code, plateforce_core::RefusalCode::UnknownParameter);
        // The axis is the slot alone when the sweep varies the rule rather than a value.
        assert_eq!(refusal.parameter.as_deref(), Some("movement_onset"));
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

    /// A sweep reports what the method choice moves and, separately, what it moves among
    /// the rules that do not disagree with themselves. A rule that gets takeoff wrong
    /// contributes the size of its own defect to the first figure and nothing to the second.
    #[test]
    fn a_variant_whose_own_routes_disagree_is_counted_and_also_set_aside() {
        let mut request = sweep_over("onset", Some("threshold_n"), vec![20.0, 280.0]);
        request.base.onset.method_id = "onset.threshold.absolute_force".into();
        let trial = super::tests::jump_whose_two_routes_agree();
        let response = run(&trial, &request).expect("the onset axis sweeps");

        for variant in &response.variants {
            println!(
                "{} value {:?} signals {}",
                variant.label,
                variant.value,
                variant.signals.len()
            );
        }
        println!(
            "whole spread {:?} over {}, without signals {:?} over {}, set aside {}",
            response.spread_absolute,
            response.succeeded,
            response.over_rules_without_signals.spread_absolute,
            response.over_rules_without_signals.counted,
            response.excluded_by_a_signal
        );

        let signalling: Vec<&Variant> = response
            .variants
            .iter()
            .filter(|variant| crate::quality::distrusted(&variant.signals))
            .collect();
        assert_eq!(
            signalling.len(),
            1,
            "the start placed 0.28 s late is the one that disagrees with itself"
        );
        assert!(signalling[0].label.contains("280"));

        assert_eq!(response.excluded_by_a_signal, 1);
        assert_eq!(response.over_rules_without_signals.counted, 1);
        assert_eq!(
            response.succeeded,
            response.over_rules_without_signals.counted + response.excluded_by_a_signal,
            "the two populations reconcile against the sweep's own denominator"
        );
        // The whole-sweep figures keep the meaning every surface already reads them under:
        // the defective variant contributes the size of its own defect to the spread.
        assert!(
            response.spread_absolute.is_some_and(|spread| spread > 0.01),
            "the late start moves the height by over a centimetre, read {:?}",
            response.spread_absolute
        );
        // A clean population of one holds all three order statistics and no spread, the
        // same way the whole sweep reports a single combination.
        assert!(response.over_rules_without_signals.median.is_some());
        assert_eq!(response.over_rules_without_signals.spread_absolute, None);
    }
}
