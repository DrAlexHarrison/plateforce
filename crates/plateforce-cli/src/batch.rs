//! A folder of trials in, one table out, and the record of what produced every number.
//!
//! Two entry points over the same set rather than one with a flag. `analyse` runs one rule
//! per construct and returns a row per trial; `compare` sweeps several rules for one quantity
//! and returns a row per trial per rule. They answer different questions and a caller asking
//! for one never silently receives the other.

use std::path::PathBuf;

use plateforce_batch::agreement::BatchCompareRequest;
use plateforce_batch::{
    analyse, compare, BatchRequest, Rendering, SourceFormat, TrialIdentity, TrialSet,
};
use plateforce_core::Refusal;
use plateforce_registry::Registry;

use crate::exit::{fault_for, Declined, Fault, Outcome};
use crate::out::Format;

/// Which of the two questions this run is asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum Mode {
    /// One rule per construct, one row per trial.
    Analyse,
    /// Several rules for one quantity, one row per trial per rule.
    Compare,
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// The folder of force traces to read
    pub trials: PathBuf,
    /// The folder the tables and the record are written to
    #[arg(long = "out-dir", value_name = "DIR")]
    pub out_dir: PathBuf,
    /// Which question this run is asking
    #[arg(long, value_enum, default_value = "analyse")]
    pub mode: Mode,
    /// Which file names in the folder are trials. Repeatable, and never guessed
    #[arg(long = "trial-suffix", value_name = "SUFFIX", required = true)]
    pub trial_suffixes: Vec<String>,
    /// Which column carries the vertical force, counting from zero
    #[arg(long, value_name = "N")]
    pub column: usize,
    /// Samples per second. These exports do not carry it and it is never guessed
    #[arg(long, value_name = "HZ")]
    pub sample_rate_hz: f64,
    /// The character between columns
    #[arg(long, value_name = "CHAR")]
    pub delimiter: Option<char>,
    /// How these files write a sample they do not have. One answer covers every trial in the
    /// folder, and a marker read as force moves system weight and everything after it
    #[arg(long, value_enum)]
    pub sentinel: crate::analyse::SentinelConvention,
    /// A template such as AT{subject}_{trial}, which gives the run a subject as well
    #[arg(long, value_name = "TEMPLATE")]
    pub pattern: Option<String>,
    /// The rule that finds the standing epoch
    #[arg(long, value_name = "METHOD_ID")]
    pub weighing: Option<String>,
    /// The rule that finds the start of the jump
    #[arg(long, value_name = "METHOD_ID")]
    pub onset: Option<String>,
    /// The rule that finds takeoff
    #[arg(long, value_name = "METHOD_ID")]
    pub takeoff: Option<String>,
    /// A published pipeline to run over every trial in the folder
    #[arg(long, value_name = "NAME")]
    pub preset: Option<String>,
    /// A rule for something computed from the landmarks, written <CONSTRUCT>=<METHOD_ID>.
    /// Repeatable, and it applies to every trial in the folder
    #[arg(long = "derive", value_name = "ASSIGNMENT")]
    pub derive: Vec<String>,
    #[arg(long = "set", value_name = "ASSIGNMENT", help = crate::analyse::SET_HELP_FOR_A_FOLDER)]
    pub set: Vec<String>,
    #[arg(long = "choose", value_name = "ASSIGNMENT", help = crate::analyse::CHOOSE_HELP)]
    pub choose: Vec<String>,
    /// A rule to sweep against the bound one, for compare. Repeatable
    #[arg(long = "against", value_name = "METHOD_ID")]
    pub against: Vec<String>,
    /// The quantity a compare run sweeps
    #[arg(
        long,
        value_name = "KEY",
        default_value = "jump_height_from_takeoff_meters"
    )]
    pub quantity: String,
    /// Cite this registry revision in the record. Unstated, the record names no pinned
    /// revision and reports the one the registry declares for itself
    #[arg(long, value_name = "REVISION")]
    pub registry_version: Option<String>,
    #[arg(long = "acquisition", value_name = "ASSIGNMENT", help = crate::acquisition_arg::ACQUISITION_HELP)]
    pub acquisition: Vec<String>,
    /// Hide the fingerprint column in the printed table. The record is written either way
    #[arg(long)]
    pub without_provenance: bool,
}

pub fn run(
    args: &Args,
    registry_directory: Option<&std::path::Path>,
    format: Format,
    document_destination: Option<&std::path::Path>,
    renderer: &crate::render::Renderer,
) -> Outcome {
    // The global flag names one file, and a run has no single document to put in one.
    if document_destination.is_some() {
        return Outcome::declined_line(
            Fault::Request,
            "a run writes a table, its chain, its refusals and the record beside them, so --out-dir names the folder they go in".to_string(),
        );
    }
    let out_dir = args.out_dir.as_path();
    // What makes a path a file is asked of the filesystem rather than inferred from a dot in
    // the name: `run-2026-08-02.v2` is a folder, and a check that read it as a file would
    // refuse a run for the shape of its name.
    match std::fs::metadata(out_dir) {
        Ok(found) if found.is_dir() => {}
        Ok(_) => {
            return Outcome::declined_line(
                Fault::Request,
                format!(
                    "{} is a file, and a run writes a table, its chain, its refusals and the record beside them, so --out-dir names a folder",
                    out_dir.display()
                ),
            )
        }
        Err(_) => {
            if let Err(error) = std::fs::create_dir_all(out_dir) {
                return Outcome::declined_line(
                    Fault::Request,
                    format!("{} cannot be made: {error}", out_dir.display()),
                );
            }
        }
    }

    let registry = match crate::registry_source::load(registry_directory) {
        Ok(registry) => registry,
        Err(error) => {
            return Outcome::declined(Declined::recorded(Refusal::registry_invalid(
                error.to_string(),
            )))
        }
    };

    let format_declaration = SourceFormat {
        delimiter: args.delimiter.unwrap_or('\u{0}'),
        force_column_index: args.column,
        sample_rate_hz: args.sample_rate_hz,
        trial_file_suffixes: args.trial_suffixes.clone(),
        sentinel: crate::analyse::marker_value(args.sentinel),
    };
    let identity = match &args.pattern {
        Some(template) => TrialIdentity::DeclaredPattern {
            template: template.clone(),
        },
        None => TrialIdentity::FileStem,
    };

    let set = match TrialSet::walk(&args.trials, &format_declaration, &identity) {
        Ok(set) => set,
        Err(error) => return Outcome::declined_line(Fault::Recording, error.to_string()),
    };

    // A run over a folder multiplies one unmade choice by the trial count, so it is refused
    // before a single trial is read, and it is refused the way one trial is: by naming the
    // choice and what can be passed, rather than by naming the flag that is missing.
    let derived = match derived_methods(&args.derive) {
        Ok(derived) => derived,
        Err(declined) => return Outcome::declined(declined),
    };
    // Values written against a construct this run named for something computed from the
    // landmarks, so `--set peak_force.window_seconds` reaches the rule under the same word
    // `--derive` bound it by.
    let also: Vec<String> = derived.keys().cloned().collect();
    let stated = match crate::analyse::stated_parameters(&args.set, &also) {
        Ok(stated) => stated,
        Err(declined) => return Outcome::declined(declined),
    };
    let named = match crate::analyse::stated_options(&args.choose, &also) {
        Ok(named) => named,
        Err(declined) => return Outcome::declined(declined),
    };
    let mut built = request_for(args, &registry, &derived, &stated, &named);
    if let Err(declined) = crate::preset::adopt(&mut built, &registry, args.preset.as_ref()) {
        return Outcome::declined(declined);
    }
    let chosen = crate::preset::methods_in(&built);
    let open = crate::decisions::open(&registry, &crate::analyse::PATH, &chosen);
    if !open.is_empty() {
        return Outcome::declined(crate::analyse::open_decisions_refusal(&open, renderer));
    }

    // Stated once for the folder rather than per file, because a trace of forces carries none
    // of it and every file in one folder came off one plate on one day.
    let acquisition = match crate::acquisition_arg::stated_acquisition(&args.acquisition) {
        Ok(acquisition) => acquisition,
        Err(declined) => return Outcome::declined(declined),
    };

    let resolved: Vec<&str> = chosen.keys().map(String::as_str).collect();
    let request = BatchRequest::new(built)
        .resolving(&resolved)
        .pinned_to(args.registry_version.clone())
        .describing(acquisition);

    match args.mode {
        Mode::Analyse => run_analyse(out_dir, args, &set, &request, &registry, format),
        Mode::Compare => run_compare(out_dir, args, &set, request, &registry, format),
    }
}

/// One request for every trial in the folder, carrying the values the operator stated and
/// the ids the registry backs.
fn request_for(
    args: &Args,
    registry: &plateforce_registry::Registry,
    derived: &std::collections::BTreeMap<String, String>,
    stated: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    named: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
) -> plateforce_analysis::AnalysisRequest {
    let parameters = |construct: &str| {
        stated
            .get(crate::decisions::slot_of(construct))
            .cloned()
            .unwrap_or_default()
    };
    let options = |construct: &str| {
        named
            .get(crate::decisions::slot_of(construct))
            .cloned()
            .unwrap_or_default()
    };
    plateforce_analysis::AnalysisRequest {
        weighing: plateforce_analysis::WeighingChoice {
            method_id: args.weighing.clone().unwrap_or_default(),
            parameters: parameters(plateforce_analysis::WEIGHING_CONSTRUCT),
            options: options(plateforce_analysis::WEIGHING_CONSTRUCT),
            ..Default::default()
        },
        onset: plateforce_analysis::MethodChoice {
            method_id: args.onset.clone().unwrap_or_default(),
            parameters: parameters(plateforce_analysis::ONSET_CONSTRUCT),
            options: options(plateforce_analysis::ONSET_CONSTRUCT),
            ..Default::default()
        },
        takeoff: plateforce_analysis::MethodChoice {
            method_id: args.takeoff.clone().unwrap_or_default(),
            parameters: parameters(plateforce_analysis::TAKEOFF_CONSTRUCT),
            options: options(plateforce_analysis::TAKEOFF_CONSTRUCT),
            ..Default::default()
        },
        touchdown_index: None,
        gravity_meters_per_second_squared:
            plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: crate::analyse::backed_ids(registry),
        // A construct computed from the landmarks has no named field, so the values and the
        // names written against it are keyed by the construct itself rather than by a slot
        // word, which is what `--set` and `--choose` were handed alongside the three steps.
        derived: derived
            .iter()
            .map(|(construct, method_id)| {
                (
                    construct.clone(),
                    plateforce_analysis::MethodChoice {
                        method_id: method_id.clone(),
                        parameters: stated.get(construct).cloned().unwrap_or_default(),
                        options: named.get(construct).cloned().unwrap_or_default(),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        ..Default::default()
    }
}

/// `--derive <construct>=<method>`, read against what this build runs.
///
/// A construct written twice is refused through the same helper `--set` and `--choose` refuse
/// through, so the three repeatable flags on this command answer one question one way.
fn derived_methods(
    lines: &[String],
) -> Result<std::collections::BTreeMap<String, String>, Declined> {
    let mut chosen = std::collections::BTreeMap::new();
    for line in lines {
        let (construct, method_id) =
            plateforce_batch::derive::choice("--derive", line).map_err(declined_binding)?;
        if let Some(first) = chosen.insert(construct.clone(), method_id.clone()) {
            return Err(crate::analyse::stated_twice(
                "--derive", &construct, &first, &method_id,
            ));
        }
    }
    Ok(chosen)
}

/// A comparison that cannot be set up, in the shape the caller's other refusals arrive in.
///
/// A name no rule answers to carries the published code, because it is the same fault as
/// naming an unknown rule anywhere else. The other two are faults in the line: they describe a
/// combination of flags rather than a rule that failed.
fn declined_axis(refusal: plateforce_batch::SweepRefusal) -> Declined {
    match refusal {
        plateforce_batch::SweepRefusal::UnknownMethod(recorded) => Declined::recorded(*recorded),
        other => Declined::line(Fault::Request, other.to_string()),
    }
}

/// A rule the run cannot bind, in the shape the caller's other refusals arrive in.
///
/// The two halves keep the split the record makes: a line the reader will rewrite from the
/// grammar carries no published code, and a name they will rewrite from a list carries one.
fn declined_binding(refusal: plateforce_batch::DeriveRefusal) -> Declined {
    match refusal {
        plateforce_batch::DeriveRefusal::Malformed { .. } => {
            Declined::line(Fault::Request, refusal.to_string())
        }
        plateforce_batch::DeriveRefusal::Recorded(recorded) => Declined::recorded(*recorded),
    }
}

fn run_analyse(
    out_dir: &std::path::Path,
    args: &Args,
    set: &TrialSet,
    request: &BatchRequest,
    registry: &Registry,
    format: Format,
) -> Outcome {
    let result = match analyse(set, request, registry) {
        Ok(result) => result,
        Err(refusal) => return declined_run(refusal),
    };

    if let Err(error) = result.write_csv(out_dir) {
        return Outcome::declined_line(Fault::Request, error.to_string());
    }

    let rendering = if args.without_provenance {
        Rendering::WithoutProvenance
    } else {
        Rendering::WithProvenance
    };
    let document = match format {
        Format::Json => result.to_json(),
        _ => render_table(&result.render(rendering)),
    };

    // A trial that declined one landmark computed the rest, so its refusals travel beside the
    // numbers: in the table, in the run's own refusals file, and in the JSON document, each
    // carrying the code and the rule that produced it.
    let mut outcome = Outcome::complete(document);
    outcome.every_requested_quantity_has_a_value = result.refusals.is_empty();
    outcome
}

fn run_compare(
    out_dir: &std::path::Path,
    args: &Args,
    set: &TrialSet,
    request: BatchRequest,
    registry: &Registry,
    format: Format,
) -> Outcome {
    if args.against.is_empty() {
        return Outcome::declined_line(
            Fault::Request,
            "a comparison runs two or more rules over one recording, and this one named one, so --against takes the rule to compare the bound one against".to_string(),
        );
    }

    // The step being compared is read off the rules named to compare, because every id in this
    // build is filed under exactly one construct.
    let axis = match plateforce_batch::axis_over(&request.analysis, &args.against) {
        Ok(axis) => axis,
        Err(refusal) => return Outcome::declined(declined_axis(refusal)),
    };
    let compare_request = BatchCompareRequest {
        analysis: request,
        slot: axis.slot,
        method_ids: axis.method_ids,
        quantity: args.quantity.clone(),
    };

    let result = compare(set, &compare_request);
    // The request that ran, pin included, rather than one rebuilt from the arguments: a digest
    // over a second construction identifies that construction rather than the run.
    let request_digest = plateforce_batch::fingerprint::request_digest(
        &compare_request.analysis.analysis,
        compare_request.analysis.registry_version.as_deref(),
    );
    if let Err(error) = result.write_csv(out_dir, &registry.content_digest, &request_digest) {
        return Outcome::declined_line(Fault::Request, error.to_string());
    }

    let document = match format {
        Format::Json => result.to_json(&registry.content_digest, &request_digest),
        _ => result.coverage(),
    };
    let mut outcome = Outcome::complete(document);
    outcome.every_requested_quantity_has_a_value = result.complete_pairs == result.trial_count;
    outcome
}

/// A run that read no trial, because a choice on its path is still open.
///
/// The constructs and their published alternatives travel out so the caller renders them
/// through whatever it already uses for a forced decision, rather than a second layout here.
fn declined_run(refusal: plateforce_batch::RunRefusal) -> Outcome {
    let outstanding: Vec<String> = refusal
        .unresolved
        .iter()
        .map(|open| open.construct.clone())
        .collect();
    let recorded = match refusal.code {
        plateforce_core::RefusalCode::DecisionNotMade => {
            Refusal::decision_not_made("this run", outstanding)
        }
        other => return Outcome::declined_line(fault_for(other), refusal.message.clone()),
    };
    Outcome::declined(Declined::shown_as(recorded, refusal.message.clone()))
}

/// The table as a terminal reads it, in the shape the renderer already decided.
fn render_table(rendered: &plateforce_batch::Rendered) -> String {
    use std::fmt::Write as _;
    let mut widths: Vec<usize> = rendered.header.iter().map(|name| name.len()).collect();
    for row in &rendered.rows {
        for (index, cell) in row.iter().enumerate() {
            if index < widths.len() {
                widths[index] = widths[index].max(cell.len());
            }
        }
    }

    let mut text = String::new();
    let line = |cells: &[String], widths: &[usize]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(index, cell)| format!("{cell:<width$}", width = widths[index]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let _ = writeln!(text, "{}", line(&rendered.header, &widths));
    for row in &rendered.rows {
        let _ = writeln!(text, "{}", line(row, &widths));
    }
    // Directly under the rows they qualify, and above any reduction, because a mean taken
    // over a column carries whatever the column carries.
    if !rendered.signals.is_empty() {
        let _ = writeln!(text);
        for signal in &rendered.signals {
            let _ = writeln!(text, "{signal}");
        }
    }
    if !rendered.summary.is_empty() {
        let _ = writeln!(text);
        for summary in &rendered.summary {
            let _ = writeln!(text, "{summary}");
        }
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "{}", rendered.coverage);
    text
}
