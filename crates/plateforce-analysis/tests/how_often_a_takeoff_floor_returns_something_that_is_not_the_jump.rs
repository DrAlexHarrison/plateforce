//! The failure rate behind the two takeoff search-floor entries, measured rather than argued.
//!
//! `MISSION.md`'s loop is implement, expose, run the corpus, write the measured sensitivity back
//! into the registry entry, and the fourth step is the one that gets dropped. Both entries carry a
//! `[method.failure]` block, and this is the run those numbers came out of. The comparison runs the
//! other way too: what the registry publishes is read back and held against what the corpus
//! produces, so a figure written into an entry cannot drift away from the recordings it came from.
//!
//! The corpus sits behind an environment variable and on one machine, so this says what it covered
//! and reports covering nothing rather than passing quietly. Directory names in that corpus are
//! athlete names: nothing here reads one, a trial is addressed by the subject and trial numbers in
//! its file name exactly as `plateforce-conformance::corpus` addresses it, and every figure it
//! prints is an aggregate over the whole corpus.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::{read_trial_from_path, Trial};
use plateforce_registry::Registry;

const CORPUS_VARIABLE: &str = "PLATEFORCE_CONFORMANCE_CORPUS";

/// The founding corpus samples at 1200 Hz and carries vertical force in the third column. Read at
/// 1000 every index below moves by 20 percent.
const SAMPLE_RATE_HZ: f64 = 1200.0;
const FORCE_COLUMN_INDEX: usize = 2;
const FILE_PREFIX: &str = "AT";

/// The window this whole measurement runs under, which is the length the rest of the workspace
/// uses and the one a caller gets without stating anything.
const WEIGHING_SECONDS: f64 = 1.0;

const FLOOR_AT_WEIGHING_EPOCH_END: &str = "takeoff.op.search_floor_at_weighing_epoch_end";
const FLOOR_AT_TRIAL_START: &str = "takeoff.op.search_floor_at_trial_start";

/// The two rules that begin searching where the weighing window ends.
const RULES_FLOORING_AT_THE_WEIGHING_EPOCH_END: &[&str] = &[
    "takeoff.threshold.absolute_force",
    "takeoff.threshold.flight_noise_k_sd",
];

/// The three that consider every sample of the recording.
const RULES_SEARCHING_THE_WHOLE_RECORDING: &[&str] = &[
    "takeoff.threshold.longest_run",
    "takeoff.threshold.descending_crossing",
    "takeoff.threshold.landing_shape",
];

/// What each rule did on this corpus. Held per rule as well as per policy, so a rate that is a
/// property of one rule's threshold rather than of the floor cannot hide inside a blended figure.
///
/// The two rules taking the derived floor agree on the same 2 trials, which is what makes the
/// figure a property of the floor. They read 2 and 114 while the re-estimating rule was measuring
/// its flight noise over a window running to the last low sample in the recording rather than to
/// the end of the provisional flight phase.
const MEASURED_PER_RULE: &[(&str, u32, u32)] = &[
    ("takeoff.threshold.absolute_force", 2, 246),
    ("takeoff.threshold.flight_noise_k_sd", 2, 246),
    ("takeoff.threshold.longest_run", 2, 246),
    ("takeoff.threshold.descending_crossing", 0, 244),
    ("takeoff.threshold.landing_shape", 0, 134),
];

/// Subject and trial number parsed out of a file name of the form `AT<subject>_<trial>.txt`, so
/// the directory a file sat in is discarded on the way past.
fn identify(file_name: &str) -> Option<(u32, u32)> {
    let stem = file_name.strip_suffix(".txt")?.strip_prefix(FILE_PREFIX)?;
    let (subject, trial) = stem.split_once('_')?;
    Some((subject.parse().ok()?, trial.parse().ok()?))
}

fn index_corpus(root: &Path) -> BTreeMap<(u32, u32), PathBuf> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Some(key) = path.file_name().and_then(|n| n.to_str()).and_then(identify) else {
                continue;
            };
            found.insert(key, path);
        }
    }
    found
}

/// One onset rule for every run, so the takeoff figures below differ by the takeoff rule and by
/// nothing else. The trailing-window rule reads its own window rather than the weighing epoch, so
/// it cannot itself land on the floor under test.
fn request(takeoff_id: &str) -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), WEIGHING_SECONDS)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.adaptive_trailing_window".into(),
            parameters: BTreeMap::from([
                ("k".to_string(), 5.0),
                ("window_seconds".to_string(), 1.0),
            ]),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: takeoff_id.to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// What one rule did on one trial: the instant it published, the floor it was given, and whether
/// anything in the result announced the outcome to a reader.
struct Outcome {
    takeoff_index: usize,
    floor_index: usize,
    announced: bool,
}

fn outcome(trial: &Trial, takeoff_id: &str) -> Option<Outcome> {
    let response = run(trial, &request(takeoff_id)).ok()?;
    Some(Outcome {
        takeoff_index: response.takeoff_index?,
        floor_index: response.weighing_end_index,
        announced: !response.signals.is_empty(),
    })
}

/// A rule flooring at the weighing window can return that floor and cannot place takeoff before
/// it. A rule searching the whole recording cannot return a floor that forbids nothing, and can
/// place takeoff inside the window the athlete was weighed in. Neither family can commit the
/// other's failure, which is why the two are counted apart.
fn failed(policy: &str, outcome: &Outcome) -> bool {
    match policy {
        FLOOR_AT_WEIGHING_EPOCH_END => {
            outcome.floor_index > 0 && outcome.takeoff_index == outcome.floor_index
        }
        _ => outcome.takeoff_index <= outcome.floor_index,
    }
}

#[derive(Default)]
struct Tally {
    /// Trials on which the rule published an instant, which is the denominator of the rate.
    answered: u32,
    /// Trials on which the rule declined. A rule that refuses has published no wrong instant.
    declined: u32,
    failed: u32,
    /// Of the failures, how many carried a signal a reader would see.
    announced: u32,
}

/// The figures one registry entry publishes about itself.
fn published(registry: &Registry, id: &str) -> (u32, u32, f64) {
    let entry = registry
        .methods
        .get(id)
        .unwrap_or_else(|| panic!("{id} is not an entry this registry carries"));
    let failure = entry
        .failure
        .as_ref()
        .unwrap_or_else(|| panic!("{id} publishes no failure block, so nothing here can check it"));
    (failure.numerator, failure.denominator, failure.rate)
}

fn registry() -> Registry {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the shipped registry loads")
}

/// The published rate is the published numerator over the published denominator, to the four
/// decimal places these entries are written at.
///
/// Cheap, runs without the corpus, and it catches the edit that moves one of the three figures and
/// not the others, which is how a hand-written failure block goes wrong.
#[test]
fn each_floor_entry_publishes_a_rate_that_is_its_own_numerator_over_its_own_denominator() {
    let registry = registry();
    for id in [FLOOR_AT_WEIGHING_EPOCH_END, FLOOR_AT_TRIAL_START] {
        let (numerator, denominator, rate) = published(&registry, id);
        assert!(denominator > 0, "{id} publishes a rate over no trials");
        let computed = f64::from(numerator) / f64::from(denominator);
        assert!(
            (computed - rate).abs() < 5e-5,
            "{id} publishes {numerator} of {denominator}, which is {computed:.6}, beside a rate of {rate}"
        );
    }
}

/// Both failure definitions on one pass over the corpus, held against what the entries publish.
#[test]
fn each_takeoff_floor_policy_fails_at_the_rate_its_registry_entry_publishes() {
    let Some(root) = std::env::var(CORPUS_VARIABLE).ok().map(PathBuf::from) else {
        println!(
            "{CORPUS_VARIABLE} is unset, so 0 trials were measured and nothing here was checked. \
             The figures the two takeoff search-floor entries publish came from a run with it set."
        );
        return;
    };

    let paths = index_corpus(&root);
    assert!(
        !paths.is_empty(),
        "{CORPUS_VARIABLE} names a directory holding no {FILE_PREFIX}<subject>_<trial>.txt file"
    );

    let mut unreadable = 0u32;
    let mut per_rule: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut trials_failing_the_derived_floor = 0u32;

    for path in paths.values() {
        let Ok((trial, _)) = read_trial_from_path(path, '\t', FORCE_COLUMN_INDEX, SAMPLE_RATE_HZ)
        else {
            unreadable += 1;
            continue;
        };
        let mut any_derived_floor_failed = false;

        for (policy, rules) in [
            (
                FLOOR_AT_WEIGHING_EPOCH_END,
                RULES_FLOORING_AT_THE_WEIGHING_EPOCH_END,
            ),
            (FLOOR_AT_TRIAL_START, RULES_SEARCHING_THE_WHOLE_RECORDING),
        ] {
            for id in rules {
                let tally = per_rule.entry(id).or_default();
                let Some(found) = outcome(&trial, id) else {
                    tally.declined += 1;
                    continue;
                };
                tally.answered += 1;
                if failed(policy, &found) {
                    tally.failed += 1;
                    tally.announced += u32::from(found.announced);
                    any_derived_floor_failed |= policy == FLOOR_AT_WEIGHING_EPOCH_END;
                }
            }
        }
        trials_failing_the_derived_floor += u32::from(any_derived_floor_failed);
    }

    println!(
        "{} trials indexed, {unreadable} unreadable, weighing window {WEIGHING_SECONDS} s anchored \
         at the start of the recording",
        paths.len()
    );
    for (id, tally) in &per_rule {
        println!(
            "  {id:44} failed {} of {} answered, {} declined, {} of the failures carried a signal",
            tally.failed, tally.answered, tally.declined, tally.announced
        );
    }

    // Per rule, because the two taking the derived floor differ by a factor of 57 and a blended
    // figure would hide it. A rule dropping to zero answers turns this red rather than reading as
    // a rule that never fails.
    for (id, failures, answered) in MEASURED_PER_RULE {
        let tally = per_rule.get(id).unwrap_or_else(|| {
            panic!("{id} was not run, so this comparison covers less than it says")
        });
        assert_eq!(
            (tally.failed, tally.answered),
            (*failures, *answered),
            "{id} on this corpus"
        );
    }

    let registry = registry();
    let (numerator, denominator, _) = published(&registry, FLOOR_AT_WEIGHING_EPOCH_END);
    assert_eq!(
        (trials_failing_the_derived_floor, paths.len() as u32),
        (numerator, denominator),
        "{FLOOR_AT_WEIGHING_EPOCH_END} publishes a figure this corpus no longer produces"
    );

    let trial_start = per_rule
        .get("takeoff.threshold.longest_run")
        .expect("the rule that commits this failure ran");
    let (numerator, denominator, _) = published(&registry, FLOOR_AT_TRIAL_START);
    assert_eq!(
        (trial_start.failed, paths.len() as u32),
        (numerator, denominator),
        "{FLOOR_AT_TRIAL_START} publishes a figure this corpus no longer produces"
    );

    // The detectability each entry claims, measured rather than asserted. One policy's failures
    // all reach a reader and the other's reach none, and an entry claiming otherwise would be
    // wrong about what a user of this build sees.
    let derived = RULES_FLOORING_AT_THE_WEIGHING_EPOCH_END
        .iter()
        .filter_map(|id| per_rule.get(id))
        .fold((0u32, 0u32), |(f, a), t| (f + t.failed, a + t.announced));
    assert_eq!(
        derived.0, derived.1,
        "a floor landing reached no reader, so this entry is no longer guarded"
    );
    assert_eq!(
        trial_start.announced, 0,
        "a takeoff placed inside the weighing window now carries a signal, so this entry's \
         detectability is no longer silent"
    );
}
