//! Reducing an athlete's trials to one number, from the terminal, bound to the published rule.
//!
//! `trial.aggregation` publishes three incompatible rules and none of them is the arithmetic
//! mean of a subject's trials. The engine has carried all three, their refusals and their
//! provenance since the batch engine landed, and until 2026-08-05 no surface reached them: a
//! construct that forces a decision could be read in the registry and run by nobody. This file
//! holds the door open.
//!
//! The cohort is generated arithmetic from `plateforce_batch::synthetic`, because reducing an
//! athlete's trials needs several trials per athlete and several athletes, and only subject 01
//! is ever public.

use std::path::PathBuf;
use std::process::Output;

fn plateforce(args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(args)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn cohort(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("plateforce-reduce-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&directory).ok();
    plateforce_batch::synthetic::write_corpus(&directory, 4, 3, 20_260_805)
        .expect("a generated cohort");
    directory
}

fn out_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "plateforce-reduce-out-{name}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("a directory to write into");
    directory
}

fn folder_run<'a>(trials: &'a str, out: &'a str) -> Vec<&'a str> {
    vec![
        "--registry",
        "../../registry",
        "batch",
        trials,
        "--out-dir",
        out,
        "--trial-suffix",
        ".txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--pattern",
        "AT{subject}_{trial}",
        "--weighing",
        "bwepoch.fixed_window",
        "--set",
        "weighing.duration=1.0",
        "--onset",
        "onset.threshold.noise_relative",
        "--set",
        "onset.k=5.0",
        "--takeoff",
        "takeoff.threshold.absolute_force",
    ]
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// One number per athlete, and the row names the rule that produced it.
#[test]
fn a_reduction_carries_the_registry_id_it_was_bound_to() {
    let trials = cohort("bound");
    let written = out_dir("bound");
    let named = trials.display().to_string();
    let target = written.display().to_string();
    let mut line = folder_run(&named, &target);
    line.extend([
        "--aggregate",
        "mean_of_best_two",
        "--aggregate-n",
        "2",
        "--aggregate-ranked-by",
        "reactive_strength_index",
        "--aggregate-quantity",
        "jump_height_from_takeoff_meters",
    ]);
    let output = plateforce(&line);
    assert!(output.status.success(), "{}", said(&output));

    let table = std::fs::read_to_string(written.join("aggregates.csv")).expect("a reduction");
    println!("{table}");
    let rows: Vec<&str> = table.lines().skip(1).filter(|l| !l.is_empty()).collect();

    // Four athletes in, four reduced values out. The control on the denominator: a run that
    // reduced one group and reported it would look identical to a working reduction in a test
    // that only asserted the id appears somewhere.
    assert_eq!(
        rows.len(),
        4,
        "four athletes were generated and {} rows came back: {table}",
        rows.len()
    );
    for row in &rows {
        assert!(
            row.contains("trial.aggregation"),
            "a reduced value carries the rule that produced it: {row}"
        );
        assert!(
            row.contains(",subject,"),
            "a reduced value names which trials it was taken over: {row}"
        );
    }

    // The count the rule was asked for travels with the value, because best of five and best of
    // three are two requests of one rule.
    assert!(
        rows.iter().all(|row| row.split(',').nth(5) == Some("2")),
        "every row records the two trials it reduced: {table}"
    );

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&written).ok();
}

/// A mean of the best trials has no best trial until the request names the construct that
/// orders them. No files containing reduced numbers are written under an unstated choice.
#[test]
fn a_mean_of_the_best_trials_without_a_ranking_criterion_is_refused() {
    let trials = cohort("ranking-unstated");
    let written = out_dir("ranking-unstated");
    let named = trials.display().to_string();
    let target = written.display().to_string();
    let mut line = folder_run(&named, &target);
    line.extend([
        "--aggregate",
        "mean_of_best_two",
        "--aggregate-n",
        "2",
        "--aggregate-quantity",
        "jump_height_from_takeoff_meters",
    ]);
    let output = plateforce(&line);
    let told = said(&output);
    println!("{told}");

    assert!(
        !output.status.success(),
        "a run chose best trials without a criterion: {told}"
    );
    assert!(
        told.contains("ranked_by"),
        "the refusal does not name the choice that is still open: {told}"
    );
    assert!(
        !written.join("aggregates.csv").exists(),
        "reduced numbers were written after the reduction was refused"
    );

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&written).ok();
}

/// The ranking criterion stated on the command line reaches the method record under the
/// registry parameter's own name.
#[test]
fn a_stated_ranking_criterion_is_recorded_with_the_reduction() {
    let trials = cohort("ranking-recorded");
    let written = out_dir("ranking-recorded");
    let named = trials.display().to_string();
    let target = written.display().to_string();
    let mut line = folder_run(&named, &target);
    line.extend([
        "--aggregate",
        "mean_of_best_two",
        "--aggregate-n",
        "2",
        "--aggregate-ranked-by",
        "reactive_strength_index",
        "--aggregate-quantity",
        "jump_height_from_takeoff_meters",
    ]);
    let output = plateforce(&line);
    assert!(output.status.success(), "{}", said(&output));

    let provenance =
        std::fs::read_to_string(written.join("provenance.csv")).expect("a method record");
    let ranking: Vec<&str> = provenance
        .lines()
        .skip(1)
        .filter(|row| row.split(',').nth(4) == Some("ranked_by"))
        .collect();
    assert_eq!(
        ranking.len(),
        1,
        "one reduction chain records one ranking criterion:\n{provenance}"
    );
    assert_eq!(
        ranking[0].split(',').nth(5),
        Some("reactive_strength_index")
    );

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&written).ok();
}

/// The closing help paragraph names exactly the relations an aggregated run writes. Reading
/// the paragraph after its own opening keeps a file named elsewhere on the page from passing.
#[test]
fn batch_help_names_all_nine_files_an_aggregated_run_writes() {
    let help = plateforce(&["batch", "--help"]);
    assert!(help.status.success(), "{}", said(&help));
    let page = String::from_utf8(help.stdout).expect("the help is UTF-8");
    let closing = page
        .split_once("--out-dir holds ")
        .map(|(_, paragraph)| paragraph)
        .unwrap_or_else(|| panic!("the help carries no closing output paragraph:\n{page}"));
    let mut named: Vec<String> = closing
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '.' && character != '_'
            })
        })
        .filter(|word| word.ends_with(".csv") || word.ends_with(".json"))
        .map(str::to_string)
        .collect();
    named.sort();
    named.dedup();

    let trials = cohort("help-files");
    let written = out_dir("help-files");
    let trials_named = trials.display().to_string();
    let target = written.display().to_string();
    let mut line = folder_run(&trials_named, &target);
    line.extend([
        "--aggregate",
        "mean_of_best_two",
        "--aggregate-n",
        "2",
        "--aggregate-ranked-by",
        "reactive_strength_index",
        "--aggregate-quantity",
        "jump_height_from_takeoff_meters",
    ]);
    let output = plateforce(&line);
    assert!(output.status.success(), "{}", said(&output));
    let mut actual: Vec<String> = std::fs::read_dir(&written)
        .expect("the output directory can be read")
        .map(|entry| {
            entry
                .expect("an output entry can be read")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    actual.sort();

    assert_eq!(
        actual.len(),
        9,
        "the aggregated run wrote {} files",
        actual.len()
    );
    assert_eq!(
        named.len(),
        9,
        "the closing paragraph names {} of {} written files: {named:?}",
        named.len(),
        actual.len()
    );
    assert_eq!(named, actual, "the help and the run name different files");

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&written).ok();
}

/// A word no published rule answers to is quoted back, so the caller can correct it.
#[test]
fn a_rule_nobody_published_is_refused_in_the_words_the_caller_used() {
    let trials = cohort("unpublished");
    let written = out_dir("unpublished");
    let named = trials.display().to_string();
    let target = written.display().to_string();
    let mut line = folder_run(&named, &target);
    line.extend(["--aggregate", "arithmetic_mean", "--aggregate-n", "3"]);
    let output = plateforce(&line);
    let told = said(&output);
    println!("{told}");

    assert!(!output.status.success(), "{told}");
    assert!(
        told.contains("arithmetic_mean"),
        "the refusal quotes the word the caller wrote, or they cannot correct it: {told}"
    );
    for published in ["best_of_n_by_peak_force", "mean_of_best_two"] {
        assert!(
            told.contains(published),
            "the refusal names {published}, which is what the caller may write instead: {told}"
        );
    }

    // The control, and it is the reason the assertion above means anything: naming nothing at
    // all is a different mistake and gets a different sentence. Both used to produce "the
    // request named none", so a caller who wrote arithmetic_mean was told they had written
    // nothing.
    let mut bare = folder_run(&named, &target);
    bare.extend(["--aggregate-n", "3"]);
    let nothing = said(&plateforce(&bare));
    println!("{nothing}");
    assert!(
        nothing.contains("named none") && !nothing.contains("arithmetic_mean"),
        "naming nothing says so, and says it differently: {nothing}"
    );

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&written).ok();
}

/// The run that cannot reduce says so, rather than returning a comparison and dropping it.
///
/// This is the defect the flag itself nearly shipped with: `--mode compare` beside `--aggregate`
/// answered with a comparison, wrote no reduction, exited 0 and said nothing.
#[test]
fn a_comparison_asked_to_reduce_refuses_rather_than_dropping_it() {
    let trials = cohort("compare");
    let written = out_dir("compare");
    let named = trials.display().to_string();
    let target = written.display().to_string();

    // The control first: the same comparison without a reduction runs, so the refusal below is
    // about the reduction and not about the comparison being malformed.
    let mut plain = folder_run(&named, &target);
    plain.extend([
        "--mode",
        "compare",
        "--against",
        "onset.threshold.last_within_band",
    ]);
    let ran = plateforce(&plain);
    assert!(
        ran.status.success(),
        "the comparison this test asks to reduce has to run on its own first: {}",
        said(&ran)
    );

    let mut line = folder_run(&named, &target);
    line.extend([
        "--mode",
        "compare",
        "--against",
        "onset.threshold.last_within_band",
        "--aggregate",
        "mean_of_best_two",
        "--aggregate-n",
        "2",
    ]);
    let output = plateforce(&line);
    let told = said(&output);
    println!("{told}");

    assert!(
        !output.status.success(),
        "a comparison asked for a reduction it cannot take exited 0, so the caller's request was \
         dropped without a word: {told}"
    );
    assert!(
        told.contains("aggregate") || told.contains("trial.aggregation"),
        "the refusal names the reduction that was asked for: {told}"
    );

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&written).ok();
}
