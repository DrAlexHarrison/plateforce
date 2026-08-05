//! A folder of trials in, one table out, and the record of what produced every number.
//!
//! Two entry points over the same set rather than one with a flag. `analyse` runs one rule
//! per construct and returns a row per trial; `compare` sweeps several rules for one quantity
//! and returns a row per trial per rule. They answer different questions and a caller asking
//! for one never silently receives the other.

use std::path::PathBuf;

use plateforce_batch::agreement::BatchCompareRequest;
use plateforce_batch::{
    analyse, compare, with_aggregates, AggregationRequest, BatchRequest, GroupKind, Rendering,
    SourceFormat, TrialIdentity, TrialSet,
};
use plateforce_core::{DispersionEstimator, Refusal};
use plateforce_registry::Registry;

use crate::exit::{fault_for, Declined, Fault, Outcome};
use crate::out::Format;

/// The athlete's mass, which is not the weighed system mass: system weight includes any bar
/// and bodyweight does not.
///
/// One spelling covers one athlete and a squad, because a reader stating either is doing one
/// thing. `{subject}` is the field `--pattern` pulls out of each file name.
const MASS_HELP: &str = "The athlete's mass, which is not the weighed system mass: system weight includes any bar and bodyweight does not. Written <KG> for a folder of one athlete, or <SUBJECT>=<KG> per athlete, repeatable";

/// What a mass keyed by subject looks like, used by the help and by the refusals, so a flag
/// cannot describe a shape its parser does not take.
const MASS_SHAPE: &str = "<SUBJECT>=<KG>";

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
    #[arg(long = "condition", value_name = "ASSIGNMENT", help = crate::analyse::CONDITION_HELP)]
    pub condition: Vec<String>,
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
    /// The published rule that reduces an athlete's trials to one number. There is no such
    /// thing as the mean here: `trial.aggregation` publishes three incompatible rules and none
    /// of them is the arithmetic mean of a subject's trials
    #[arg(long = "aggregate", value_name = "RULE")]
    pub aggregate: Option<String>,
    /// How many trials the rule was asked for, which travels with the value everywhere. Best
    /// of five and best of three are two requests of one rule and not one request
    #[arg(long = "aggregate-n", value_name = "N")]
    pub aggregate_n: Option<usize>,
    /// Which trials one reduction is taken over. A session is one athlete on one occasion, so
    /// grouping by subject pools that athlete's occasions and grouping by session keeps them
    /// apart
    #[arg(long = "aggregate-by", value_name = "GROUP", default_value = "subject")]
    pub aggregate_by: String,
    /// A quantity to reduce, repeatable. Unstated, every quantity the run computed is reduced
    /// and each row names its own
    #[arg(long = "aggregate-quantity", value_name = "KEY")]
    pub aggregate_quantity: Vec<String>,
    /// Which standard deviation sits beside each reduced value. Recorded as assumed where
    /// nobody states it, because no published rule for this reduction names one
    #[arg(
        long = "aggregate-dispersion",
        value_name = "ESTIMATOR",
        default_value = "sample"
    )]
    pub aggregate_dispersion: String,
    /// Gravity where the plate stands, which applies to every trial in the folder. Unstated,
    /// standard gravity runs and the record says nobody was asked
    #[arg(long, value_name = "M/S2")]
    pub gravity: Option<f64>,
    #[arg(
        long = "body-mass-kg",
        value_name = "KG",
        allow_negative_numbers = true,
        help = MASS_HELP
    )]
    pub body_mass_kg: Vec<String>,
    /// Cite this registry revision in the record. Unstated, the record names no pinned
    /// revision and reports the one the registry declares for itself
    #[arg(long, value_name = "REVISION")]
    pub registry_version: Option<String>,
    #[arg(long = "acquisition", value_name = "ASSIGNMENT", help = crate::acquisition_arg::ACQUISITION_HELP)]
    pub acquisition: Vec<String>,
    #[arg(long, value_name = "NAME", help = crate::plate_source::PLATE_HELP)]
    pub plate: Option<String>,
    /// Hide the fingerprint column in the printed table. The record is written either way
    #[arg(long)]
    pub without_provenance: bool,
}

impl Args {
    /// Whether this run asked for its trials to be reduced to one number per group.
    ///
    /// One home, read by the run that performs the reduction and by the run that cannot, so the
    /// two cannot come to disagree about what counts as asking. They did: `--mode compare`
    /// beside `--aggregate` returned a comparison, wrote no reduction, exited 0 and said
    /// nothing, which is the silent-default failure this product exists against, arriving in
    /// the flag that had just been added to prevent one.
    ///
    /// `--aggregate-by` and `--aggregate-dispersion` are not read here. Both carry a default,
    /// so both are always set, and a run that never mentioned a reduction would read as having
    /// asked for one.
    fn asked_for_a_reduction(&self) -> bool {
        self.aggregate.is_some()
            || self.aggregate_n.is_some()
            || !self.aggregate_quantity.is_empty()
    }
}

pub fn run(
    args: &Args,
    registry_directory: Option<&std::path::Path>,
    plates_directory: Option<&std::path::Path>,
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
    let derived = match crate::analyse::derived_methods(&args.derive) {
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
    let conditioning = match crate::analyse::conditioning_methods(&args.condition) {
        Ok(conditioning) => conditioning,
        Err(declined) => return Outcome::declined(declined),
    };
    let (body_mass_kilograms, body_mass_by_subject) = match stated_masses(&args.body_mass_kg) {
        Ok(masses) => masses,
        Err(declined) => return Outcome::declined(declined),
    };
    let mut built = request_for(
        args,
        &registry,
        &derived,
        &conditioning,
        &stated,
        &named,
        body_mass_kilograms,
    );
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
    let capture = match crate::plate_source::capture_for(
        args.plate.as_deref(),
        &args.acquisition,
        plates_directory,
    ) {
        Ok(capture) => capture,
        Err(declined) => return Outcome::declined(declined),
    };

    let resolved: Vec<&str> = chosen.keys().map(String::as_str).collect();
    let mut request = BatchRequest::new(built)
        .resolving(&resolved)
        .pinned_to(args.registry_version.clone())
        .describing(capture);
    // A folder holding several athletes runs each trial under its own athlete's mass. The
    // engine refuses a name the folder does not hold and a subject the masses do not cover,
    // so a typo applies to nothing loudly rather than quietly.
    if !body_mass_by_subject.is_empty() {
        request = request.massing(body_mass_by_subject);
    }

    match args.mode {
        Mode::Analyse => run_analyse(out_dir, args, &set, &request, &registry, format, renderer),
        Mode::Compare => run_compare(out_dir, args, &set, request, &registry, format),
    }
}

/// One request for every trial in the folder, carrying the values the operator stated and
/// the ids the registry backs.
fn request_for(
    args: &Args,
    registry: &plateforce_registry::Registry,
    derived: &std::collections::BTreeMap<String, String>,
    conditioning: &std::collections::BTreeMap<String, String>,
    stated: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    named: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    body_mass_kilograms: Option<f64>,
) -> plateforce_analysis::AnalysisRequest {
    // The value and the claim about where it came from are written together, by the one
    // routine every surface writes a gravity through.
    let (gravity_meters_per_second_squared, gravity_source) =
        plateforce_analysis::gravity_stated(args.gravity);
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
    let mut request = plateforce_analysis::AnalysisRequest {
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
        gravity_meters_per_second_squared,
        gravity_source,
        // Stated once for the folder, as the plate and the acquisition block are: every file
        // in one folder came off one athlete on one day.
        body_mass_kilograms,
        registry_backed_ids: crate::analyse::backed_ids(registry),
        // The phase that conditions the signal runs on every trial in the folder, so a value
        // written against it applies to every trial the same way `--set onset.k` does. Read
        // through the same routine the single trial reads it through, because a folder that
        // conditioned differently from one file would be a second answer to one question.
        conditioning: crate::analyse::conditioning_choices(conditioning, stated, named),
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
    };
    // The published defaults every rule in the folder runs on, read from the registry this
    // command was pointed at rather than held in the rules.
    request.reading(registry);
    request
}

/// What this folder was told about the athletes' masses: one mass, or one per subject.
///
/// Mixing the two is refused rather than resolved. A bare mass beside a keyed one leaves it
/// unsaid which trials the bare one covers, and every answer to that is a rule this software
/// would be inventing on the reader's behalf.
///
/// Each value goes through the check one trial's mass goes through, so a mass at or below zero
/// is refused by the name the record reports it under rather than dividing into an infinity
/// three surfaces downstream.
fn stated_masses(
    assignments: &[String],
) -> Result<(Option<f64>, std::collections::BTreeMap<String, f64>), Declined> {
    let mut one_athlete: Vec<&str> = Vec::new();
    let mut by_subject: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for written in assignments {
        let Some((subject, kilograms)) = written.split_once('=') else {
            one_athlete.push(written);
            continue;
        };
        if subject.trim().is_empty() {
            return Err(Declined::line(
                Fault::Request,
                format!("--body-mass-kg takes {MASS_SHAPE}\n  it was given '{written}'"),
            ));
        }
        let value = crate::analyse::stated_body_mass(Some(parsed(written, kilograms)?))?;
        if by_subject
            .insert(subject.trim().to_string(), value.unwrap_or_default())
            .is_some()
        {
            return Err(Declined::line(
                Fault::Request,
                format!(
                    "--body-mass-kg names {} twice, and a subject has one mass",
                    subject.trim()
                ),
            ));
        }
    }

    match (one_athlete.as_slice(), by_subject.is_empty()) {
        ([], _) => Ok((None, by_subject)),
        ([only], true) => Ok((crate::analyse::stated_body_mass(Some(parsed(only, only)?))?, by_subject)),
        ([_, ..], true) => Err(Declined::line(
            Fault::Request,
            format!(
                "--body-mass-kg was given {} masses for one folder\n  a folder of several athletes states {MASS_SHAPE} per athlete",
                one_athlete.len()
            ),
        )),
        ([_, ..], false) => Err(Declined::line(
            Fault::Request,
            "--body-mass-kg states a mass by athlete and one for the folder".to_string(),
        )),
    }
}

/// A mass as a number, refused by the line the caller wrote where it is not one.
///
/// The value goes on its own line, so a long one cannot push the sentence past the eighty
/// columns every common terminal reaches.
fn parsed(written: &str, number: &str) -> Result<f64, Declined> {
    number.trim().parse().map_err(|_| {
        Declined::line(
            Fault::Request,
            format!("--body-mass-kg takes a mass in kilograms\n  it was given '{written}'"),
        )
    })
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

fn run_analyse(
    out_dir: &std::path::Path,
    args: &Args,
    set: &TrialSet,
    request: &BatchRequest,
    registry: &Registry,
    format: Format,
    renderer: &crate::render::Renderer,
) -> Outcome {
    let result = match analyse(set, request, registry) {
        Ok(result) => result,
        Err(refusal) => return declined_run(refusal, registry, &request.analysis, renderer),
    };

    // The reduction runs after the trials and before anything is written, so a request that
    // names a rule the group cannot satisfy refuses instead of leaving a folder of tables
    // beside a reduction that never happened.
    let result = match reduced_per_group(args, set, result) {
        Ok(result) => result,
        Err(declined) => return Outcome::declined(declined),
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

/// The reduction a run asked for, joined onto its result, or the refusal that says why not.
///
/// Nothing here reduces anything. `plateforce_batch::with_aggregates` does it, bound to
/// `trial.aggregation`, and this is the door: the engine has carried the three published rules,
/// their refusals and their provenance since the batch engine landed, and no surface reached
/// them, so a construct publishing three incompatible rules could be read in the registry and
/// run by nobody.
///
/// A run naming no rule reduces nothing and says nothing, which is the correct state of a run
/// that asked no question about athletes.
fn reduced_per_group(
    args: &Args,
    set: &TrialSet,
    result: plateforce_batch::BatchResult,
) -> Result<plateforce_batch::BatchResult, Declined> {
    if !args.asked_for_a_reduction() {
        return Ok(result);
    }

    let group_kind = match args.aggregate_by.as_str() {
        "subject" => GroupKind::Subject,
        "session" => GroupKind::Session,
        "run" => GroupKind::Run,
        other => {
            return Err(Declined::line(
                Fault::Request,
                format!(
                    "a reduction is taken over subject, session or run, and this one named {other}"
                ),
            ))
        }
    };

    // Read through core's own words rather than matched here, so a third estimator arrives on
    // this flag without an edit and cannot arrive under a second spelling.
    let dispersion = match DispersionEstimator::from_published_str(&args.aggregate_dispersion) {
        Some(estimator) => estimator,
        None => {
            return Err(Declined::line(
                Fault::Request,
                format!(
                "the standard deviation beside a reduced value is one of {}, and this run named {}",
                DispersionEstimator::PUBLISHED.join(", "),
                args.aggregate_dispersion
            ),
            ))
        }
    };

    // Every quantity the run computed, where nobody named one. A scope rather than a method
    // choice, and each row names the quantity it reduced, so nothing is reduced unseen.
    let quantities = if args.aggregate_quantity.is_empty() {
        result.quantities.clone()
    } else {
        let absent: Vec<&String> = args
            .aggregate_quantity
            .iter()
            .filter(|key| !result.quantities.contains(key))
            .collect();
        if !absent.is_empty() {
            let named: Vec<&str> = absent.iter().map(|key| key.as_str()).collect();
            return Err(Declined::line(
                Fault::Request,
                format!(
                    "this run computed {}, and a reduction was asked for {}",
                    result.quantities.join(", "),
                    named.join(", ")
                ),
            ));
        }
        args.aggregate_quantity.clone()
    };

    let request = AggregationRequest::declared(
        args.aggregate.as_deref(),
        args.aggregate_n,
        group_kind,
        quantities,
        dispersion,
    )
    .map_err(|refusal| Declined::line(Fault::Request, refusal.message()))?;

    with_aggregates(result, set, &request)
        .map_err(|refusal| Declined::line(Fault::Request, refusal.message()))
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

    // A comparison returns one row per trial per rule, so an athlete's trials do not sit in one
    // column for a rule to reduce. Refused by name rather than ignored: this run silently
    // dropped the reduction and exited 0 on the day the flag was added.
    if args.asked_for_a_reduction() {
        return Outcome::declined_line(
            Fault::Request,
            "a comparison returns one row per trial per rule, so an athlete's trials are not in one column for trial.aggregation to reduce, and --mode analyse is the run that takes --aggregate".to_string(),
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
        axis,
        quantity: args.quantity.clone(),
    };

    let result = compare(set, &compare_request);
    // The request the sweep varied, pin included, taken off the request that ran rather than
    // one rebuilt from the arguments: a digest over a second construction identifies that
    // construction. It is the base and not the run, because the axis is not in it, so two
    // comparisons over one folder that swept different rules answer alike here. The axis
    // reaches the record through construct, method_ids and held_fixed beside it.
    let base_request_digest = plateforce_batch::fingerprint::request_digest(
        &compare_request.analysis.analysis,
        compare_request.analysis.registry_version.as_deref(),
        &compare_request.analysis.body_mass_kilograms_by_subject,
    );
    // Which registry produced these numbers, as the three facts a reader asks for, built where
    // `analyse` builds them. The pin is the caller's word and the declared revision is the
    // registry's, and this surface published the second under the first's name once already.
    let stamp = crate::analyse::registry_stamp(registry, args.registry_version.clone());
    if let Err(error) = result.write_csv(out_dir, &stamp, &base_request_digest) {
        return Outcome::declined_line(Fault::Request, error.to_string());
    }

    let document = match format {
        Format::Json => result.to_json(&stamp, &base_request_digest),
        _ => result.coverage(),
    };
    let mut outcome = Outcome::complete(document);
    outcome.every_requested_quantity_has_a_value = result.complete_pairs == result.trial_count;
    outcome
}

/// A run that read no trial, because a choice on its path is still open.
///
/// Two kinds of open choice arrive here. A construct nobody bound a rule to renders through the
/// run's own sentence, which already names the constructs and their published alternatives. A
/// rule whose required number the literature publishes several ways renders through the layout
/// the single trial refuses with, so one request refused on two surfaces reads one way.
fn declined_run(
    refusal: plateforce_batch::RunRefusal,
    registry: &Registry,
    request: &plateforce_analysis::AnalysisRequest,
    renderer: &crate::render::Renderer,
) -> Outcome {
    if !refusal.unresolved_values.is_empty() {
        return Outcome::declined(crate::analyse::open_values_refusal(
            &refusal.unresolved_values,
            values_forcing_a_choice(registry, request),
            "this run",
            renderer,
        ));
    }
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

/// How many values on this run's path the literature publishes more than one way, which is the
/// denominator the open count is reported against.
fn values_forcing_a_choice(
    registry: &Registry,
    request: &plateforce_analysis::AnalysisRequest,
) -> usize {
    let bound = [
        (
            plateforce_analysis::WEIGHING_CONSTRUCT,
            request.weighing.method_id.as_str(),
            &request.weighing.parameters,
        ),
        (
            plateforce_analysis::ONSET_CONSTRUCT,
            request.onset.method_id.as_str(),
            &request.onset.parameters,
        ),
        (
            plateforce_analysis::TAKEOFF_CONSTRUCT,
            request.takeoff.method_id.as_str(),
            &request.takeoff.parameters,
        ),
    ];
    plateforce_batch::values_forcing_a_choice(registry, &bound)
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
