//! Every surface that publishes the chain behind a number reads the one derivation, and
//! derives none of its own.
//!
//! Four sites built this tree for themselves and the four disagreed. What stops a fifth
//! appearing is not that the four were converted, it is that a new one cannot be written
//! quietly: the derivation is a function call, and a hand-built chain is a step constructed
//! somewhere that has no business constructing one.
//!
//! The consumers are read as sources rather than linked, because two of them cannot be linked
//! from here. The R package is R, and the R boundary crate is built against the copies
//! `bindings/r/tools/sync-engine.sh` makes rather than against this workspace, so no cargo
//! test can call either. A comparison covering only the consumers cargo can reach would leave
//! out the two that had the worst of the disagreement.

use std::path::{Path, PathBuf};

use plateforce_analysis::chains_of;
use plateforce_analysis::{run, AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::provenance::RegistryStamp;
use plateforce_core::Trial;

/// One way of assembling a chain step by hand, and a file that still spells it that way.
///
/// The second half is the control on the first. A pattern that drifted out of the language
/// matches nothing, and matching nothing in a consumer is what this guard reports as clean, so
/// each pattern has to be shown alive somewhere before its absence means anything.
type Construction = (&'static str, &'static str);

/// The Rust ways: the struct literal, the two named constructors, and the call that turns a
/// bound rule into a record.
const RUST_CHAIN_CONSTRUCTORS: &[Construction] = &[
    (
        "ProvenanceChain {",
        "crates/plateforce-analysis/src/chain.rs",
    ),
    (
        "ProvenanceChain::leaf",
        "crates/plateforce-core/src/provenance.rs",
    ),
    (
        "ProvenanceChain::with_inputs",
        "crates/plateforce-core/src/provenance.rs",
    ),
    (
        ".into_provenance(",
        "crates/plateforce-analysis/src/chain.rs",
    ),
];

/// One surface that hands a caller the chain behind a number, what it names to reach the one
/// derivation, and what a second derivation would have to construct.
///
/// Data rather than a chain of conditions, so a consumer added later is a row and a row left
/// off is visible beside the ones that are there.
struct Consumer {
    path: &'static str,
    reads: &'static str,
    builds_none_of: &'static [Construction],
}

const CONSUMERS: &[Consumer] = &[
    Consumer {
        path: "crates/plateforce-batch/src/engine.rs",
        reads: "chain_of(",
        builds_none_of: RUST_CHAIN_CONSTRUCTORS,
    },
    Consumer {
        path: "crates/plateforce-python/src/analysis.rs",
        reads: "chain_of(",
        builds_none_of: RUST_CHAIN_CONSTRUCTORS,
    },
    Consumer {
        path: "bindings/r/src/rust/src/lib.rs",
        reads: "chains_of(",
        builds_none_of: RUST_CHAIN_CONSTRUCTORS,
    },
    // R reads the record off the wire, because R links the engine and cannot call a Rust
    // function from the package. So what it names is the reader of that field, and the
    // construction it must not perform is the object itself, which `provenance_from_record`
    // builds once and nowhere else.
    Consumer {
        path: "bindings/r/R/analyse.R",
        reads: "provenance_from_record(",
        builds_none_of: &[("provenance(", "bindings/r/R/provenance.R")],
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
            "{} is named as a consumer and did not read: {error}",
            full.display()
        )
    })
}

/// Every consumer reaches the derivation by name.
#[test]
fn every_consumer_names_the_one_derivation() {
    for consumer in CONSUMERS {
        let text = source(consumer.path);
        // A path that stopped existing panics in `source`, and one that read as a stub would
        // contain nothing to find, so the size is asserted before the contents are.
        assert!(
            text.len() > 500,
            "{} read as {} bytes, which is not a consumer",
            consumer.path,
            text.len()
        );
        assert!(
            text.contains(consumer.reads),
            "{} publishes a chain and never names {}",
            consumer.path,
            consumer.reads
        );
    }
    println!("{} consumers checked", CONSUMERS.len());
}

/// And none of them assembles one.
///
/// This is the half that stops a fifth derivation. A consumer that starts building its own
/// chain reddens on the line it builds it, rather than when somebody notices the tree it
/// publishes has changed shape.
#[test]
fn no_consumer_builds_a_chain_of_its_own() {
    let mut offences: Vec<String> = Vec::new();
    for consumer in CONSUMERS {
        let text = source(consumer.path);
        for (construction, _) in consumer.builds_none_of {
            if text.contains(construction) {
                offences.push(format!("{} builds {construction}", consumer.path));
            }
        }
    }
    assert!(
        offences.is_empty(),
        "a chain is derived outside the one home: {offences:?}"
    );
}

/// The control on the guard above, which is the one that can pass by looking at nothing.
#[test]
fn the_patterns_a_second_derivation_would_match_still_match_a_first_one() {
    let mut checked = 0usize;
    for consumer in CONSUMERS {
        for (construction, proven_in) in consumer.builds_none_of {
            assert!(
                source(proven_in).contains(construction),
                "{construction} matches nothing in {proven_in}, so it would match nothing in \
                 {} either and this guard would read as clean",
                consumer.path
            );
            checked += 1;
        }
    }
    println!("{checked} constructions shown alive");
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

/// One response gives one tree, whoever asks and however often.
///
/// What the guards above assert about the call sites is worth asserting only if the call
/// itself answers the same way twice. A derivation reading anything outside its four arguments
/// would not, and four consumers calling it would still be four answers.
#[test]
fn one_response_gives_one_tree_however_many_times_it_is_asked() {
    let request = AnalysisRequest {
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
    };
    let response = run(&a_jump_that_lands(), &request).expect("the request is well formed");
    let stamp = RegistryStamp {
        version: Some("fixture-pin".to_string()),
        declared_version: Some("fixture-declares".to_string()),
        digest: Some("content-fixture".to_string()),
    };

    let once = chains_of(&response, &stamp, true);
    let again = chains_of(&response, &stamp, true);
    assert_eq!(once, again);

    let steps: usize = once.iter().map(|one| one.chain.flattened().len()).sum();
    println!(
        "{} quantities over {steps} steps, derived twice from one response",
        once.len()
    );
    assert!(
        once.len() >= 11,
        "only {} quantities were reached",
        once.len()
    );
    assert!(steps >= 50, "only {steps} steps were reached");
}
