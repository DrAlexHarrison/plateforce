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

mod common;

/// One surface that hands a caller a document, what it names to reach the one generator, and
/// what a second generator would have to define.
///
/// A list of names rather than one, because the generator has two doors into it and both lead
/// to the same loop: `accounts_of` derives the chains itself, and `descriptions_of` takes chains
/// a caller already holds. A publisher that needs the chains for something else reaches the
/// second and is not writing an account of its own, which is what the guard below this one is
/// for.
struct Publisher {
    path: &'static str,
    reads: &'static [&'static str],
}

/// The generator, and the file that holds it. A publisher carrying this line has written a
/// second one, whatever it goes on to do with it.
const A_GENERATOR_OF_ITS_OWN: (&str, &str) = (
    "fn descriptions_of",
    "crates/plateforce-analysis/src/chain.rs",
);

/// Where the document is assembled, and the two facts that make an empty block unwritable by
/// a caller: the constructor fills the field, and it takes no such argument.
///
/// Anchored on the impl block rather than on the first `pub fn of(` in the file, because
/// `SpreadDocument` declares one above `ResultDocument` and a read that took the first
/// reported the sweep's three arguments. That list holds no `descriptions` either, so the
/// assertion passed on the wrong constructor, which is what the control below now catches.
const THE_DOCUMENT: (&str, &str, &str) = (
    "crates/plateforce-analysis/src/document.rs",
    "descriptions: crate::accounts_of(",
    "impl ResultDocument {",
);

/// An argument this constructor takes and the one above it in the same file does not.
///
/// The control on reading the list, and it is the discriminating one: a parse that found the
/// wrong function, or stopped at the first line, reports a list without this name in it, and
/// a list nothing is in holds no `descriptions` either. `spread` stood here first and both
/// constructors take one, so the control agreed with the wrong answer.
const AN_ARGUMENT_ONLY_THIS_CONSTRUCTOR_TAKES: &str = "capture";

/// The document built by hand, which is the way past a constructor that fills a field. Proven
/// alive against a file that really does build one, so a spelling that drifted out of the
/// language cannot read as no publisher building one.
const THE_DOCUMENT_BUILT_BY_HAND: (&str, &str) = (
    "ResultDocument {",
    "crates/plateforce-analysis/tests/a_spread_that_leaves_alone_says_which_build_produced_it.rs",
);

/// The two doors into the one generator. Naming either is reaching it.
const THE_GENERATOR: &[&str] = &["accounts_of(", "descriptions_of("];

const PUBLISHERS: &[Publisher] = &[
    // The terminal and the browser tab assemble `ResultDocument`, which fills the block from
    // the response, so what they name is the document.
    Publisher {
        path: "crates/plateforce-cli/src/analyse.rs",
        reads: &["ResultDocument::of("],
    },
    Publisher {
        path: "crates/plateforce-wasm/src/lib.rs",
        reads: &["ResultDocument::of("],
    },
    // A notebook and an R session assemble documents of their own, because neither is handed
    // a path and neither can carry the trial block, so each names the generator directly.
    Publisher {
        path: "crates/plateforce-python/src/analysis.rs",
        reads: THE_GENERATOR,
    },
    Publisher {
        path: "bindings/r/src/rust/src/lib.rs",
        reads: THE_GENERATOR,
    },
    // A folder run publishes one account per number as a relation of its own, and holds the
    // chains anyway to read the rule at the root of each.
    Publisher {
        path: "crates/plateforce-batch/src/engine.rs",
        reads: THE_GENERATOR,
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
            publisher.reads.iter().any(|name| text.contains(name)),
            "{} hands out a result and names none of {:?}",
            publisher.path,
            publisher.reads
        );
    }
    println!("{} publishers checked", PUBLISHERS.len());

    // Every name a publisher is allowed to reach the generator by is proven alive in the file
    // that declares the generator. A spelling that drifted out of the language matches nothing,
    // and a publisher matching nothing is what the assertion above reports as reaching it.
    let (_, home) = A_GENERATOR_OF_ITS_OWN;
    let declaring = source(home);
    for name in THE_GENERATOR {
        assert!(
            declaring.contains(&format!("fn {}", name.trim_end_matches('('))),
            "{name} is offered as a way to reach the generator and {home} declares no such \
             function, so a publisher naming nothing would read as reaching it"
        );
    }
}

/// And none of them writes one, or builds the document that fills it by hand.
///
/// Two ways past the constructor, so two patterns. A publisher defining a generator of its own
/// is a second home for the sentence; a publisher assembling `ResultDocument` field by field
/// reaches the field itself, which `ResultDocument::of` not taking it does not stop.
#[test]
fn no_publisher_writes_a_generator_of_its_own() {
    let (construction, _) = A_GENERATOR_OF_ITS_OWN;
    let (by_hand, _) = THE_DOCUMENT_BUILT_BY_HAND;
    let mut offences: Vec<String> = Vec::new();
    for publisher in PUBLISHERS {
        let text = source(publisher.path);
        if text.contains(construction) {
            offences.push(format!("{} defines {construction}", publisher.path));
        }
        if text.contains(by_hand) {
            offences.push(format!("{} assembles {by_hand}", publisher.path));
        }
    }
    assert!(
        offences.is_empty(),
        "an account is written outside the one home: {offences:?}"
    );
}

/// The control on the guard above, which is the one that can pass by looking at nothing.
///
/// Both patterns are shown alive in a file that really carries them: a spelling that drifted
/// out of the language matches nothing, and matching nothing in a publisher is what that
/// guard reports as clean. Neither is proven against this file, because a pattern that only
/// matches its own declaration is a set compared with itself.
#[test]
fn the_spellings_a_second_generator_would_match_still_match_a_first_one() {
    let (construction, proven_in) = A_GENERATOR_OF_ITS_OWN;
    assert!(
        source(proven_in).contains(construction),
        "{construction} matches nothing in {proven_in}, so it would match nothing in a \
         publisher either and that guard would read as clean"
    );
    let (by_hand, built_in) = THE_DOCUMENT_BUILT_BY_HAND;
    assert!(
        source(built_in).contains(by_hand),
        "{by_hand} matches nothing in {built_in}, which does build one, so it would match \
         nothing in a publisher either"
    );
}

/// The block is filled where the document is assembled, and no caller states one.
///
/// This is what the two surfaces passing an empty map ran into, and it is a signature rather
/// than a convention: an argument re-added here is an argument a publisher has to pass, and
/// an empty map is what both of them passed. The compiler says nothing about the value.
#[test]
fn the_document_fills_the_block_and_no_caller_states_one() {
    let (path, fills_it, impl_block) = THE_DOCUMENT;
    let text = source(path);
    assert!(
        text.contains(fills_it),
        "{path} assembles the document and never writes {fills_it}, so the field is coming \
         from somewhere else"
    );

    let at = text
        .find(impl_block)
        .unwrap_or_else(|| panic!("{path} declares no {impl_block}"));
    let body = &text[at + impl_block.len()..];
    let opens = body
        .find("pub fn of(")
        .unwrap_or_else(|| panic!("{impl_block} in {path} declares no constructor"));
    let rest = &body[opens + "pub fn of(".len()..];
    let end = rest
        .find(") -> Self {")
        .unwrap_or_else(|| panic!("the constructor under {impl_block} never closes its arguments"));
    // The last word before the colon, rather than everything before it: an argument carrying
    // an attribute or a `mut` on its own line reads as one name with the whole prefix on it,
    // which is a name no comparison here matches and a way past this guard.
    let arguments: Vec<&str> = rest[..end]
        .lines()
        .map(|line| {
            line.split(':')
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .next_back()
                .unwrap_or_default()
        })
        .filter(|name| !name.is_empty())
        .collect();

    // The control, and the one that has to discriminate: a parse that found the other
    // constructor in this file, or stopped early, reports a list without this name in it, and
    // no `descriptions` is in that list either, so the assertion below would pass on it.
    assert!(
        arguments.contains(&AN_ARGUMENT_ONLY_THIS_CONSTRUCTOR_TAKES),
        "the arguments read under {impl_block} are {arguments:?}, which does not include \
         {AN_ARGUMENT_ONLY_THIS_CONSTRUCTOR_TAKES}, so this read is some other constructor's"
    );
    assert!(
        !arguments.contains(&"descriptions"),
        "the constructor under {impl_block} takes the block from its caller again, and both \
         callers passed an empty one: {arguments:?}"
    );
    println!(
        "{} arguments stated by the caller: {arguments:?}",
        arguments.len()
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
    common::prepared(AnalysisRequest {
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
    })
}

fn document_for(trial: &Trial) -> ResultDocument {
    let response = run(trial, &a_request()).expect("the request is well formed");
    ResultDocument::of(
        "0.1.0",
        TrialSource {
            name: "trial".into(),
            rows_read: trial.len(),
            samples_matching_the_convention: 0,
            sample_rate_hz: 1200.0,
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

/// A quantity no rule computed gives its producer's account and never an invented one.
///
/// The control on the case above, and it has to come from a trial where the state is real: a
/// block that simply held every key would pass that one, and a sentence about a number nobody
/// computed is the shape this whole field exists against.
///
/// The property is the one this guard has always held. What moved is where the invention would
/// have to show: the block used to answer by holding no key at all, and a reader filtering it
/// for a quantity met the same silence whether the rule declined or was never asked. It now
/// holds a key for every quantity, so the assertion is on the sentence rather than on the key,
/// and the sentence has to be one an existing producer wrote. `describe`'s output opens with the
/// value and its unit, which is exactly what a number nobody computed must never carry, so that
/// is what is checked and the trial where every quantity answers is what proves the check can
/// see it.
#[test]
fn a_quantity_with_no_value_gives_its_producers_account_and_never_an_invented_one() {
    let document = document_for(&a_trial_that_never_leaves_the_plate());
    let absent: Vec<&plateforce_analysis::Metric> = document
        .metrics
        .iter()
        .filter(|metric| metric.value.is_none())
        .collect();

    assert!(
        !absent.is_empty(),
        "every quantity carried a value on a trial written to leave some without one"
    );

    // The refusals this response carries, reached through the record every other surface is
    // handed rather than through the call the block itself makes.
    let refusals: Vec<String> = document
        .refusals
        .iter()
        .map(|refusal| refusal.message().to_string())
        .collect();
    let remedies: Vec<&str> = document
        .signals
        .iter()
        .map(|signal| signal.remedy.as_str())
        .collect();

    let mut invented: Vec<String> = Vec::new();
    for metric in &absent {
        let account = document.descriptions.get(&metric.key).unwrap_or_else(|| {
            panic!(
                "the block holds no entry for {}, so a reader filtering for it meets the same \
                 silence whether a rule declined or nobody asked",
                metric.key
            )
        });
        let written_by_a_producer = account.is_empty()
            || refusals.iter().any(|sentence| sentence == account)
            || remedies.iter().any(|remedy| *remedy == account);
        if !written_by_a_producer {
            invented.push(format!("{}: {account}", metric.key));
        }
    }
    assert!(
        invented.is_empty(),
        "{} of {} quantities with no value carry a sentence no producer wrote: {invented:?}",
        invented.len(),
        absent.len()
    );

    // And none of them asserts a measurement. `describe` opens with the value and its unit, so
    // an account beginning with a number is a claim about a number nobody computed.
    let asserting: Vec<&str> = absent
        .iter()
        .filter(|metric| {
            document.descriptions[&metric.key]
                .split_whitespace()
                .next()
                .is_some_and(|token| token.parse::<f64>().is_ok())
        })
        .map(|metric| metric.key.as_str())
        .collect();
    assert!(
        asserting.is_empty(),
        "an account of a number nobody computed opens with a value: {asserting:?}"
    );
    println!(
        "{} of {} quantities carried no value and none of them asserts a measurement",
        absent.len(),
        document.metrics.len()
    );
}

/// The control on the predicate the guard above rests on.
///
/// A test for accounts that open with a value proves nothing until the predicate is shown
/// finding one. Every quantity answers on the trial that lands, and every one of their accounts
/// opens with its own value, so a predicate that had stopped recognising `describe`'s output
/// reddens here rather than reading as a clean block over there.
#[test]
fn the_check_for_an_account_that_opens_with_a_value_finds_one_where_every_quantity_answers() {
    let document = document_for(&a_jump_that_lands());
    let opening: Vec<&str> = document
        .metrics
        .iter()
        .filter(|metric| metric.value.is_some())
        .filter(|metric| {
            document
                .descriptions
                .get(&metric.key)
                .and_then(|account| account.split_whitespace().next())
                .is_some_and(|token| token.parse::<f64>().is_ok())
        })
        .map(|metric| metric.key.as_str())
        .collect();
    println!(
        "{} of {} accounts open with the value they are about",
        opening.len(),
        document.metrics.len()
    );
    assert_eq!(
        opening.len(),
        document.metrics.len(),
        "an account of a number does not open with that number, so the guard beside this one is \
         looking for a shape this build no longer writes"
    );
}
