//! One trace in, every number the bound rules reach, and what produced each of them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use plateforce_analysis::quality::{QualitySignal, QualityStatus};
use plateforce_analysis::{
    bindings_for, AnalysisRequest, AnalysisResponse, BoundMethod, MethodChoice, Metric,
    WeighingChoice, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT,
};
use plateforce_core::signal::{partition_sentinels, Sentinel};
use plateforce_core::{read_delimited_column, Refusal, Trial};
use plateforce_registry::{Registry, Surfacing};

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
    /// A published pipeline to run, which binds the rules and the values its source states
    #[arg(long, value_name = "NAME")]
    pub preset: Option<String>,
    /// A rule for something computed from the landmarks, written <construct>=<method>.
    /// Repeatable
    #[arg(long = "derive", value_name = "ASSIGNMENT")]
    pub derive: Vec<String>,
    #[arg(long = "set", value_name = "ASSIGNMENT", help = SET_HELP)]
    pub set: Vec<String>,
    #[arg(long = "choose", value_name = "ASSIGNMENT", help = CHOOSE_HELP)]
    pub choose: Vec<String>,
    #[arg(long = "place", value_name = "ASSIGNMENT", help = PLACE_HELP)]
    pub place: Vec<String>,
    /// Gravity where the plate stands. Unstated, standard gravity runs and the record says
    /// nobody was asked
    #[arg(long, value_name = "M/S2")]
    pub gravity: Option<f64>,
    /// Cite this registry revision in the result. Unstated, the result names no pinned
    /// revision and reports the one the registry declares for itself
    #[arg(long, value_name = "REVISION")]
    pub registry_version: Option<String>,
    /// Show every value each rule read, including the ones it chose for itself
    #[arg(long)]
    pub provenance: bool,
}

/// What `--set` takes, in the word the method flags already use: a reader who wrote `--onset`
/// writes `--set onset.k=5`.
///
/// One string for the help and the refusals, because a flag whose help describes something
/// the parser does not accept is a silent default wearing a different costume.
pub(crate) const SET_SHAPE: &str = "<slot>.<name>=<value>";

/// The help both commands show for `--set`. `batch` adds what a folder run does with it.
pub(crate) const SET_HELP: &str = "A value for a rule, written <slot>.<name>=<value>. Repeatable";
pub(crate) const SET_HELP_FOR_A_FOLDER: &str =
    "A value for a rule, written <slot>.<name>=<value>. Repeatable, and it applies to every trial in the folder";

/// What `--choose` takes. The same grammar `--set` takes, because a reader who wrote
/// `--set onset.k=5` writes `--choose onset.selection=first`.
///
/// A separate flag rather than a value type inside `--set`, so a value's kind is known from
/// the line rather than from the rule it reaches. `--set weighing.duration=fast` refuses by
/// naming what a number is; a flag taking both could only refuse once the name had reached a
/// rule, and every mistyped number would arrive there as a name.
pub(crate) const CHOOSE_SHAPE: &str = "<slot>.<name>=<value>";

/// Where the names a rule takes are listed, so a reader meets them in one place rather than
/// in whichever refusal they happen to raise first.
pub(crate) const CHOOSE_HELP: &str =
    "A name a rule takes, written <slot>.<name>=<value>. Repeatable, and `registry show <method>` lists the names each rule takes";

/// What `--place` takes. One sample per landmark, in the same assignment grammar `--set` and
/// `--choose` use, with the slot alone on the left because a landmark is one number and a
/// second name for it would be a name the reader has to learn.
pub(crate) const PLACE_SHAPE: &str = "<slot>=<sample>";

pub(crate) const PLACE_HELP: &str =
    "A landmark placed by hand, written <slot>=<sample>, counting samples from zero. Repeatable, and `weighing` places the start of the standing window";

/// The landmarks a reader can place, in the order the analysis meets them.
///
/// Every one of them travels in the record as an override rather than replacing the rule's
/// answer silently, which is why placing one is offered at all.
pub(crate) const PLACEABLE: [&str; 4] = [WEIGHING_SLOT, ONSET_SLOT, TAKEOFF_SLOT, TOUCHDOWN_SLOT];

pub(crate) const WEIGHING_SLOT: &str = "weighing";
pub(crate) const ONSET_SLOT: &str = "onset";
pub(crate) const TAKEOFF_SLOT: &str = "takeoff";

/// Touchdown is the return above the threshold that defined takeoff, so it runs no rule of
/// its own and is not a step a value can be written against. A reader can still say where it
/// is, and the record says they did.
pub(crate) const TOUCHDOWN_SLOT: &str = "touchdown";

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
}

/// Which registry produced these numbers, as the three facts a reader asks for, built in one
/// place so no document written by this surface can assemble its own answer.
///
/// The pin is the caller's word and the declared revision is the registry's. This surface
/// used to publish the second under the first's name, so every unpinned run told a reader the
/// operator had cited a revision no operator had chosen.
pub(crate) fn registry_stamp(
    registry: &Registry,
    args: &Args,
) -> plateforce_core::provenance::RegistryStamp {
    plateforce_core::provenance::RegistryStamp::unpinned(
        registry.declared_version.clone(),
        Some(registry.content_digest.clone()),
    )
    .pinned_to(args.registry_version.clone())
}

/// One home for the path from a command line to a request, so a second command asking a
/// second question of the same trial meets the same decision rail rather than its own.
pub(crate) fn prepare(
    args: &Args,
    registry_directory: Option<&Path>,
    renderer: &Renderer,
) -> Result<Prepared, Outcome> {
    let registry = match crate::registry_source::load(registry_directory) {
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
    let also: Vec<String> = derived.keys().cloned().collect();
    let stated = match stated_parameters(&args.set, &also) {
        Ok(stated) => stated,
        Err(declined) => return Err(Outcome::declined(declined)),
    };
    let named = match stated_options(&args.choose, &also) {
        Ok(named) => named,
        Err(declined) => return Err(Outcome::declined(declined)),
    };
    let placed = match placed_samples(&args.place) {
        Ok(placed) => placed,
        Err(declined) => return Err(Outcome::declined(declined)),
    };

    // A named pipeline is adopted before the decision rail rather than after it: its source
    // published the choices it binds, so a caller who named one has answered them.
    let mut request = build_request(
        &registry,
        &chosen,
        &derived,
        &stated,
        &named,
        &placed,
        args.gravity,
    );
    if let Err(declined) = crate::preset::adopt(&mut request, &registry, args.preset.as_ref()) {
        return Err(Outcome::declined(declined));
    }
    let chosen = crate::preset::methods_in(&request);
    let bound = crate::preset::parameters_in(&request);

    let open = decisions::open(&registry, &PATH, &chosen);
    if !open.is_empty() {
        return Err(Outcome::declined(open_decisions_refusal(&open, renderer)));
    }
    if let Some(declined) = unresolved_parameters(&registry, &chosen, &bound, renderer) {
        return Err(Outcome::declined(declined));
    }

    let trial = read_trial(args)?;
    Ok(Prepared {
        registry,
        trial,
        request,
    })
}

pub fn run(
    args: &Args,
    registry_directory: Option<&Path>,
    format: Format,
    renderer: &Renderer,
) -> Outcome {
    let Prepared {
        registry,
        trial,
        request,
    } = match prepare(args, registry_directory, renderer) {
        Ok(prepared) => prepared,
        Err(outcome) => return outcome,
    };

    match plateforce_analysis::run(&trial.trial, &request) {
        // The record carries the code and the class follows from it. This surface used to
        // ask the binding table a second time for the fault class, because the record that
        // knew the answer was flattened to its sentence one frame earlier.
        Err(refusal) => Outcome::declined(Declined::recorded(*refusal)),
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

/// Which step of this run a qualified name is written against, and which of its parameters.
///
/// Read against the steps the run actually has rather than off the first dot in the name.
/// Three of the fifteen constructs this build binds are named with a dot in them, all three
/// of them jump-height constructs carrying nine rules, and splitting on the first dot reports
/// every value written against one of them as naming a step that does not exist.
///
/// The longest match wins, so a step whose name begins another step's name takes only the
/// names written against itself.
fn slot_and_parameter<'a>(qualified: &'a str, slots: &[&str]) -> Option<(&'a str, &'a str)> {
    slots
        .iter()
        .filter(|slot| qualified.starts_with(*slot) && qualified[slot.len()..].starts_with('.'))
        .max_by_key(|slot| slot.len())
        .map(|slot| (&qualified[..slot.len()], &qualified[slot.len() + 1..]))
        .filter(|(_, name)| !name.is_empty())
}

/// Every step a value can be written against on this run: the three on the path, and any
/// construct this run named for something computed from the landmarks.
fn steps_of_this_run(also: &[String]) -> Vec<&str> {
    let mut slots: Vec<&str> = PATH.iter().map(|c| decisions::slot_of(c)).collect();
    slots.extend(also.iter().map(String::as_str));
    slots
}

/// One assignment read against the steps this run has, for both flags that take one.
///
/// The shape of an assignment is a fault in the line and reaches no rule, so it carries no
/// published code. Two flags rather than one shared grammar function would let the two drift
/// into refusing differently for the same malformed line.
fn assignment_of<'a>(
    flag: &str,
    shape: &str,
    assignment: &'a str,
    slots: &[&str],
) -> Result<(&'a str, &'a str, &'a str), Declined> {
    let Some((qualified, written)) = assignment.split_once('=') else {
        return Err(Declined::line(
            Fault::Request,
            format!("{flag} takes {shape}, and '{assignment}' carries no ="),
        ));
    };
    // Two ways to reach no step, and they are answered differently: a name carrying no step at
    // all is a line the reader will rewrite from the grammar, and a name carrying one this run
    // does not have is a line they will rewrite from the list.
    if !qualified.contains('.') {
        return Err(Declined::line(
            Fault::Request,
            format!("{flag} takes {shape}, and '{qualified}' names no slot"),
        ));
    }
    // A value written against a step this run does not have would otherwise be read, accepted
    // and never passed to anything.
    let Some((slot, name)) = slot_and_parameter(qualified, slots) else {
        return Err(Declined::line(
            Fault::Request,
            format!(
                "{flag} {qualified} names no step of this run, which has {}",
                slots.join(", ")
            ),
        ));
    };
    Ok((slot, name, written))
}

/// One name given two values on one line, refused rather than settled by position.
///
/// The three repeatable flags cannot lean on the parser the way the method flags do: clap
/// refuses a second `--onset` because a run has one onset, and it cannot refuse a second
/// `--set` because a run states many. So a second value for a name a caller has already
/// written was kept and the first was dropped, with nothing recorded anywhere: the result of
/// a caller who wrote both was byte-identical to the result of a caller who wrote only the
/// second. That is this project's founding observation arriving on its own request path, and
/// it reached a knob with a measured 6.6 second failure, `onset.direction`.
///
/// Refused rather than warned, because there is no reading of two values under one name that
/// this software can act on, and P5 of the mission is that it refuses rather than guesses.
pub(crate) fn stated_twice(flag: &str, name: &str, first: &str, second: &str) -> Declined {
    Declined::line(
        Fault::Request,
        format!(
            "{flag} {name} was given '{first}' and then '{second}', and a name takes one value"
        ),
    )
}

/// `--choose`, keyed by the same word `--set` takes. A name a rule offers, never a number.
///
/// Which names a rule takes is the rule's own to answer, so an unaccepted one is refused by
/// the rule with the list it offers rather than checked twice, once here against a copy.
pub(crate) fn stated_options(
    assignments: &[String],
    also: &[String],
) -> Result<BTreeMap<String, BTreeMap<String, String>>, Declined> {
    let slots = steps_of_this_run(also);
    let mut chosen: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for assignment in assignments {
        let (slot, name, written) = assignment_of("--choose", CHOOSE_SHAPE, assignment, &slots)?;
        let named = written.trim();
        if named.is_empty() {
            return Err(Declined::line(
                Fault::Request,
                format!("--choose {slot}.{name} was given no name"),
            ));
        }
        if let Some(first) = chosen
            .entry(slot.to_string())
            .or_default()
            .insert(name.to_string(), named.to_string())
        {
            return Err(stated_twice(
                "--choose",
                &format!("{slot}.{name}"),
                &first,
                named,
            ));
        }
    }
    Ok(chosen)
}

/// `--set`, keyed by the same word the method flag carries, so a reader who wrote `--onset`
/// writes `--set onset.k`. Kept per slot, so two rules reading a name spelled the same way
/// never receive each other's number.
pub(crate) fn stated_parameters(
    assignments: &[String],
    also: &[String],
) -> Result<BTreeMap<String, BTreeMap<String, f64>>, Declined> {
    let slots = steps_of_this_run(also);
    let mut stated: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    // What the caller typed against each qualified name, so a name stated twice reads back as
    // the two lines they wrote. Reporting the parsed numbers instead would answer `1.0` and
    // `1.00` with "given '1' and then '1'", which reads as the software confusing itself.
    let mut as_written: BTreeMap<String, String> = BTreeMap::new();
    for assignment in assignments {
        let (slot, name, written) = assignment_of("--set", SET_SHAPE, assignment, &slots)?;
        let qualified = format!("{slot}.{name}");
        // A number a rule cannot run on is a fault the engine has a code for, and it reaches
        // the same code here as it would from inside a rule.
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
        if stated
            .entry(slot.to_string())
            .or_default()
            .insert(name.to_string(), value)
            .is_some()
        {
            let first = as_written
                .get(&qualified)
                .map(String::as_str)
                .unwrap_or_default();
            return Err(stated_twice("--set", &qualified, first, written.trim()));
        }
        as_written.insert(qualified, written.trim().to_string());
    }
    Ok(stated)
}

/// `--place`, one sample per landmark, keyed by the same slot word the method flags carry.
///
/// A landmark placed twice is refused through `stated_twice`, the sentence `--set`, `--choose`
/// and `--derive` already refuse a repeated name with, so one line means one thing whichever
/// flag wrote it. Two samples for one landmark is a line whose meaning depends on argument
/// order, and the sample that lost would have left no trace in the record.
pub(crate) fn placed_samples(assignments: &[String]) -> Result<BTreeMap<String, usize>, Declined> {
    let mut placed: BTreeMap<String, usize> = BTreeMap::new();
    for assignment in assignments {
        let Some((slot, written)) = assignment.split_once('=') else {
            return Err(Declined::line(
                Fault::Request,
                format!("--place takes {PLACE_SHAPE}, and '{assignment}' carries no ="),
            ));
        };
        let slot = slot.trim();
        if !PLACEABLE.contains(&slot) {
            return Err(Declined::line(
                Fault::Request,
                format!(
                    "--place {slot} names no landmark of this run, which has {}",
                    PLACEABLE.join(", ")
                ),
            ));
        }
        let sample: usize = written.trim().parse().map_err(|_| {
            Declined::line(
                Fault::Request,
                format!(
                    "--place {slot} was given '{written}', which is not a sample index counting from zero"
                ),
            )
        })?;
        if let Some(already) = placed.insert(slot.to_string(), sample) {
            return Err(stated_twice(
                "--place",
                slot,
                &already.to_string(),
                &sample.to_string(),
            ));
        }
    }
    Ok(placed)
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
    // Both failures publish the record rather than a sentence, so a shell can branch on the
    // code and tell a path it cannot open from a file it read and did not understand. The
    // read failure used to reach here as prose with no code at all.
    let text = std::fs::read_to_string(&args.trial).map_err(|error| {
        Outcome::declined(Declined::recorded(Refusal::file_not_read(
            args.trial.display().to_string(),
            error.to_string(),
        )))
    })?;
    // A row with no stated delimiter is one field, so `--column 0` reads a single-column
    // export and any other column refuses by naming the index it wanted.
    let delimiter = args.delimiter.unwrap_or('\u{0}');
    let (values, report) = read_delimited_column(&text, delimiter, args.column)
        .map_err(|error| Outcome::declined(Declined::recorded(Refusal::from(error))))?;

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
        if let Some(first) = chosen.insert(construct.to_string(), method_id.to_string()) {
            return Err(stated_twice("--derive", construct, &first, method_id));
        }
    }
    Ok(chosen)
}

fn build_request(
    registry: &Registry,
    chosen: &BTreeMap<String, String>,
    derived: &BTreeMap<String, String>,
    stated: &BTreeMap<String, BTreeMap<String, f64>>,
    named: &BTreeMap<String, BTreeMap<String, String>>,
    placed: &BTreeMap<String, usize>,
    gravity: Option<f64>,
) -> AnalysisRequest {
    let parameters = |construct: &str| {
        stated
            .get(decisions::slot_of(construct))
            .cloned()
            .unwrap_or_default()
    };
    // A name a reader stated reaches the rule under the same step word its numbers do. The
    // two travel as separate maps because the record keeps them separate, and a fingerprint
    // carries the split.
    let options = |construct: &str| {
        named
            .get(decisions::slot_of(construct))
            .cloned()
            .unwrap_or_default()
    };
    let id = |construct: &str| chosen.get(construct).cloned().unwrap_or_default();
    let at = |slot: &str| placed.get(slot).copied();
    // The value and the claim about where it came from are written together, by the one
    // routine every surface writes a gravity through.
    let (gravity_meters_per_second_squared, gravity_source) =
        plateforce_analysis::gravity_stated(gravity);

    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: id(WEIGHING_CONSTRUCT),
            start_index: at(WEIGHING_SLOT),
            parameters: parameters(WEIGHING_CONSTRUCT),
            options: options(WEIGHING_CONSTRUCT),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: id(ONSET_CONSTRUCT),
            parameters: parameters(ONSET_CONSTRUCT),
            options: options(ONSET_CONSTRUCT),
            manual_index: at(ONSET_SLOT),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: id(TAKEOFF_CONSTRUCT),
            parameters: parameters(TAKEOFF_CONSTRUCT),
            options: options(TAKEOFF_CONSTRUCT),
            manual_index: at(TAKEOFF_SLOT),
            ..Default::default()
        },
        touchdown_index: at(TOUCHDOWN_SLOT),
        gravity_meters_per_second_squared,
        gravity_source,
        registry_backed_ids: backed_ids(registry),
        derived: derived
            .iter()
            .map(|(construct, method_id)| {
                (
                    construct.clone(),
                    MethodChoice {
                        method_id: method_id.clone(),
                        parameters: parameters(construct),
                        options: options(construct),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        ..Default::default()
    }
}

/// What this registry carries, which is the question `registry_entry` answers on every
/// record the run produces.
///
/// Every id, not the ones the caller named. The binding composes operators onto the rule a
/// caller chose, and each of those is an entry in its own right that has to be judged
/// against the same list. A list built from the caller's choices alone reports a published
/// entry as absent from the registry it is filed in.
pub(crate) fn backed_ids(registry: &Registry) -> Vec<String> {
    registry.methods.keys().cloned().collect()
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

    // The shape every surface writes a result in, rather than a second one assembled here.
    // A terminal reporting one result under different field names from an R session is the
    // same defect as two implementations of one method, one layer out from the maths, and
    // this surface's own document carried neither the version that produced the numbers nor
    // the landmarks they rest on.
    let reported = plateforce_analysis::document::ResultDocument::of(
        env!("CARGO_PKG_VERSION"),
        plateforce_analysis::document::TrialSource {
            name: args.trial.display().to_string(),
            rows_read: trial.rows_read,
            sentinel_rows: trial.sentinel_rows,
        },
        &registry_stamp(registry, args),
        // No acquisition block reaches this surface, and a dataset that cannot fill one
        // fingerprints as incomplete rather than as matching.
        false,
        response,
        BTreeMap::new(),
        spread,
    );

    let document = match format {
        Format::Json => match serde_json::to_value(&reported) {
            Ok(value) => canonical(&value),
            Err(error) => {
                return Outcome::declined(Declined::line(Fault::Internal, format!("{error}")))
            }
        },
        // The signals the analysis already computed. Recomputing them here ran the same
        // function over the same response a second time.
        Format::Text => text_body(
            response,
            reported.spread.as_ref(),
            registry,
            args,
            renderer,
            &refusals,
            &response.signals,
        ),
    };

    Outcome {
        document: Some(document),
        refusals,
        fault: None,
        every_requested_quantity_has_a_value: missing.is_empty(),
    }
}

/// The word the record carries for a status, as a line of prose reads it.
///
/// The word comes from the vocabulary rather than from a second match here, so a reader who
/// runs this trace in a terminal and opens the same result in a browser or a notebook meets
/// one word for one status. The separator is the only thing this surface decides, which is
/// why a status added to the vocabulary reaches this surface with no edit at all.
fn status_reads(status: QualityStatus) -> String {
    status.wire_name().replace('_', " ")
}

/// How many decimals it takes to print two figures as the different numbers they are.
///
/// A fixed precision suits the magnitudes its author had in front of them. One decimal for a
/// value and none for a threshold reads a 0.0475 s gap between two instants as "1.2 seconds,
/// past 1 seconds", which is a gap four times the size against a threshold that looks like a
/// round number somebody chose.
///
/// Both figures print at the same precision, because two numbers a reader is asked to compare
/// at different precisions is the same defect one step smaller.
fn decimals_telling_apart(value: f64, threshold: f64) -> usize {
    (1..=4)
        .find(|places| format!("{value:.0$}", places) != format!("{threshold:.0$}", places))
        .unwrap_or(4)
}

/// What the software knows about a number, said where the reader is already looking.
///
/// A value, the threshold it passed, and an action naming the construct whose rule the
/// reader would change. Never a verdict, and never a block at the end of the document,
/// where a reader scanning the values does not go.
///
/// A signal holding no value says which status it is under rather than what became of one
/// comparison. A sentence written for one signal is false of the next one to carry no value,
/// and a reader has no way to tell which they are holding.
fn describe_signal(signal: &QualitySignal, renderer: &Renderer) -> Vec<String> {
    let head = match signal.value {
        Some(value) => format!(
            "{}: {value:.places$} {}, past {:.places$} {}.",
            signal.label,
            signal.unit,
            signal.threshold,
            signal.unit,
            places = decimals_telling_apart(value, signal.threshold)
        ),
        None => format!("{}: {}.", signal.label, status_reads(signal.status)),
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

    // Never behind `--provenance`. A gravity nobody was asked about moved four of the eleven
    // numbers above, and a reader who does not know to ask for the record is the reader that
    // record exists for.
    let _ = writeln!(document);
    let _ = writeln!(
        document,
        "{}",
        renderer.paint(Role::Heading, "Global to this analysis")
    );
    for bound in &response.bound_globals {
        for line in renderer.wrap(&describe_global(bound), 2) {
            let _ = writeln!(document, "{line}");
        }
    }
    let _ = document.pop();
    document
}

/// A value the analysis was bound to, with the word for where it came from.
///
/// The claim is printed beside every one of them rather than only where it is interesting,
/// because which of the two claims is the interesting one is the reader's call, and a record
/// that prints a source only sometimes reads as a record that has none the rest of the time.
fn describe_global(bound: &plateforce_analysis::BoundGlobal) -> String {
    format!(
        "{} = {} {}, {}",
        bound.name,
        bound.value,
        bound.unit_symbol,
        bound.source.wire_name()
    )
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
    let adopted_from = match &bound.preset {
        Some(adopted) => format!(", from {}", adopted.id),
        None => String::new(),
    };
    let row = format!(
        "{}{unfiled}{adopted_from}   {}{}",
        bound.method_id,
        shown.join(", "),
        displaced(bound)
    );
    renderer.wrap(&row, 2)
}

/// What the reader's own value displaced, printed beside the value that ran.
///
/// A reader comparing this result against the pipeline's own paper sees where the two part
/// company without having to look the pipeline up.
fn displaced(bound: &BoundMethod) -> String {
    let Some(adopted) = &bound.preset else {
        return String::new();
    };
    if !adopted.was_overridden() {
        return String::new();
    }
    let stated: Vec<String> = adopted
        .superseded_parameters
        .iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .chain(
            adopted
                .superseded_options
                .iter()
                .map(|(name, value)| format!("{name} = {value}")),
        )
        .collect();
    format!(", and {} states {}", adopted.id, stated.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every step this run has, against a value written for one of them.
    ///
    /// The cases that matter are the three constructs whose own names carry a dot, because
    /// splitting a qualified name on its first one reads `jump_height` out of
    /// `jump_height.takeoff_frame` and reports a step this run has as a step it does not.
    #[test]
    fn a_value_reaches_the_step_it_names_however_many_dots_that_name_carries() {
        let dotted = "jump_height.takeoff_frame";
        let stated = stated_parameters(
            &[
                "onset.k=5".to_string(),
                "jump_height.takeoff_frame.gravity=9.81".to_string(),
            ],
            std::slice::from_ref(&dotted.to_string()),
        )
        .expect("both values name a step this run has");
        println!(
            "steps carrying a value: {:?}",
            stated.keys().collect::<Vec<_>>()
        );
        assert_eq!(stated["onset"]["k"], 5.0);
        assert_eq!(stated[dotted]["gravity"], 9.81);
    }

    /// The control. A step this run does not have is still refused, so the match above is
    /// reading the run's own list rather than accepting whatever it is given.
    #[test]
    fn a_value_naming_a_step_this_run_does_not_have_is_refused() {
        let refused = stated_parameters(
            &["jump_height.standing_frame.gravity=9.81".to_string()],
            &[],
        )
        .expect_err("a step this run does not have is refused");
        println!("{}", refused.terminal());
        assert!(refused.terminal().contains("jump_height.standing_frame"));
    }

    /// A name that carries no dot at all names no step, and the longest-prefix match must not
    /// turn that into a step whose name happens to start the same way.
    #[test]
    fn a_name_carrying_no_step_is_refused_rather_than_matched_by_its_opening() {
        let refused = stated_parameters(&["onset=5".to_string()], &[])
            .expect_err("a name with no step is refused");
        println!("{}", refused.terminal());
        assert!(refused.terminal().contains("onset"));
    }

    /// Two steps where one's name opens the other's, which is the case the longest match
    /// exists for. The shorter step reads `takeoff_frame.gravity` as a parameter name and
    /// takes a value written for its neighbour, and nothing about the shorter name is wrong,
    /// so the run has no way to notice.
    #[test]
    fn a_step_whose_name_opens_another_takes_only_the_values_written_for_itself() {
        let stated = stated_parameters(
            &["jump_height.takeoff_frame.gravity=9.81".to_string()],
            &[
                "jump_height".to_string(),
                "jump_height.takeoff_frame".to_string(),
            ],
        )
        .expect("the value names a step this run has");
        println!(
            "steps carrying a value: {:?}",
            stated.keys().collect::<Vec<_>>()
        );
        assert_eq!(stated["jump_height.takeoff_frame"]["gravity"], 9.81);
        assert!(!stated.contains_key("jump_height"), "{stated:?}");
    }
}
