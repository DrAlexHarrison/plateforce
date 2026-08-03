//! One trace in, every number the bound rules reach, and what produced each of them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use plateforce_analysis::quality::QualitySignal;
use plateforce_analysis::{
    bindings_for, AnalysisRequest, AnalysisResponse, BoundMethod, MethodChoice, Metric,
    WeighingChoice, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
use plateforce_core::signal::{partition_sentinels, Sentinel};
use plateforce_core::{read_delimited_column, Refusal, Trial};
use plateforce_registry::{Registry, Surfacing};
use serde_json::json;

use crate::decisions;
use crate::exit::{Declined, Fault, Outcome};
use crate::out::Format;
use crate::registry_cmd::canonical;
use crate::render::{Renderer, Role};

/// How this file writes a sample it does not have. Vendor exports encode it as a zero, a
/// minus one or a spelled-out gap, and reading one as a measurement is the same defect as
/// dropping it silently: three sentinel rows in 244 moved a published correlation by 0.16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum SentinelConvention {
    #[value(alias = "negative-one")]
    NegativeOne,
    None,
    Zero,
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// The force trace to read
    pub trial: PathBuf,
    /// Which column carries the vertical force, counting from zero
    #[arg(long, value_name = "N")]
    pub column: usize,
    /// Samples per second. These exports do not carry it and it is never guessed
    #[arg(long, value_name = "HZ")]
    pub sample_rate_hz: Option<f64>,
    /// How this file writes a sample it does not have
    #[arg(long, value_enum)]
    pub sentinel: SentinelConvention,
    /// The character between columns. Absent reads each row whole
    #[arg(long, value_name = "CHAR")]
    pub delimiter: Option<char>,
    /// The rule that finds the standing epoch
    #[arg(long, value_name = "METHOD")]
    pub weighing: Option<String>,
    /// The rule that finds the start of the jump
    #[arg(long, value_name = "METHOD")]
    pub onset: Option<String>,
    /// The rule that finds takeoff
    #[arg(long, value_name = "METHOD")]
    pub takeoff: Option<String>,
    /// A rule for something computed from the landmarks, written <construct>=<method>.
    /// Repeatable
    #[arg(long = "derive", value_name = "ASSIGNMENT")]
    pub derive: Vec<String>,
    /// A value for a rule, written <construct>.<name>=<value>. Repeatable
    #[arg(long = "set", value_name = "ASSIGNMENT")]
    pub set: Vec<String>,
    /// Show every value each rule read, including the ones it chose for itself
    #[arg(long)]
    pub provenance: bool,
}

/// The number a file writes where it has no sample, for a reader that takes the value
/// rather than the word.
///
/// One vocabulary for both commands. A word that means one thing under `analyse` and
/// another under `batch` is the flag whose meaning depends on where it appears.
pub(crate) fn marker_value(convention: SentinelConvention) -> Option<f64> {
    match convention {
        SentinelConvention::Zero => Some(0.0),
        SentinelConvention::NegativeOne => Some(-1.0),
        SentinelConvention::None => None,
    }
}

/// The constructs a jump height is reached through, in the order the pipeline runs them.
pub const PATH: [&str; 3] = [WEIGHING_CONSTRUCT, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT];

/// A trial read, a request built, and every choice on the path answered.
pub(crate) struct Prepared {
    pub registry: Registry,
    pub trial: ReadTrial,
    pub request: AnalysisRequest,
    pub chosen: BTreeMap<String, String>,
}

/// One home for the path from a command line to a request, so a second command asking a
/// second question of the same trial meets the same decision rail rather than its own.
pub(crate) fn prepare(
    args: &Args,
    registry_directory: &Path,
    renderer: &Renderer,
) -> Result<Prepared, Outcome> {
    let registry = match Registry::load(registry_directory) {
        Ok(registry) => registry,
        Err(error) => {
            return Err(Outcome::declined(Declined::recorded(
                Refusal::registry_invalid(format!("{error}")),
            )))
        }
    };

    let chosen = chosen_methods(args)?;
    let derived = match derived_methods(args) {
        Ok(derived) => derived,
        Err(declined) => return Err(Outcome::declined(declined)),
    };
    let stated = match stated_parameters(&args.set, &derived.keys().cloned().collect::<Vec<_>>()) {
        Ok(stated) => stated,
        Err(declined) => return Err(Outcome::declined(declined)),
    };

    let open = decisions::open(&registry, &PATH, &chosen);
    if !open.is_empty() {
        return Err(Outcome::declined(open_decisions_refusal(&open, renderer)));
    }
    if let Some(declined) = unresolved_parameters(&registry, &chosen, &stated, renderer) {
        return Err(Outcome::declined(declined));
    }

    let trial = read_trial(args)?;
    let request = build_request(&registry, &chosen, &derived, &stated);
    Ok(Prepared {
        registry,
        trial,
        request,
        chosen,
    })
}

pub fn run(args: &Args, registry_directory: &Path, format: Format, renderer: &Renderer) -> Outcome {
    let Prepared {
        registry,
        trial,
        request,
        chosen,
    } = match prepare(args, registry_directory, renderer) {
        Ok(prepared) => prepared,
        Err(outcome) => return outcome,
    };

    match plateforce_analysis::run(&trial.trial, &request) {
        // The engine writes the sentence. Which class of fault it is, is a question about
        // the request, so it is answered by asking the binding table rather than by reading
        // the sentence back apart.
        Err(message) => Outcome::declined_line(fault_of(&chosen), message),
        Ok(response) => {
            let spread = crate::spread_cmd::measure(
                &trial.trial,
                &request,
                crate::spread_cmd::HEADLINE_QUANTITY,
            );
            render(
                &response,
                spread.ok(),
                &trial,
                &registry,
                args,
                format,
                renderer,
            )
        }
    }
}

pub(crate) struct ReadTrial {
    pub trial: Trial,
    pub rows_read: usize,
    pub sentinel_rows: usize,
}

/// Every method the request named, keyed by the construct it fills, refused when the id has
/// no rule behind it rather than served by the nearest neighbour.
fn chosen_methods(args: &Args) -> Result<BTreeMap<String, String>, Outcome> {
    let mut chosen = BTreeMap::new();
    for (construct, given) in [
        (WEIGHING_CONSTRUCT, &args.weighing),
        (ONSET_CONSTRUCT, &args.onset),
        (TAKEOFF_CONSTRUCT, &args.takeoff),
    ] {
        let Some(id) = given else { continue };
        let slot = decisions::slot_of(construct);
        if !bindings_for(slot).any(|binding| binding.id == id) {
            let available: Vec<String> = bindings_for(slot)
                .map(|binding| binding.id.to_string())
                .collect();
            return Err(Outcome::declined(Declined::recorded(
                Refusal::method_not_implemented(id, construct, available),
            )));
        }
        chosen.insert(construct.to_string(), id.clone());
    }
    Ok(chosen)
}

/// `--set <slot>.<name>=<value>`, keyed by the same word the method flag carries, so a reader
/// who wrote `--onset` writes `--set onset.k`. Kept per slot, so two rules reading a name
/// spelled the same way never receive each other's number.
pub(crate) fn stated_parameters(
    assignments: &[String],
    also: &[String],
) -> Result<BTreeMap<String, BTreeMap<String, f64>>, Declined> {
    let mut slots: Vec<&str> = PATH.iter().map(|c| decisions::slot_of(c)).collect();
    // A construct this run named for something computed from the landmarks is a step this
    // run has, so a value written against it reaches a rule.
    slots.extend(also.iter().map(String::as_str));
    let mut stated: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    // The shape of the assignment is a fault in the line and reaches no rule, so it carries
    // no published code. A number a rule cannot run on is a fault the engine has a code for,
    // and it reaches the same code here as it would from inside a rule.
    for assignment in assignments {
        let Some((qualified, written)) = assignment.split_once('=') else {
            return Err(Declined::line(
                Fault::Request,
                format!("--set takes <slot>.<name>=<value>, and '{assignment}' carries no ="),
            ));
        };
        let Some((slot, name)) = qualified.split_once('.') else {
            return Err(Declined::line(
                Fault::Request,
                format!("--set takes <slot>.<name>=<value>, and '{qualified}' names no slot"),
            ));
        };
        // A value written against a step this run does not have would otherwise be read,
        // accepted and never passed to anything.
        if !slots.contains(&slot) {
            return Err(Declined::line(
                Fault::Request,
                format!("--set {qualified} names the step '{slot}', and this run has {slots:?}"),
            ));
        }
        let value: f64 = written.trim().parse().map_err(|_| {
            Declined::line(
                Fault::Request,
                format!("--set {qualified} was given '{written}', which is not a number"),
            )
        })?;
        // Named by the step and the parameter together, because no rule is bound yet and a
        // slot written into the record's method id would be a word nobody can look up.
        if !value.is_finite() {
            return Err(Declined::recorded(Refusal::parameter_not_finite(
                "", qualified, value,
            )));
        }
        stated
            .entry(slot.to_string())
            .or_default()
            .insert(name.to_string(), value);
    }
    Ok(stated)
}

/// A choice the registry forces, stated once as the record and once as the terminal's layout.
///
/// The record names the constructs still open; the screen names the rules each one can be
/// answered with and what the literature publishes for them.
pub(crate) fn open_decisions_refusal(
    open: &[decisions::OpenDecision],
    renderer: &Renderer,
) -> Declined {
    let outstanding: Vec<String> = open
        .iter()
        .map(|decision| decision.construct.clone())
        .collect();
    Declined::shown_as(
        Refusal::decision_not_made("this result", outstanding),
        decisions::describe(open, PATH.len(), renderer),
    )
}

/// A parameter the bound rule requires that the literature publishes more than one value for.
///
/// The unit of resolution is the parameter, so a rule chosen by name still leaves a choice
/// open when the number behind it was published several ways.
fn unresolved_parameters(
    registry: &Registry,
    chosen: &BTreeMap<String, String>,
    stated: &BTreeMap<String, BTreeMap<String, f64>>,
    renderer: &Renderer,
) -> Option<Declined> {
    let empty = BTreeMap::new();
    let mut lines = Vec::new();
    let mut outstanding = Vec::new();
    for (construct, method_id) in chosen {
        let slot = decisions::slot_of(construct);
        let open = decisions::open_parameters(
            registry,
            construct,
            method_id,
            stated.get(slot).unwrap_or(&empty),
        );
        for (name, published) in open {
            outstanding.push(format!("{slot}.{name}"));
            let values: Vec<String> = published.iter().map(|value| format!("{value:?}")).collect();
            lines.push(format!("  --set {slot}.{name}=<VALUE>"));
            lines.extend(renderer.wrap(
                &format!("{method_id} was published at {}", values.join(", ")),
                6,
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    let mut terminal = format!(
        "{} values on the path to a jump height are published more than one way and were not named.\n",
        outstanding.len(),
    );
    terminal.push_str(&lines.join("\n"));
    Some(Declined::shown_as(
        Refusal::decision_not_made("this result", outstanding),
        terminal,
    ))
}

fn read_trial(args: &Args) -> Result<ReadTrial, Outcome> {
    let Some(sample_rate_hz) = args.sample_rate_hz else {
        return Err(Outcome::declined_line(
            Fault::Request,
            format!(
                "{} carries no sample rate, so --sample-rate-hz names it. Reading a 1200 Hz recording as 1000 Hz scales every velocity, displacement and impulse by a fifth",
                args.trial.display()
            ),
        ));
    };
    let text = std::fs::read_to_string(&args.trial).map_err(|error| {
        Outcome::declined_line(
            Fault::Request,
            format!("{} cannot be read: {error}", args.trial.display()),
        )
    })?;
    // A row with no stated delimiter is one field, so `--column 0` reads a single-column
    // export and any other column refuses by naming the index it wanted.
    let delimiter = args.delimiter.unwrap_or('\u{0}');
    let (values, report) = read_delimited_column(&text, delimiter, args.column)
        .map_err(|error| Outcome::declined_line(Fault::Recording, format!("{error}")))?;

    let sentinel = match args.sentinel {
        SentinelConvention::Zero => Some(Sentinel::Zero),
        SentinelConvention::NegativeOne => Some(Sentinel::NegativeOne),
        SentinelConvention::None => None,
    };
    let sentinel_rows = sentinel
        .map(|convention| partition_sentinels(&values, convention).1.len())
        .unwrap_or(0);

    let trial = Trial::new(values, sample_rate_hz)
        .map_err(|error| Outcome::declined(Declined::recorded(Refusal::from(error))))?;
    Ok(ReadTrial {
        trial,
        rows_read: report.rows_read,
        sentinel_rows,
    })
}

/// `--derive <construct>=<method>`, refused when the construct runs no rule or the id is not
/// one of the rules filed under it. Both halves, because either alone lets a request through
/// that the engine would have to refuse later or, worse, skip.
fn derived_methods(args: &Args) -> Result<BTreeMap<String, String>, Declined> {
    let mut chosen = BTreeMap::new();
    for assignment in &args.derive {
        let Some((construct, method_id)) = assignment.split_once('=') else {
            return Err(Declined::line(
                Fault::Request,
                format!("--derive takes <construct>=<method>, and '{assignment}' carries no ="),
            ));
        };
        let runs = plateforce_analysis::binding::derived_constructs();
        if !runs.contains(&construct) {
            return Err(Declined::recorded(Refusal::construct_not_on_the_path(
                construct,
                runs.into_iter().map(str::to_string).collect(),
            )));
        }
        let available: Vec<String> =
            plateforce_analysis::binding::bindings_for_construct(construct)
                .map(|binding| binding.id.to_string())
                .collect();
        if !available.iter().any(|id| id == method_id) {
            return Err(Declined::recorded(Refusal::method_not_implemented(
                method_id, construct, available,
            )));
        }
        chosen.insert(construct.to_string(), method_id.to_string());
    }
    Ok(chosen)
}

fn build_request(
    registry: &Registry,
    chosen: &BTreeMap<String, String>,
    derived: &BTreeMap<String, String>,
    stated: &BTreeMap<String, BTreeMap<String, f64>>,
) -> AnalysisRequest {
    let parameters = |construct: &str| {
        stated
            .get(decisions::slot_of(construct))
            .cloned()
            .unwrap_or_default()
    };
    let id = |construct: &str| chosen.get(construct).cloned().unwrap_or_default();
    let backed: Vec<String> = chosen
        .values()
        .chain(derived.values())
        .filter(|method_id| registry.methods.contains_key(*method_id))
        .cloned()
        .collect();

    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: id(WEIGHING_CONSTRUCT),
            start_index: None,
            parameters: parameters(WEIGHING_CONSTRUCT),
            options: BTreeMap::new(),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: id(ONSET_CONSTRUCT),
            parameters: parameters(ONSET_CONSTRUCT),
            options: BTreeMap::new(),
            manual_index: None,
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: id(TAKEOFF_CONSTRUCT),
            parameters: parameters(TAKEOFF_CONSTRUCT),
            options: BTreeMap::new(),
            manual_index: None,
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared:
            plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: backed,
        derived: derived
            .iter()
            .map(|(construct, method_id)| {
                (
                    construct.clone(),
                    MethodChoice {
                        method_id: method_id.clone(),
                        parameters: parameters(construct),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        ..Default::default()
    }
}

/// A rule that produced nothing, as the record the engine hands back.
///
/// One arm, because every decline now carries a code the rule itself chose. This surface
/// used to publish a sentence with no code for the refusals that arrived as prose, on the
/// reasoning that a code chosen here would name a failure no other surface can raise. That
/// reasoning was right and the remedy sat one layer down: the rule says which code it is
/// declining under, and every surface reads the same one.
fn declined_landmark(declined: &plateforce_analysis::DeclinedRule) -> Declined {
    Declined::recorded(plateforce_analysis::document::refusal_from_rule(declined))
}

/// The construct a refusal happened under, on every row. A record built without one is a
/// fault the engine records no construct for, and the caller still has to know which
/// landmark declined.
fn with_slot(slot: &str, mut record: serde_json::Value) -> serde_json::Value {
    if let Some(object) = record.as_object_mut() {
        if object.get("slot").is_none_or(serde_json::Value::is_null) {
            object.insert("slot".to_string(), json!(slot));
        }
    }
    record
}

/// A request naming a rule this build cannot run asked for something not on offer; one whose
/// rules all exist and still produced nothing met a recording without what they look for.
fn fault_of(chosen: &BTreeMap<String, String>) -> Fault {
    let unbound = chosen.iter().any(|(construct, method_id)| {
        !bindings_for(decisions::slot_of(construct)).any(|binding| binding.id == method_id)
    });
    if unbound {
        Fault::Request
    } else {
        Fault::Recording
    }
}

fn render(
    response: &AnalysisResponse,
    spread: Option<plateforce_analysis::spread::SpreadResponse>,
    trial: &ReadTrial,
    registry: &Registry,
    args: &Args,
    format: Format,
    renderer: &Renderer,
) -> Outcome {
    let missing: Vec<&Metric> = response
        .metrics
        .iter()
        .filter(|metric| metric.value.is_none())
        .collect();
    let refusals: Vec<Declined> = response.refusals.iter().map(declined_landmark).collect();
    let recorded: Vec<serde_json::Value> = response
        .refusals
        .iter()
        .zip(&refusals)
        .map(|(rule, declined)| with_slot(rule.construct, declined.record()))
        .collect();
    // What the software already knows about a number it is about to print. A value the
    // browser flags and the pipe does not is a confident wrong number reaching a paper.
    let signals = plateforce_analysis::quality::signals(response);

    let document = match format {
        Format::Json => canonical(&json!({
            "trial": args.trial.display().to_string(),
            "rows_read": trial.rows_read,
            "sentinel_rows": trial.sentinel_rows,
            "registry_digest": registry.content_digest,
            "metrics": response.metrics,
            "bound_methods": response.bound_methods,
            "levels": response.levels,
            "warnings": response.warnings,
            "refusals": recorded,
            "signals": signals,
            "spread": spread,
        })),
        Format::Text => text_body(
            response,
            spread.as_ref(),
            registry,
            args,
            renderer,
            &refusals,
            &signals,
        ),
    };

    Outcome {
        document: Some(document),
        refusals,
        fault: None,
        every_requested_quantity_has_a_value: missing.is_empty(),
    }
}

/// What the software knows about a number, said where the reader is already looking.
///
/// A value, the threshold it passed, and an action naming the construct whose rule the
/// reader would change. Never a verdict, and never a block at the end of the document,
/// where a reader scanning the values does not go.
fn describe_signal(signal: &QualitySignal, renderer: &Renderer) -> Vec<String> {
    let head = match signal.value {
        Some(value) => format!(
            "{}: {:.1} {}, past {:.0} {}.",
            signal.label, value, signal.unit, signal.threshold, signal.unit
        ),
        None => format!("{}: not comparable.", signal.label),
    };
    renderer.wrap(&format!("{head} {}", signal.remedy), 6)
}

fn text_body(
    response: &AnalysisResponse,
    spread: Option<&plateforce_analysis::spread::SpreadResponse>,
    registry: &Registry,
    args: &Args,
    renderer: &Renderer,
    refusals: &[Declined],
    signals: &[QualitySignal],
) -> String {
    let mut document = String::new();
    let widest = response
        .metrics
        .iter()
        .map(|metric| metric.label.chars().count())
        .max()
        .unwrap_or(0);

    let mut said: Vec<usize> = Vec::new();
    for metric in &response.metrics {
        match metric.value {
            Some(value) => {
                let _ = writeln!(
                    document,
                    "  {:<widest$}  {:>12.4} {}",
                    metric.label, value, metric.unit_symbol
                );
            }
            // A rule that ran correctly and found nothing is its own state, and it is not
            // an empty cell.
            None => {
                let _ = writeln!(
                    document,
                    "  {:<widest$}  {:>12} {}",
                    metric.label, "no value", metric.unit_symbol
                );
            }
        }
        // A signal qualifying several metrics is said once, under the first of them to
        // appear, rather than repeated under each.
        for (index, signal) in signals.iter().enumerate() {
            if signal.qualifies.iter().any(|key| *key == metric.key) && !said.contains(&index) {
                said.push(index);
                for line in describe_signal(signal, renderer) {
                    let _ = writeln!(document, "{line}");
                }
            }
        }
    }
    for refusal in refusals {
        for line in renderer.wrap(refusal.terminal(), 2) {
            let _ = writeln!(document, "{line}");
        }
    }
    for warning in &response.warnings {
        for line in renderer.wrap(warning, 2) {
            let _ = writeln!(document, "{line}");
        }
    }

    // The spread is the second thing this command shows and it is never behind a flag.
    if let Some(spread) = spread {
        let _ = writeln!(document);
        let _ = writeln!(
            document,
            "{}",
            crate::spread_cmd::describe(spread, renderer)
        );
    }

    let _ = writeln!(document);
    let _ = writeln!(document, "{}", renderer.paint(Role::Heading, "Rules"));
    for bound in &response.bound_methods {
        for line in describe_bound(bound, registry, args.provenance, renderer) {
            let _ = writeln!(document, "{line}");
        }
    }
    let _ = document.pop();
    document
}

/// What a rule was bound to. A value the rule chose for itself under an entry the registry
/// hides is recorded rather than shown, so a reader sees what they decided and an export
/// still carries what the software decided.
fn describe_bound(
    bound: &BoundMethod,
    registry: &Registry,
    provenance: bool,
    renderer: &Renderer,
) -> Vec<String> {
    let hidden = registry
        .methods
        .get(&bound.method_id)
        .and_then(|method| method.gui.as_ref())
        .is_some_and(|gui| gui.surfacing == Surfacing::DefaultAndHide);

    let shown: Vec<String> = bound
        .bound_parameters
        .iter()
        .filter(|(name, _)| {
            let assumed = matches!(
                bound.parameter_sources.get(name),
                Some(plateforce_core::provenance::ParameterSource::Assumed)
            );
            provenance || !(hidden && assumed)
        })
        .map(|(name, value)| format!("{name} = {value}"))
        .collect();

    let unfiled = if registry.methods.contains_key(&bound.method_id) {
        ""
    } else {
        ", not filed in the registry under this id"
    };
    let row = format!("{}{unfiled}   {}", bound.method_id, shown.join(", "));
    renderer.wrap(&row, 2)
}
