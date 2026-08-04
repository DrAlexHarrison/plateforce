//! The account a number gives of itself is written in one place and published by every
//! surface that hands out a result.
//!
//! It was written in the R boundary and nowhere else, so an R session was the only place a
//! reader ever met one: on the committed `quiet` request, 11 of 11 quantities carried an
//! account in R and 0 of 11 in the terminal, a notebook and a browser tab, and the terminal
//! and the tab were passing an empty map into the document by hand.
//!
//! Two halves, as `every_consumer_reads_one_chain.rs` has: every publisher reaches the one
//! generator by name, and none of them writes a generator of its own. The document assembles
//! the block rather than accepting it, which is what makes the empty map unwritable, and the
//! two source guards are what stop a surface assembling a second one beside it.
//!
//! The publishers are read as sources rather than linked, because two of them cannot be
//! linked from here. The R boundary crate is built against the copies
//! `bindings/r/tools/sync-engine.sh` makes rather than against this workspace, so no cargo
//! test can call it, and a guard covering only what cargo reaches would leave out the surface
//! that had the whole of it.

use std::path::{Path, PathBuf};

use plateforce_analysis::document::{ResultDocument, TrialSource};
use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::Trial;

/// One surface that hands a caller a document, what it names to reach the one generator, and
/// what a second generator would have to define.
struct Publisher {
    path: &'static str,
    reads: &'static str,
}

/// The generator, and the file that holds it. A publisher carrying this line has written a
/// second one, whatever it goes on to do with it.
const A_GENERATOR_OF_ITS_OWN: (&str, &str) = (
    "fn descriptions_of",
    "crates/plateforce-analysis/src/chain.rs",
);

/// The empty block, as the two surfaces that published one spelled it. Both handed it to
/// `ResultDocument::of`, which no longer takes one, so this is the spelling a surface would
/// reach for if it started assembling the field itself.
const AN_EMPTY_BLOCK: &str = "descriptions: BTreeMap::new()";

const PUBLISHERS: &[Publisher] = &[
    // The terminal and the browser tab assemble `ResultDocument`, which fills the block from
    // the response, so what they name is the document.
    Publisher {
        path: "crates/plateforce-cli/src/analyse.rs",
        reads: "ResultDocument::of(",
    },
    Publisher {
        path: "crates/plateforce-wasm/src/lib.rs",
        reads: "ResultDocument::of(",
    },
    // A notebook and an R session assemble documents of their own, because neither is handed
    // a path and neither can carry the trial block, so each names the generator directly.
    Publisher {
        path: "crates/plateforce-python/src/analysis.rs",
        reads: "accounts_of(",
    },
    Publisher {
        path: "bindings/r/src/rust/src/lib.rs",
        reads: "descriptions_of(",
    },
    // A folder run publishes one account per number as a relation of its own.
    Publisher {
        path: "crates/plateforce-batch/src/engine.rs",
        reads: "accounts_of(",
    },
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate sits inside the repository")
}

fn source(path: &str) -> String {
    let full = repository().join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|error| {
        panic!(
            "{} is named as a publisher and did not read: {error}",
            full.display()
        )
    })
}

/// Every publisher reaches the generator by name.
#[test]
fn every_publisher_names_the_one_generator() {
    for publisher in PUBLISHERS {
        let text = source(publisher.path);
        // A path that stopped existing panics in `source`, and one that read as a stub would
        // hold nothing to find, so the size is asserted before the contents are.
        assert!(
            text.len() > 500,
            "{} read as {} bytes, which is not a publisher",
            publisher.path,
            text.len()
        );
        assert!(
            text.contains(publisher.reads),
            "{} hands out a result and never names {}",
            publisher.path,
            publisher.reads
        );
    }
    println!("{} publishers checked", PUBLISHERS.len());
}

/// And none of them writes one.
#[test]
fn no_publisher_writes_a_generator_of_its_own() {
    let (construction, _) = A_GENERATOR_OF_ITS_OWN;
    let mut offences: Vec<String> = Vec::new();
    for publisher in PUBLISHERS {
        let text = source(publisher.path);
        if text.contains(construction) {
            offences.push(format!("{} defines {construction}", publisher.path));
        }
        if text.contains(AN_EMPTY_BLOCK) {
            offences.push(format!("{} publishes {AN_EMPTY_BLOCK}", publisher.path));
        }
    }
    assert!(
        offences.is_empty(),
        "an account is written outside the one home: {offences:?}"
    );
}

/// The control on the guard above, which is the one that can pass by looking at nothing.
///
/// Both patterns have to be shown alive somewhere: a spelling that drifted out of the
/// language matches nothing, and matching nothing in a publisher is what that guard reports
/// as clean.
#[test]
fn the_spellings_a_second_generator_would_match_still_match_a_first_one() {
    let (construction, proven_in) = A_GENERATOR_OF_ITS_OWN;
    assert!(
        source(proven_in).contains(construction),
        "{construction} matches nothing in {proven_in}, so it would match nothing in a \
         publisher either and that guard would read as clean"
    );
    assert!(
        source(file!()).contains(AN_EMPTY_BLOCK),
        "{AN_EMPTY_BLOCK} matches nothing here, so it would match nothing in a publisher \
         either"
    );
}

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// A countermovement jump that leaves the plate and lands back on it, so every landmark is
/// placed and every quantity reports a number.
fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, 811));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

/// A trial that never leaves the plate, so the quantities past takeoff have no value to give
/// an account of.
fn a_trial_that_never_leaves_the_plate() -> Trial {
    let mut force = vec![600.0; 2400];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn a_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
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
        ..Default::default()
    }
}

fn document_for(trial: &Trial) -> ResultDocument {
    let response = run(trial, &a_request()).expect("the request is well formed");
    ResultDocument::of(
        "0.1.0",
        TrialSource {
            name: "trial".into(),
            rows_read: trial.len(),
            samples_matching_the_convention: 0,
        },
        &RegistryStamp {
            version: Some("fixture-pin".to_string()),
            declared_version: Some("fixture-declares".to_string()),
            digest: Some("content-fixture".to_string()),
        },
        &plateforce_core::Capture::default(),
        &response,
        None,
    )
}

/// Every number the document reports gives an account of itself, and the account names the
/// rule that produced it.
#[test]
fn every_number_in_the_document_gives_an_account_of_itself() {
    let document = document_for(&a_jump_that_lands());
    let valued: Vec<&plateforce_analysis::Metric> = document
        .metrics
        .iter()
        .filter(|metric| metric.value.is_some())
        .collect();

    // A document reporting almost nothing would satisfy the comparison below having looked at
    // almost nothing, and the count is the denominator the sentence below is over.
    assert!(
        valued.len() >= 8,
        "only {} of {} quantities carried a value",
        valued.len(),
        document.metrics.len()
    );

    let mut silent: Vec<&str> = Vec::new();
    for metric in &valued {
        match document.descriptions.get(&metric.key) {
            None => silent.push(&metric.key),
            Some(account) => {
                // The rule the response names is the rule the sentence names, so an account
                // written around some other chain reddens here rather than reading as prose.
                let named = metric
                    .computed_by
                    .as_deref()
                    .unwrap_or(&metric.contributing_method_ids[0]);
                assert!(
                    account.contains(named),
                    "the account of {} never names {named}: {account}",
                    metric.key
                );
            }
        }
    }
    assert!(
        silent.is_empty(),
        "{} of {} quantities carrying a value gave no account of themselves: {silent:?}",
        silent.len(),
        valued.len()
    );
    println!(
        "{} of {} quantities carried a value and every one of them gave an account",
        valued.len(),
        document.metrics.len()
    );
}

/// A quantity no rule computed gives no account rather than an invented one.
///
/// The control on the case above, and it has to come from a trial where the state is real: a
/// block that simply held every key would pass that one, and a sentence about a number nobody
/// computed is the shape this whole field exists against.
#[test]
fn a_quantity_with_no_value_gives_no_account_rather_than_an_invented_one() {
    let document = document_for(&a_trial_that_never_leaves_the_plate());
    let absent: Vec<&str> = document
        .metrics
        .iter()
        .filter(|metric| metric.value.is_none())
        .map(|metric| metric.key.as_str())
        .collect();

    assert!(
        !absent.is_empty(),
        "every quantity carried a value on a trial written to leave some without one"
    );
    let invented: Vec<&&str> = absent
        .iter()
        .filter(|key| document.descriptions.contains_key(**key))
        .collect();
    assert!(
        invented.is_empty(),
        "{} of {} quantities with no value carry an account: {invented:?}",
        invented.len(),
        absent.len()
    );
    println!(
        "{} of {} quantities carried no value and none of them was described",
        absent.len(),
        document.metrics.len()
    );
}
