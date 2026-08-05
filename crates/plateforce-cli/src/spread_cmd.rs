//! How far the choice of rule moves a number, on this trial.
//!
//! Five of six course documents ask an undergraduate to compute jump height two or three ways
//! and explain the disagreement. It rides in `analyse`'s own output for the headline quantity,
//! and this command reports it for any other.

use std::fmt::Write as _;

use plateforce_analysis::spread::{run as sweep, Axis, SpreadRequest, SpreadResponse};
use plateforce_analysis::{bindings_for, AnalysisRequest};
use plateforce_core::Trial;

use std::path::Path;

use crate::analyse::PATH;
use crate::decisions::slot_of;
use crate::exit::{Declined, Fault, Outcome};
use crate::out::Format;
use crate::registry_cmd::canonical;
use crate::render::{Renderer, Role};

/// The quantity `analyse` reports the spread for without being asked.
pub const HEADLINE_QUANTITY: &str = "jump_height_from_takeoff_meters";

/// One axis per construct the run bound, holding every rule this build can run for it. The
/// sweep varies the choice a user would otherwise make once and never revisit.
#[derive(Debug, clap::Args)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub analysis: crate::analyse::Args,
    /// The quantity to sweep. Absent takes the one `analyse` reports without being asked
    #[arg(long, value_name = "KEY")]
    pub quantity: Option<String>,
    /// A step to vary, repeated for each. Absent varies every step this run bound more than
    /// one rule for
    #[arg(long, value_name = "STEP")]
    pub slot: Vec<String>,
    #[arg(long = "vary", value_name = "ASSIGNMENT", help = VARY_HELP)]
    pub vary: Vec<String>,
    #[arg(long = "vary-choice", value_name = "ASSIGNMENT", help = VARY_CHOICE_HELP)]
    pub vary_choice: Vec<String>,
}

/// What `--vary` takes, in the grammar `--set` already takes, because a reader who wrote
/// `--set onset.k=5` to bind a value writes `--vary onset.k=2,5` to sweep it.
///
/// A separate flag from `--slot` because the two name different kinds of alternative: one
/// varies which rule runs, the other varies a number inside the rule the run bound. Both can
/// be written on one line, and that is the sweep the engine has always run and no surface
/// could ask for: five onset rules by three values of `k` is `--slot onset --vary
/// onset.k=2,5,10`.
pub(crate) const VARY_SHAPE: &str = "<slot>.<name>=<value>,<value>";

pub(crate) const VARY_HELP: &str =
    "A value to sweep beside or instead of the rule, written <slot>.<name>=<value>,<value>. Repeatable, and `global.gravity_meters_per_second_squared` sweeps gravity";

/// What `--vary-choice` takes. `--choose` binds a name a rule takes and this compares them,
/// in the relation `--vary` has to `--set`.
///
/// A separate flag from `--vary` for the reason `--choose` is separate from `--set`: the kind
/// is known from the line rather than from the rule the alternatives reach. Read off the
/// values, `--vary weighing.duration=fast,slow` would arrive at a rule as two names, and
/// every mistyped number in a numeric sweep with it.
pub(crate) const VARY_CHOICE_SHAPE: &str = "<slot>.<name>=<name>,<name>";

pub(crate) const VARY_CHOICE_HELP: &str =
    "A name to sweep, written <slot>.<name>=<name>,<name>. Repeatable, and `registry show <method>` lists the names each rule takes";

/// The step gravity is written against, which is a value of the run rather than of any rule.
const GLOBAL_STEP: &str = "global";

pub fn run(
    args: &Args,
    registry_directory: Option<&Path>,
    format: Format,
    renderer: &Renderer,
) -> Outcome {
    let prepared = match crate::analyse::prepare(&args.analysis, registry_directory, renderer) {
        Ok(prepared) => prepared,
        Err(outcome) => return outcome,
    };
    let quantity = args.quantity.as_deref().unwrap_or(HEADLINE_QUANTITY);

    // Asked of the analysis rather than of a list kept beside it. A key nothing computes would
    // sweep every combination, fail every one, and report an empty spread with exit 0.
    let reported = match plateforce_analysis::run(&prepared.trial.trial, &prepared.request) {
        Ok(response) => response.metrics,
        Err(refusal) => return Outcome::declined(Declined::recorded(*refusal)),
    };
    if !reported.iter().any(|metric| metric.key == quantity) {
        let names: Vec<&str> = reported.iter().map(|metric| metric.key.as_str()).collect();
        return Outcome::declined_line(
            Fault::Request,
            format!("'{quantity}' is not one of the quantities this run reports: {names:?}"),
        );
    }

    let axes = match axes_asked_for(&prepared.request, &args.slot, &args.vary, &args.vary_choice) {
        Ok(axes) => axes,
        Err(declined) => return Outcome::declined(declined),
    };

    match measure(&prepared.trial.trial, &prepared.request, quantity, axes) {
        Err(refusal) => Outcome::declined(Declined::recorded(*refusal)),
        Ok(response) => {
            // A sweep that leaves on its own says which build and which registry produced it.
            // One inside an analysed result inherits that document's identity, so `describe`
            // renders the panel alone. The pin comes from the same `registry_stamp` `analyse`
            // reads, so the two commands cannot answer it differently.
            let reported = plateforce_analysis::document::SpreadDocument::of(
                env!("CARGO_PKG_VERSION"),
                &crate::analyse::registry_stamp(
                    &prepared.registry,
                    args.analysis.registry_version.clone(),
                ),
                response,
            );
            let document = match format {
                Format::Markdown => return crate::out::markdown_wants_a_result("spread"),
                Format::Json => match serde_json::to_value(&reported) {
                    Ok(value) => canonical(&value),
                    Err(error) => {
                        return Outcome::declined_line(Fault::Internal, format!("{error}"))
                    }
                },
                Format::Text => describe(&reported.spread, renderer),
            };
            Outcome::complete(document)
        }
    }
}

/// Every step this run bound that carries more than one rule, as an axis over those rules.
///
/// The three landmarks and, since a request can bind them, the constructs computed from the
/// landmarks. A sweep that held the rule computing the quantity still reported 3.11 cm on
/// subject 01 trial 1 where the three rules reporting that key span 3.38 cm.
///
/// A construct the request did not bind is not an axis. Sweeping it would run a rule nobody
/// chose, which is what `spread::unsweepable` refuses.
pub fn axes_over_every_rule(request: &AnalysisRequest) -> Vec<Axis> {
    axes_over_every_step(request)
        .into_iter()
        .filter(|axis| axis.method_ids.len() > 1)
        .collect()
}

/// Every step this run bound, as an axis over the rules this build runs for it, whether or
/// not that is more than one.
///
/// The unfiltered set, so a caller who named a step by hand is told this build runs one rule
/// for it rather than told no such step exists.
fn axes_over_every_step(request: &AnalysisRequest) -> Vec<Axis> {
    let landmarks = PATH.iter().map(|construct| slot_of(construct).to_string());
    // Keyed by construct on the request, and the sweep reaches a derived rule by that same
    // word, so the construct is the slot for every one of them. Conditioning is here for the
    // same reason it is a step `--set` accepts: the phase runs on every run, and a rule that
    // shapes the trace the landmark rules read is an alternative in the sense every other
    // rule on this list is.
    let bound = request
        .derived
        .keys()
        .chain(request.conditioning.keys())
        .cloned();
    landmarks
        .chain(bound)
        .map(|slot| Axis {
            method_ids: bindings_for(&slot)
                .map(|binding| binding.id.to_string())
                .collect(),
            slot,
            parameter: None,
            values: Vec::new(),
            options: Vec::new(),
        })
        .collect()
}

/// The steps the caller named, as axes, or a sentence saying why one of them is not a step
/// this run can vary.
///
/// A named step this build runs one rule for is refused rather than dropped. Dropped, the
/// command would run, print a spread taken over the steps it kept, and say nothing about the
/// step the caller asked about, which reads as an answer to the question they put. Python's
/// `slot=` says this in the same words, because the two are the same question asked from two
/// keyboards.
///
/// The word is the one `--set` takes as its prefix, and the construct the panel prints is
/// accepted too, because a reader narrowing a sweep is reading `varied system_weight` off
/// the panel above it. What the record names is the construct either way.
fn axes_over_named_steps(request: &AnalysisRequest, named: &[String]) -> Result<Vec<Axis>, String> {
    let bound = axes_over_every_step(request);
    let offered: Vec<&str> = bound.iter().map(|axis| axis.slot.as_str()).collect();
    let mut chosen: Vec<Axis> = Vec::new();
    for word in named {
        let Some(axis) = bound.iter().find(|axis| answers_to(&axis.slot, word)) else {
            return Err(format!(
                "'{word}' is not a step this run bound: {offered:?}"
            ));
        };
        if axis.method_ids.len() < 2 {
            let runs = match axis.method_ids.len() {
                0 => "no rule",
                _ => "one rule",
            };
            return Err(format!(
                "this analysis runs {runs} for {}, so there is nothing to sweep",
                axis.slot
            ));
        }
        if chosen.iter().any(|held| held.slot == axis.slot) {
            return Err(format!("'{word}' is named twice, and one step is one axis"));
        }
        chosen.push(axis.clone());
    }
    Ok(chosen)
}

/// Whether a step answers to a word a caller typed: the word `--set` takes, or the construct
/// the record and the panel name it by.
fn answers_to(slot: &str, word: &str) -> bool {
    slot == word || plateforce_analysis::binding::construct_for_slot(slot) == Some(word)
}

/// What this run was asked to sweep: rules, values inside the rules it bound, or both at
/// once.
///
/// A sweep varies the choice a reader would otherwise make once. Which rule runs is one such
/// choice, the number the rule reads is another, and the name it reads is a third. `k` moves
/// a jump height 0.01981 m across its six published values on subject 01 trial 1, against
/// 0.01924 m across the five onset rules, so the three are alternatives in the same sense and
/// a reader asking about a number resting on more than one of them asks about all of them at
/// once. `--slot onset --vary onset.k=2,5,10` is that question.
///
/// Naming nothing sweeps every rule this run bound more than one of, which is the question a
/// reader asks first.
fn axes_asked_for(
    request: &AnalysisRequest,
    slots: &[String],
    varied: &[String],
    chosen_among: &[String],
) -> Result<Vec<Axis>, Declined> {
    if slots.is_empty() && varied.is_empty() && chosen_among.is_empty() {
        return Ok(axes_over_every_rule(request));
    }

    let mut axes = if slots.is_empty() {
        Vec::new()
    } else {
        axes_over_named_steps(request, slots)
            .map_err(|sentence| Declined::line(Fault::Request, sentence))?
    };
    for written in varied {
        axes.push(axis_over_a_value(request, written)?);
    }
    for written in chosen_among {
        axes.push(axis_over_a_name(request, written)?);
    }

    // One step and one setting is one axis. Written twice it was two, and the sweep squared
    // its own combinations, each combination binding the setting twice with the second
    // binding winning, so the denominator every figure is reported over counts a set the
    // caller never asked for. The other two surfaces refuse the repeat in these words.
    for position in 0..axes.len() {
        let named = |axis: &Axis| (axis.slot.clone(), axis.parameter.clone());
        if axes[..position]
            .iter()
            .any(|held| named(held) == named(&axes[position]))
        {
            let axis = &axes[position];
            let word = match axis.parameter.as_deref() {
                Some(parameter) => format!("{}.{parameter}", axis.slot),
                None => axis.slot.clone(),
            };
            return Err(Declined::line(
                Fault::Request,
                format!("'{word}' is named twice, and one setting is one axis"),
            ));
        }
    }
    Ok(axes)
}

/// The steps a setting can be written against on this run.
///
/// The ones `--set` accepts, so a value that can be bound can be swept and neither flag
/// reaches a step the other does not. `global` is here and not there because gravity belongs
/// to the run rather than to a rule: it is bound by `--gravity` and swept by name.
fn steps_a_setting_reaches(request: &AnalysisRequest) -> (Vec<String>, Vec<&'static str>) {
    let derived: Vec<String> = request
        .derived
        .keys()
        .chain(request.conditioning.keys())
        .cloned()
        .collect();
    (derived, vec![GLOBAL_STEP])
}

/// One `--vary` read into the axis the engine sweeps a number along.
fn axis_over_a_value(request: &AnalysisRequest, written: &str) -> Result<Axis, Declined> {
    let (bound, extra) = steps_a_setting_reaches(request);
    let mut steps = crate::analyse::steps_of_this_run(&bound);
    steps.extend(extra);

    let (slot, name, stated) =
        crate::analyse::assignment_of("--vary", VARY_SHAPE, written, &steps)?;
    let qualified = format!("{slot}.{name}");

    let mut values = Vec::new();
    for one in stated.split(',') {
        let value: f64 = one.trim().parse().map_err(|_| {
            Declined::line(
                Fault::Request,
                format!("--vary {qualified} was given '{one}', which is not a number"),
            )
        })?;
        if !value.is_finite() {
            return Err(Declined::recorded(
                plateforce_core::Refusal::parameter_not_finite("", qualified.clone(), value),
            ));
        }
        // A value swept twice is a variant paired with a copy of itself, and it would pull the
        // spread toward a number no second rule produced while counting in the denominator.
        if values.contains(&value) {
            return Err(Declined::line(
                Fault::Request,
                format!("--vary {qualified} names {one} twice, and one value is one variant"),
            ));
        }
        values.push(value);
    }

    Ok(Axis {
        slot: slot.to_string(),
        parameter: Some(name.to_string()),
        values,
        options: Vec::new(),
        method_ids: Vec::new(),
    })
}

/// One `--vary-choice` read into the axis the engine sweeps a name along.
///
/// Which names a rule takes is the rule's own to answer, so an unaccepted one is refused by
/// the rule with the list it offers, exactly as `--choose` leaves it. What is checked here is
/// what the line can say without reaching a rule at all: an empty alternative, and a name
/// written twice.
///
/// Gravity is refused because it is a number the run carries and no rule reads, which is the
/// one step `--vary` accepts and this does not.
fn axis_over_a_name(request: &AnalysisRequest, written: &str) -> Result<Axis, Declined> {
    let (bound, _) = steps_a_setting_reaches(request);
    let steps = crate::analyse::steps_of_this_run(&bound);

    let (slot, name, stated) =
        crate::analyse::assignment_of("--vary-choice", VARY_CHOICE_SHAPE, written, &steps)?;
    let qualified = format!("{slot}.{name}");

    let mut options: Vec<String> = Vec::new();
    for one in stated.split(',') {
        let chosen = one.trim().to_string();
        if chosen.is_empty() {
            return Err(Declined::line(
                Fault::Request,
                format!("--vary-choice {qualified} names an empty alternative"),
            ));
        }
        if options.contains(&chosen) {
            return Err(Declined::line(
                Fault::Request,
                format!(
                    "--vary-choice {qualified} names {chosen} twice, and one name is one variant"
                ),
            ));
        }
        options.push(chosen);
    }

    Ok(Axis {
        slot: slot.to_string(),
        parameter: Some(name.to_string()),
        values: Vec::new(),
        options,
        method_ids: Vec::new(),
    })
}

pub fn measure(
    trial: &Trial,
    request: &AnalysisRequest,
    quantity_key: &str,
    axes: Vec<Axis>,
) -> Result<SpreadResponse, Box<plateforce_core::Refusal>> {
    sweep(
        trial,
        &SpreadRequest {
            base: request.clone(),
            axes,
            quantity_key: quantity_key.to_string(),
            maximum_combinations: 512,
        },
    )
}

/// The rules that produced the two ends of the spread, so the figure between them names
/// what it is a disagreement between.
fn extremes(response: &SpreadResponse) -> Vec<(String, String)> {
    let mut valued: Vec<(&str, f64, &[String])> = response
        .variants
        .iter()
        .filter_map(|variant| {
            variant
                .value
                .map(|value| (variant.label.as_str(), value, variant.method_ids.as_slice()))
        })
        .collect();
    if valued.len() < 2 {
        return Vec::new();
    }
    valued.sort_by(|left, right| left.1.total_cmp(&right.1));
    let lowest = valued.first().expect("two or more values");
    let highest = valued.last().expect("two or more values");
    // A sweep over a value inside one rule runs every combination under the same rules, so
    // naming them would print one identical list against each end and call it the
    // disagreement. What differs there is the value, which is what the label carries.
    let ends_run_the_same_rules = lowest.2 == highest.2;
    [("lowest", lowest), ("highest", highest)]
        .into_iter()
        .map(|(end, (label, value, ids))| {
            let named = if ids.is_empty() || ends_run_the_same_rules {
                label.to_string()
            } else {
                ids.join(", ")
            };
            (format!("{end} {value:.4} {}", response.unit_symbol), named)
        })
        .collect()
}

/// The block `analyse` prints under the metrics. Every figure carries what it is taken over:
/// the percentage its median, the count its denominator.
pub fn describe(response: &SpreadResponse, renderer: &Renderer) -> String {
    let mut block = String::new();
    let _ = writeln!(
        block,
        "{}",
        renderer.paint(
            Role::Heading,
            &format!("Method spread, {}", response.quantity_key)
        )
    );

    match (
        response.minimum,
        response.maximum,
        response.median,
        response.spread_percent_of_median,
    ) {
        (Some(minimum), Some(maximum), Some(median), Some(percent)) => {
            let _ = writeln!(
                block,
                "  {minimum:.4} to {maximum:.4} {}, median {median:.4}, {percent:.1} percent of the median",
                response.unit_symbol
            );
        }
        // A sweep that produced no value is not a spread of zero, and printing one would be a
        // number no rule produced.
        _ => {
            let _ = writeln!(block, "  no combination produced a value");
        }
    }

    // A spread is two rules disagreeing, so the two rules are named. Left as a percentage
    // alone it reads as a property of the trial, and a single broken rule inside it reads as
    // a disagreement between sound ones.
    for (label, rules) in extremes(response) {
        let _ = writeln!(block, "  {label}");
        for line in renderer.wrap(&rules, 6) {
            let _ = writeln!(block, "{line}");
        }
    }

    // A spread is a number over a set of choices, and the set is printed beside it.
    //
    // Both kinds of choice. An axis over a rule's own value carries no rules, so a filter on
    // the rule count alone dropped it: `--vary onset.k=2,2.5,3,4,5,8` printed 4.8 percent of
    // the median with three held lines under it and never said what had moved, which is a
    // figure whose provenance omits the choice that produced it.
    let varied: Vec<String> = response
        .axes_varied
        .iter()
        .filter_map(
            |axis| match (axis.rules_varied, axis.parameter.as_deref()) {
                (rules, _) if rules > 1 => Some(format!("{} ({rules} rules)", axis.construct)),
                (_, Some(parameter)) if axis.values_varied > 1 => Some(format!(
                    "{}.{parameter} ({} values)",
                    axis.construct, axis.values_varied
                )),
                _ => None,
            },
        )
        .collect();
    if !varied.is_empty() {
        let _ = writeln!(block, "  varied {}", varied.join(", "));
    }
    for held in &response.held_fixed {
        let sentence = format!(
            "held {} at {}, so this spread is not over it",
            held.construct, held.method_id
        );
        for line in renderer.wrap(&sentence, 2) {
            let _ = writeln!(block, "{line}");
        }
    }

    let _ = write!(
        block,
        "  {} of {} combinations produced a value",
        response.succeeded, response.combinations_run
    );
    if response.capped {
        let _ = write!(
            block,
            ", of {} the rules on this path can make",
            response.combinations_requested
        );
    }
    // A variant that failed stays in the denominator and says why, because a spread taken
    // over the ones that worked is a spread over a set nobody chose.
    for variant in response.variants.iter().filter(|v| v.value.is_none()) {
        let reason = variant
            .failure_reason
            .as_ref()
            .map(|refusal| refusal.message())
            .unwrap_or("no value");
        block.push('\n');
        let lines = renderer.wrap(&format!("{}: {reason}", variant.label), 4);
        let _ = write!(block, "{}", lines.join("\n"));
    }
    block
}
