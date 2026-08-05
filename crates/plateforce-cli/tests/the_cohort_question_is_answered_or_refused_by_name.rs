//! Alex's cohort question, decomposed into the calls an agent makes, and what each one meets.
//!
//! `phase2/ROADMAP.md` section 8a names this question as M8's acceptance test: an agent, given
//! only the documentation and the terminal, answers it over a folder of jumps, every number it
//! reports names the rules and the cohort choices that produced it, and where the build cannot
//! answer it says which construct it lacks. This file is that test. It was written before the
//! milestone was built, so the milestone's progress is a query rather than a claim.
//!
//! **What it gates is not how many steps are answered.** It is that no step is silent. A step
//! the build cannot do refuses and names what it lacks; a step it can do produces the thing it
//! was asked for. Silence is the failure this product exists to prevent, and an agent on the
//! other end of a pipe cannot see a number it was given no reason for. The count of answered
//! steps is held to a committed floor instead, so it rises and never falls, and raising it is
//! what finishing M8 looks like.
//!
//! The cohort is generated arithmetic from `plateforce_batch::synthetic`, so nothing here is
//! athlete data. Only subject 01 is ever public and this question needs many athletes, so the
//! fixture had to be built rather than drawn from the corpus.

use std::path::{Path, PathBuf};
use std::process::Output;

/// The question, in Alex's own words, 2026-08-05. Every step below quotes the fragment it
/// carries, so a reader checks the decomposition against the sentence rather than trusting it.
const THE_QUESTION: &str = "what if we cut out the lowest two jumping athletes and top highest \
jumping and just looked at women athletes, what's the relationship between their time spent \
below bodyweight during a countermovement and their jump height for the top half of athletes \
compared to the bottom half of athletes separated by jump height?";

/// What one call met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// The call produced what the step asked for.
    Answered,
    /// The call declined, and its words name what the build lacks, so an agent can branch.
    RefusedByName,
    /// The call succeeded without producing what was asked for, or declined naming nothing an
    /// agent could act on. Either way the caller is left to guess, which is the failure.
    Silent,
}

/// Which of the two refusal channels answered, because they are not equally useful.
///
/// The registry's own vocabulary carries a code and an enumeration of what is available.
/// Argument parsing carries the token and nothing else, and it exits 64, which is also the exit
/// code of `decision_not_made` and `conventions_not_comparable`. So a step refused by the parser
/// is refused audibly and is not yet refused in the vocabulary M8 asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    Vocabulary,
    ArgumentParser,
    None,
}

impl Channel {
    fn of(text: &str) -> Channel {
        if text.contains("plateforce:") {
            Channel::Vocabulary
        } else if text.contains("unexpected argument") || text.starts_with("error:") {
            Channel::ArgumentParser
        } else {
            Channel::None
        }
    }

    fn label(self) -> &'static str {
        match self {
            Channel::Vocabulary => "registry vocabulary",
            Channel::ArgumentParser => "argument parser",
            Channel::None => "",
        }
    }
}

/// One call an agent makes on the way to the answer.
struct Step {
    /// The fragment of the question this call carries.
    fragment: &'static str,
    /// The token a refusal has to name. A refusal that does not contain it names nothing the
    /// agent can act on, which is silence wearing an exit code.
    asked_for: &'static str,
    /// What a successful answer contains. A call that exits 0 without this produced something
    /// other than what was asked for.
    evidence: &'static str,
}

fn plateforce(args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(args)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// Six athletes, three trials each, generated arithmetic at 1200 Hz.
///
/// Six rather than three because the question cuts two athletes off one end and one off the
/// other, so a cohort that a rank exclusion would empty could not tell a working exclusion from
/// a broken one.
///
/// **Named per test, not per process.** Keyed on the process id alone, the tests in this file
/// share one directory and run concurrently, so the first to finish removed the cohort the
/// others were still reading. That happened: a step meant to meet "this build does not carry
/// that construct" met "no such file or directory" instead, and the refusal was audible enough
/// to be counted as the refusal that was wanted. A test that passes for the wrong reason reads
/// exactly like one that passes.
fn cohort(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("plateforce-cohort-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&directory).ok();
    plateforce_batch::synthetic::write_corpus(&directory, 6, 3, 20_260_805)
        .expect("a generated cohort");
    directory
}

/// Where a run writes, and it must not share a prefix with `cohort`. It did: `scratch` clears
/// the directory it hands back, so one test's output directory removed its own cohort, the
/// folder run read an empty folder, and the control fell over. Two purposes, two names.
fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("plateforce-out-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).expect("a directory to write into");
    directory
}

/// The words every folder run over the generated cohort needs before it is asked anything.
fn over_the_cohort<'a>(trials: &'a str, out_dir: &'a str) -> Vec<&'a str> {
    vec![
        "--registry",
        "../../registry",
        "batch",
        trials,
        "--out-dir",
        out_dir,
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
        // Stated because the registry publishes this rule at six values and refuses to pick
        // one. Leaving it out is not a smaller request, it is a different question, and the
        // first run of this fixture met that refusal and was right to.
        "--set",
        "onset.k=5.0",
        "--takeoff",
        "takeoff.threshold.absolute_force",
    ]
}

/// A number in the answer, for the assertion that a step the build cannot do returns none.
///
/// Read off the quantity names the analysis publishes rather than off a shape like `= 0.42`,
/// because a usage message carries digits and a refusal that quoted one would read as a number.
const A_QUANTITY_IS_PRESENT: [&str; 4] = [
    "jump_height_from_takeoff_meters",
    "takeoff_velocity_meters_per_second",
    "net_impulse_newton_seconds",
    "flight_time_seconds",
];

fn classify(step: &Step, output: &Output) -> (Verdict, Channel, String) {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let both = format!("{stdout}\n{stderr}");
    let channel = Channel::of(stderr.trim());

    let verdict = if output.status.success() {
        if both.contains(step.evidence) {
            Verdict::Answered
        } else {
            // Exit 0 and the thing was not produced. The caller is told nothing at all, which
            // is the one outcome an agent cannot recover from.
            Verdict::Silent
        }
    } else if both.contains(step.asked_for) {
        // The refusal has to quote the token the caller used. Accepting any refusal from the
        // registry's own vocabulary was tried and is too generous: it counted "no such file or
        // directory" as the answer to "this build does not carry that construct", which is a
        // refusal about the fixture rather than about the question.
        Verdict::RefusedByName
    } else {
        Verdict::Silent
    };

    let first_line = both
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(96)
        .collect::<String>();
    (verdict, channel, first_line)
}

/// The whole question, call by call, in the order an agent would make them.
#[test]
fn every_call_on_the_way_to_the_answer_is_answered_or_names_what_it_lacks() {
    let trials = cohort("question");
    let trials_named = trials.display().to_string();
    let run_dir = scratch("run");
    let run_named = run_dir.display().to_string();

    println!("The question, verbatim:\n  {THE_QUESTION}\n");

    let mut verdicts: Vec<(&str, Verdict, Channel, String)> = Vec::new();

    // 1. Before anything else an agent asks what it may ask for. The operator rows are the half
    //    of that answer nothing else can carry: only the build knows that stating `selection` on
    //    a takeoff rule reaches `takeoff.op.crossing_selection`. How many calls it takes to learn
    //    the rest is measured separately below rather than judged here, because an answer that
    //    takes a hundred and eight calls is a cost and not a silence.
    let step = Step {
        fragment: "the agent's first call: what may I ask this build",
        asked_for: "capability",
        evidence: "operators",
    };
    let output = plateforce(&[
        "--registry",
        "../../registry",
        "capability",
        "--format",
        "json",
    ]);
    verdicts.push(record(&step, &output));

    // 2. The folder run itself, which is the table every later step reads. It is also this
    //    fixture's own precondition, so a base run that stops working says so in those words
    //    rather than arriving as a verdict about the question.
    let step = Step {
        fragment: "over a folder of jumps",
        asked_for: "batch",
        evidence: "AT01",
    };
    let output = plateforce(&over_the_cohort(&trials_named, &run_named));
    assert!(
        output.status.success(),
        "the folder run every later step reads no longer runs, so nothing below is a measurement \
         of the question: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    verdicts.push(record(&step, &output));

    // 3. One number per athlete, which every clause of the question is stated over. The
    //    registry publishes `trial.aggregation` with three incompatible rules, so which one
    //    runs is a method choice and not a mean.
    let step = Step {
        fragment: "athletes, rather than trials",
        asked_for: "aggregate",
        evidence: "aggregates.csv",
    };
    let mut line = over_the_cohort(&trials_named, &run_named);
    line.extend(["--aggregate", "mean_of_best_two", "--aggregate-n", "2"]);
    let output = plateforce(&line);
    verdicts.push(record(&step, &output));

    // 4. The metric the question is actually about, and the roadmap already measured that it is
    //    not a construct: phase boundaries compute and phase durations are not published
    //    quantities.
    let step = Step {
        fragment: "their time spent below bodyweight during a countermovement",
        asked_for: "time_below_bodyweight_seconds",
        evidence: "time_below_bodyweight_seconds",
    };
    let mut line = over_the_cohort(&trials_named, &run_named);
    line.extend([
        "--derive",
        "time_below_bodyweight_seconds=phase.duration.below_system_weight",
    ]);
    let output = plateforce(&line);
    verdicts.push(record(&step, &output));

    // 5. An athlete attribute, of which a folder run carries exactly one, mass. The attribute
    //    table is the piece that collides with the corpus constraint, so the intake has to be
    //    built such that the private path is the only path.
    let step = Step {
        fragment: "just looked at women athletes",
        asked_for: "subject-attribute",
        evidence: "subject-attribute",
    };
    let mut line = over_the_cohort(&trials_named, &run_named);
    line.extend(["--subject-attribute", "AT01=sex:female"]);
    let output = plateforce(&line);
    verdicts.push(record(&step, &output));

    // 6. Cutting the ends off a cohort. Published outlier conventions disagree, so which cut
    //    ran is itself a method choice that belongs in the record.
    let step = Step {
        fragment: "cut out the lowest two jumping athletes and top highest jumping",
        asked_for: "exclude",
        evidence: "exclusions.csv",
    };
    let mut line = over_the_cohort(&trials_named, &run_named);
    line.extend([
        "--exclude",
        "lowest:2:jump_height_from_takeoff_meters",
        "--exclude",
        "highest:1:jump_height_from_takeoff_meters",
    ]);
    let output = plateforce(&line);
    verdicts.push(record(&step, &output));

    // 7. A median split, which is one of several ways to cut a cohort and is known to throw
    //    information away, so the choice is recorded rather than assumed.
    let step = Step {
        fragment: "the top half of athletes compared to the bottom half separated by jump height",
        asked_for: "split",
        evidence: "split",
    };
    let mut line = over_the_cohort(&trials_named, &run_named);
    line.extend(["--split", "median:jump_height_from_takeoff_meters"]);
    let output = plateforce(&line);
    verdicts.push(record(&step, &output));

    // 8. The relation the question ends on. Pearson against Spearman changes the number and the
    //    claim, so a relation without its rule is the founding defect at the cohort level.
    let step = Step {
        fragment: "what's the relationship between their X and their Y",
        asked_for: "relate",
        evidence: "relate",
    };
    let mut line = over_the_cohort(&trials_named, &run_named);
    line.extend([
        "--relate",
        "time_below_bodyweight_seconds,jump_height_from_takeoff_meters",
    ]);
    let output = plateforce(&line);
    verdicts.push(record(&step, &output));

    println!(
        "\n{:<58} {:<16} channel, and the first line back",
        "step", "verdict"
    );
    for (fragment, verdict, channel, line) in &verdicts {
        println!(
            "{:<58} {:<16} {:<20} {}",
            truncate(fragment, 57),
            format!("{verdict:?}"),
            channel.label(),
            line
        );
    }

    let silent: Vec<&str> = verdicts
        .iter()
        .filter(|(_, verdict, _, _)| *verdict == Verdict::Silent)
        .map(|(fragment, _, _, _)| *fragment)
        .collect();
    let answered = verdicts
        .iter()
        .filter(|(_, verdict, _, _)| *verdict == Verdict::Answered)
        .count();

    println!(
        "\nanswered {answered} of {}, refused by name {}, silent {}",
        verdicts.len(),
        verdicts
            .iter()
            .filter(|(_, verdict, _, _)| *verdict == Verdict::RefusedByName)
            .count(),
        silent.len(),
    );

    assert!(
        silent.is_empty(),
        "{} of {} calls left the caller with nothing to act on, which is the one outcome an agent \
         cannot recover from: {silent:?}",
        silent.len(),
        verdicts.len(),
    );

    let floor_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cohort-question-floor.txt");
    let floor = std::fs::read_to_string(&floor_path).expect("the committed floor");
    let stated: Vec<&str> = floor.split_whitespace().collect();
    assert_eq!(stated.len(), 3, "the floor reads <answered> of <calls>");
    let floor_answered: usize = stated[0].parse().expect("a count");
    let floor_total: usize = stated[2].parse().expect("a denominator");

    assert_eq!(
        verdicts.len(),
        floor_total,
        "the question was recut into {} calls without the floor being restated in the same commit, \
         so the count above is measured against a denominator that no longer exists",
        verdicts.len(),
    );
    assert!(
        answered >= floor_answered,
        "the terminal answers {answered} of {} calls on the way to this question, below the \
         committed floor of {floor_answered}, so a step that used to be answerable no longer is",
        verdicts.len(),
    );

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&run_dir).ok();
}

/// A call the build cannot serve must not hand back a number anyway.
///
/// The sharpest way this milestone could fail is not a missing feature. It is a cohort operation
/// that quietly runs a near-enough substitute, because an agent has no way to tell that from the
/// thing it asked for, and it is the founding defect one level up.
#[test]
fn a_call_the_build_cannot_serve_hands_back_no_number() {
    let trials = cohort("no-number");
    let trials_named = trials.display().to_string();
    let run_dir = scratch("no-number");
    let run_named = run_dir.display().to_string();

    // The control, and it is the half that makes the rest mean anything: the same folder under
    // the same rules, asked for nothing it cannot do, does return numbers. Without it a run that
    // returned numbers for no reason at all would read exactly like a run that refused correctly.
    let served = plateforce(&over_the_cohort(&trials_named, &run_named));
    let served_text = String::from_utf8_lossy(&served.stdout).to_string();
    let served_quantities: Vec<&str> = A_QUANTITY_IS_PRESENT
        .iter()
        .filter(|name| served_text.contains(**name))
        .copied()
        .collect();
    assert!(
        !served_quantities.is_empty(),
        "the control run returned no quantity at all, so this test cannot see the difference \
         between a refusal and an empty answer: {}",
        String::from_utf8_lossy(&served.stderr),
    );
    println!(
        "control: a served folder run reports {} of {} quantities looked for",
        served_quantities.len(),
        A_QUANTITY_IS_PRESENT.len(),
    );

    let invented = scratch("invented");
    let invented_named = invented.display().to_string();
    let mut line = over_the_cohort(&trials_named, &invented_named);
    line.extend([
        "--derive",
        "time_below_bodyweight_seconds=phase.duration.below_system_weight",
    ]);
    let output = plateforce(&line);
    let text = String::from_utf8_lossy(&output.stdout).to_string();

    assert!(
        !output.status.success(),
        "a run asking for a construct this build does not carry exited 0, so an agent reading \
         only the exit code would take whatever came back as the answer to its question",
    );
    let leaked: Vec<&str> = A_QUANTITY_IS_PRESENT
        .iter()
        .filter(|name| text.contains(**name))
        .copied()
        .collect();
    assert!(
        leaked.is_empty(),
        "the refusal handed back {} of {} quantities anyway, and an agent cannot tell a number it \
         asked for from a number it did not: {leaked:?}",
        leaked.len(),
        A_QUANTITY_IS_PRESENT.len(),
    );
    println!("a refused construct returns no quantity, checked against {A_QUANTITY_IS_PRESENT:?}");

    std::fs::remove_dir_all(&trials).ok();
    std::fs::remove_dir_all(&run_dir).ok();
    std::fs::remove_dir_all(&invented).ok();
}

/// How many calls it takes an agent to learn what it may state on every rule this build runs.
///
/// M8's first requirement is one call that returns everything askable, and queue entry 1.1 is
/// open on the half of it that is missing: the manifest names every rule and the names each one
/// declines without, and it does not name the values a caller may state. An agent that wants
/// those has to ask the registry entry by entry, which works and costs one call per rule.
///
/// So this is a cost rather than a silence, and it is held to a ceiling rather than asserted at
/// one. Closing 1.1 drops the number to 1, and nothing else may raise it.
#[test]
fn learning_what_may_be_stated_costs_one_call_per_rule_until_the_manifest_carries_it() {
    let manifest = plateforce(&[
        "--registry",
        "../../registry",
        "capability",
        "--format",
        "json",
    ]);
    assert!(manifest.status.success(), "the manifest is readable");
    let text = String::from_utf8_lossy(&manifest.stdout).to_string();

    // Parsed off the wire rather than off the struct, because the question is what an agent
    // holding only the bytes can learn.
    let document: serde_json::Value = serde_json::from_str(&text).expect("a manifest");
    let methods = document["ok"]["methods"]
        .as_array()
        .expect("the manifest names its methods");
    let carrying_parameters = methods
        .iter()
        .filter(|row| row.get("parameters").is_some())
        .count();

    let calls = if carrying_parameters == methods.len() {
        1
    } else {
        1 + methods.len()
    };

    // The control, and without it a count of 108 could not be told from a route that does not
    // work at all: the per-entry call an agent would fall back to really does return the values
    // the manifest withholds. Taken over a rule the registry publishes several values for.
    let entry = plateforce(&[
        "--registry",
        "../../registry",
        "registry",
        "show",
        "onset.threshold.noise_relative",
        "--format",
        "json",
    ]);
    let entry_text = String::from_utf8_lossy(&entry.stdout).to_string();
    assert!(
        entry.status.success() && entry_text.contains("published_values"),
        "the per-entry route returns no published values, so the fallback this count assumes \
         does not exist and the number above measures nothing: {}",
        String::from_utf8_lossy(&entry.stderr),
    );

    println!(
        "calls to learn what may be stated on every rule: {calls}, over {} rules, of which {} \
         carry their parameters on the wire",
        methods.len(),
        carrying_parameters,
    );

    let ceiling_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/discovery-calls.txt");
    let stated = std::fs::read_to_string(&ceiling_path).expect("the committed ceiling");
    let words: Vec<&str> = stated.split_whitespace().collect();
    assert_eq!(words.len(), 3, "the ceiling reads <calls> for <rules>");
    let ceiling: usize = words[0].parse().expect("a count");
    let over: usize = words[2].parse().expect("a denominator");

    assert_eq!(
        methods.len(),
        over,
        "the build runs {} rules against a ceiling written for {over}, so the count above is \
         measured against a denominator that has moved and the ceiling has to be restated in the \
         same commit",
        methods.len(),
    );
    assert!(
        calls <= ceiling,
        "learning what may be stated now costs {calls} calls against a committed ceiling of \
         {ceiling}, so discovery got more expensive rather than less",
    );
}

fn record(step: &Step, output: &Output) -> (&'static str, Verdict, Channel, String) {
    let (verdict, channel, line) = classify(step, output);
    (step.fragment, verdict, channel, line)
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    text.chars()
        .take(width - 1)
        .chain(std::iter::once('.'))
        .collect()
}
