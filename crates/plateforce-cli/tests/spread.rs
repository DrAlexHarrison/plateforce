//! Sweeping one quantity over every rule this build runs for each step on its path.
//!
//! `analyse` reports this for the jump height without being asked. This command reports it
//! for any other quantity, and it answers the same open choices first, so a sweep is never a
//! way to get a number the analysis itself would have refused to give.

use std::process::Output;

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn spread(extra: &[&str], rules_named: bool) -> Output {
    let mut arguments: Vec<&str> = vec![
        "--registry",
        "../../registry",
        "spread",
        FIXTURE,
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
    ];
    if rules_named {
        arguments.extend([
            "--weighing",
            "bwepoch.fixed_window",
            "--set",
            "weighing.duration=1.0",
            "--onset",
            "onset.threshold.noise_relative",
            "--set",
            "onset.k=5",
            "--takeoff",
            "takeoff.threshold.absolute_force",
            "--set",
            "takeoff.threshold_n=20",
        ]);
    }
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn body(output: &Output) -> serde_json::Value {
    let text = String::from_utf8(output.stdout.clone()).expect("the document is UTF-8");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
    parsed["ok"].clone()
}

/// Every figure carries what it was taken over, so a percentage is never read without the
/// count behind it.
#[test]
fn the_sweep_reports_what_ran_and_what_did_not() {
    let output = spread(&["--format", "json"], true);
    assert_eq!(output.status.code(), Some(0));
    let response = body(&output);

    let run = response["combinations_run"].as_u64().expect("a count");
    let succeeded = response["succeeded"].as_u64().expect("a count");
    let failed = response["failed"].as_u64().expect("a count");
    println!("succeeded {succeeded}, failed {failed}, of combinations_run {run}");
    assert!(run > 1, "a sweep over one combination is not a sweep");
    assert_eq!(
        succeeded + failed,
        run,
        "every combination is accounted for"
    );
}

/// A quantity other than the one `analyse` headlines, which is the reason this command
/// exists rather than a flag on that one.
#[test]
fn a_quantity_the_analysis_does_not_headline_can_be_swept() {
    let output = spread(
        &["--format", "json", "--quantity", "time_to_takeoff_seconds"],
        true,
    );
    assert_eq!(output.status.code(), Some(0));
    let response = body(&output);
    assert_eq!(response["quantity_key"], "time_to_takeoff_seconds");
    assert_eq!(response["unit_symbol"], "s");

    let headline = body(&spread(&["--format", "json"], true));
    assert_eq!(headline["quantity_key"], "jump_height_from_takeoff_meters");
}

/// The same rail `analyse` meets, from the same code, rather than a second one that could
/// answer differently.
#[test]
fn an_unanswered_choice_refuses_here_exactly_as_it_does_in_the_analysis() {
    let output = spread(&[], false);
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(said.contains("have no default"), "{said}");
    assert!(said.contains("--onset"), "{said}");
}

/// A quantity nothing computes is refused rather than swept into an empty answer.
#[test]
fn a_quantity_this_build_does_not_compute_is_refused() {
    let output = spread(&["--quantity", "nothing.computes.this"], true);
    println!("exit {:?}", output.status.code());
    assert_ne!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty(), "no number is published");
}

/// The constructs a sweep says it varied, which is how a narrowed sweep is told from the
/// whole one rather than by reading a count.
fn varied(response: &serde_json::Value) -> Vec<String> {
    response["axes_varied"]
        .as_array()
        .expect("the record says what it varied")
        .iter()
        .map(|axis| axis["construct"].as_str().expect("a construct").to_string())
        .collect()
}

fn held(response: &serde_json::Value) -> Vec<String> {
    response["held_fixed"]
        .as_array()
        .expect("the record says what it held")
        .iter()
        .map(|rule| {
            rule["construct"]
                .as_str()
                .expect("a construct")
                .to_string()
        })
        .collect()
}

/// One step named, and the sweep is over that step. The terminal swept every bound step with
/// more than one rule and had no way to say otherwise, while the tab has a tick per step and
/// Python and R take `slot=`.
///
/// The control is the same command without the flag, which has to vary all three, or a
/// narrowing that narrowed nothing would read the same as one that worked.
#[test]
fn a_named_step_is_the_only_one_the_sweep_varies() {
    let whole = body(&spread(&["--format", "json"], true));
    assert_eq!(
        varied(&whole),
        ["system_weight", "movement_onset", "takeoff"],
        "the unnarrowed sweep is not over three steps, so narrowing to one proves nothing"
    );

    let narrowed = body(&spread(&["--format", "json", "--slot", "onset"], true));
    assert_eq!(varied(&narrowed), ["movement_onset"]);
    assert_eq!(
        narrowed["combinations_run"].as_u64(),
        Some(5),
        "the five onset rules are what this build runs"
    );
    assert!(
        whole["combinations_run"].as_u64() > narrowed["combinations_run"].as_u64(),
        "the narrowed sweep ran as many combinations as the whole one"
    );
    // The other half of the same question: a step left out is held, and the record says so.
    let held = held(&narrowed);
    assert!(
        held.contains(&"system_weight".to_string()) && held.contains(&"takeoff".to_string()),
        "the steps this sweep did not vary are not recorded as held: {held:?}"
    );
}

/// Several steps, which is the question a reader asks about a number resting on more than
/// one rule, and the wording Python's `slot=` already takes.
#[test]
fn several_steps_can_be_named_and_the_order_they_were_named_in_does_not_reach_the_document() {
    let asked = body(&spread(
        &["--format", "json", "--slot", "onset", "--slot", "takeoff"],
        true,
    ));
    assert_eq!(varied(&asked), ["movement_onset", "takeoff"]);
    assert_eq!(
        asked["combinations_run"].as_u64(),
        Some(25),
        "five onset rules by five takeoff rules"
    );

    // One sweep is one document whichever order the caller named the steps in. Before the
    // binding table decided it, the same sweep from the tab and from the terminal differed in
    // 520 paths of `variants`.
    let reversed = body(&spread(
        &["--format", "json", "--slot", "takeoff", "--slot", "onset"],
        true,
    ));
    assert_eq!(
        serde_json::to_string(&asked).expect("the document serialises"),
        serde_json::to_string(&reversed).expect("the document serialises"),
    );
    assert!(
        asked["succeeded"].as_u64().is_some_and(|count| count > 0),
        "no combination produced a value, so the two documents agree about nothing in them"
    );
}

/// A step this run did not bind is refused by name, with what could have been asked instead.
///
/// The build runs three rules for peak force, so the refusal is about this request rather
/// than about the build, and the pair below tells the two apart.
#[test]
fn a_step_this_run_did_not_bind_is_refused_rather_than_swept_over_a_rule_nobody_chose() {
    let output = spread(&["--format", "json", "--slot", "peak_force"], true);
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty(), "no number is published");
    assert!(said.contains("peak_force"), "{said}");
    assert!(said.contains("onset"), "{said}");

    // Bound, the same construct is a step this run can be asked about, so the refusal above
    // is about what this request carries and not a name the terminal cannot spell.
    let bound = spread(
        &[
            "--format",
            "json",
            "--derive",
            "peak_force=force.peak.net",
            "--slot",
            "peak_force",
        ],
        true,
    );
    assert_eq!(bound.status.code(), Some(0));
    assert_eq!(varied(&body(&bound)), ["peak_force"]);
}

/// A step this build runs one rule for is refused rather than quietly dropped.
///
/// Dropped, the command runs, reports a spread over whatever was left, and heads it with the
/// quantity the caller asked about, which reads as an answer to the question they put. Time
/// to takeoff is reached one way in this build, so naming it is a question with no answer.
#[test]
fn a_step_with_one_rule_is_refused_rather_than_dropped_from_a_sweep_that_still_reports() {
    let output = spread(
        &[
            "--format",
            "json",
            "--derive",
            "time_to_takeoff=time_to_takeoff.onset_to_takeoff",
            "--slot",
            "time_to_takeoff",
        ],
        true,
    );
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty(), "no number is published");
    assert!(said.contains("one rule"), "{said}");
    assert!(said.contains("time_to_takeoff"), "{said}");
}

/// The construct the panel prints is a name this flag answers to, because a reader narrowing
/// a sweep is reading `varied system_weight` off the panel above it.
#[test]
fn the_construct_the_record_names_is_a_word_this_flag_takes() {
    let by_flag_word = body(&spread(&["--format", "json", "--slot", "weighing"], true));
    let by_construct = body(&spread(
        &["--format", "json", "--slot", "system_weight"],
        true,
    ));
    assert_eq!(varied(&by_flag_word), ["system_weight"]);
    assert_eq!(
        serde_json::to_string(&by_flag_word).expect("the document serialises"),
        serde_json::to_string(&by_construct).expect("the document serialises"),
    );
}

/// One step is one axis, so a name stated twice is refused rather than squaring the
/// combinations and labelling every one of them with the step twice over.
#[test]
fn a_step_named_twice_is_refused() {
    let output = spread(
        &["--format", "json", "--slot", "onset", "--slot", "onset"],
        true,
    );
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty(), "no number is published");
    assert!(said.contains("twice"), "{said}");
}

/// The sweep records the registry revision its caller cited.
///
/// This surface accepts `--registry-version`, prints in its own help that the result will name
/// the revision, and discarded it: measured against `analyse` under the identical flag, the
/// analysis carried the pin and the sweep carried null. The panel that answers how far the
/// choice of method moves a number was the one surface that could not say which registry
/// produced its answer.
///
/// The control is the same sweep without the flag, which must carry null. Asserting only the
/// pinned side would pass on a surface writing the string into every document.
#[test]
fn a_pinned_sweep_names_the_revision_and_an_unpinned_one_names_none() {
    let pinned = spread(
        &["--format", "json", "--registry-version", "PIN-2026-08-04"],
        true,
    );
    assert_eq!(pinned.status.code(), Some(0));
    let pinned = body(&pinned);

    let unpinned = spread(&["--format", "json"], true);
    assert_eq!(unpinned.status.code(), Some(0));
    let unpinned = body(&unpinned);

    assert_eq!(
        pinned["registry_version"].as_str(),
        Some("PIN-2026-08-04"),
        "the sweep dropped the revision its caller cited"
    );
    assert!(
        unpinned.get("registry_version").is_some(),
        "the document does not carry registry_version at all"
    );
    assert!(
        unpinned["registry_version"].is_null(),
        "an unpinned sweep named {}, which reads as a revision somebody chose",
        unpinned["registry_version"]
    );
}

/// What the registry claims about itself rides beside the pin rather than inside it.
///
/// The sweep's document carried no such field while an analysed result did, so the two
/// documents this build writes about one registry answered a different number of questions.
#[test]
fn the_sweep_names_what_the_registry_declares_without_laundering_it_into_the_pin() {
    let pinned = body(&spread(
        &["--format", "json", "--registry-version", "PIN-2026-08-04"],
        true,
    ));
    let unpinned = body(&spread(&["--format", "json"], true));

    let declared = unpinned["registry_declared_version"].as_str();
    assert!(
        declared.is_some(),
        "the sweep does not say what the registry declares about itself"
    );
    assert_eq!(
        pinned["registry_declared_version"].as_str(),
        declared,
        "the registry's own claim moved when the caller's pin did"
    );
    assert_ne!(
        pinned["registry_declared_version"].as_str(),
        Some("PIN-2026-08-04"),
        "the caller's pin was written into the registry's claim"
    );

    // The digest is measured from the files that were read, so it is there whether or not
    // anybody pinned anything, and it is the control that says this sweep read a registry at
    // all rather than reporting three nulls.
    assert!(
        unpinned["registry_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("content-")),
        "the sweep names no registry digest, so the two nulls above say nothing"
    );
}
