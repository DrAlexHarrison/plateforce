//! Two published methods over one folder of trials, and how far apart they are.

mod common;

use common::{
    analysis_request, bound_request, committed_format, copy_committed_fixtures, declared_pattern,
    registry, synthetic_format, tempdir,
};
use plateforce_batch::agreement::{
    bland_altman, bound_statistic_ids, correlation_with_limits, guard_same_repetition, olp,
    pairs_from,
};
use plateforce_batch::{
    analyse, bind_statistic, compare, AgreementRefusal, BatchCompareRequest, BatchRequest,
    LimitsRequest, TrialIdentity, TrialSet,
};
use plateforce_core::DispersionEstimator;

const HEIGHT: &str = "jump_height_from_takeoff_meters";
const TWO_ONSET_RULES: [&str; 2] = [
    "onset.threshold.noise_relative",
    "onset.threshold.relative_to_system_weight",
];

fn compare_request() -> BatchCompareRequest {
    BatchCompareRequest {
        analysis: BatchRequest::new(analysis_request(1.0)).resolving(&[
            "system_weight",
            "movement_onset",
            "takeoff",
        ]),
        slot: "onset".to_string(),
        method_ids: TWO_ONSET_RULES.iter().map(|id| id.to_string()).collect(),
        quantity: HEIGHT.to_string(),
    }
}

#[test]
fn paired_relation_is_one_row_per_trial_per_method() {
    let directory = tempdir("agreement-paired");
    plateforce_batch::synthetic::write_corpus(&directory, 5, 4, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();

    let result = compare(&set, &compare_request());
    println!("{}", result.coverage());
    assert_eq!(result.trial_count, 20);
    assert_eq!(
        result.paired.len(),
        40,
        "two methods over twenty trials, and a variant that failed stays in the denominator"
    );
    assert!(
        result.paired.iter().all(|row| !row.subject.is_empty()),
        "a declared pattern carries the subject onto every paired row"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// Sweeping a slot releases a marker the caller pinned on it, so the swept values are the
/// rule's own. A sweep that honoured the pin would return one index for every method and read
/// perfect agreement on data that has none, which is the one reading this statistic must never
/// produce.
#[test]
fn a_swept_slot_reports_the_rules_own_values_and_not_a_pinned_index() {
    let directory = tempdir("agreement-pinned-marker");
    plateforce_batch::synthetic::write_corpus(&directory, 3, 3, 7).unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();

    let free = compare(&set, &compare_request());
    let mut pinned_request = compare_request();
    pinned_request.analysis = BatchRequest::new({
        let mut request = analysis_request(1.0);
        request.onset.manual_index = Some(1300);
        request
    })
    .resolving(&["system_weight", "movement_onset", "takeoff"]);
    let pinned = compare(&set, &pinned_request);

    let values = |result: &plateforce_batch::BatchCompareResult| -> Vec<Option<f64>> {
        result.paired.iter().map(|row| row.value).collect()
    };
    let distinct: std::collections::BTreeSet<String> = values(&pinned)
        .iter()
        .map(|value| format!("{value:?}"))
        .collect();
    println!(
        "{} paired rows under a pinned onset, {} distinct values",
        pinned.paired.len(),
        distinct.len()
    );

    assert_eq!(
        values(&pinned),
        values(&free),
        "the sweep read the rules rather than the pin"
    );
    assert!(
        distinct.len() > 1,
        "and the rules disagree here, so equality above was not two runs of one number"
    );
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn pairs_from_one_trace_satisfy_the_design_and_record_it() {
    let directory = tempdir("agreement-one-trace");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    // One trace in, two methods over it, so every pair comes from one repetition by
    // construction. The guard is satisfied here rather than promised.
    let pairs = pairs_from(&result).expect("the run produced pairs");
    println!("{} pairs from {} trials", pairs.len(), result.trial_count);
    assert_eq!(pairs.len(), copied);
    assert!(bind_statistic("agreement.design.simultaneous_capture").is_some());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn pairs_from_two_files_are_refused_and_named() {
    let left = vec![
        ("subject01_trial1".to_string(), 0.41),
        ("subject01_trial2".to_string(), 0.43),
    ];
    let same = left.clone();
    assert!(guard_same_repetition(&left, &same).is_ok());

    let elsewhere = vec![
        ("subject01_trial1".to_string(), 0.41),
        ("subject01_trial9".to_string(), 0.44),
    ];
    let refusal = guard_same_repetition(&left, &elsewhere)
        .expect_err("agreement across two repetitions is not agreement");
    let message = refusal.message();
    println!("{message}");
    assert!(matches!(
        refusal,
        AgreementRefusal::NotTheSameRepetition { .. }
    ));
    assert!(message.contains("subject01_trial2"), "{message}");
    assert!(message.contains("subject01_trial9"), "{message}");
}

#[test]
fn bland_altman_refuses_when_neither_required_parameter_is_stated() {
    let refusal = LimitsRequest::declared(None, None)
        .expect_err("both are required with no registry default");
    let message = refusal.message();
    println!("{message}");
    assert!(message.contains("unit_of_analysis"), "{message}");
    assert!(message.contains("dispersion"), "{message}");
    assert!(
        message.contains("subject"),
        "the legal values are named: {message}"
    );
    assert!(message.contains("population"), "{message}");
}

#[test]
fn the_subject_unit_of_analysis_needs_a_declared_grouping() {
    let directory = tempdir("agreement-subject-unit");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    // Taking trials where the design is repeated measures inflates the count and reports a
    // tighter agreement than the data supports, which is why the entry carries the parameter.
    let request = LimitsRequest::declared(Some("subject"), Some("sample")).unwrap();
    let refusal = bland_altman(&set, &result, request).expect_err("no pattern, no subject");
    println!("{}", refusal.message());
    assert_eq!(refusal, AgreementRefusal::SubjectUnitWithoutGrouping);

    let over_trials = LimitsRequest::declared(Some("trial"), Some("sample")).unwrap();
    let limits = bland_altman(&set, &result, over_trials).expect("trials are available");
    println!(
        "bias {:.6} m, limits {:.6} to {:.6}, n = {} of {}",
        limits.bias, limits.lower, limits.upper, limits.n, result.trial_count
    );
    assert_eq!(limits.n, copied);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_correlation_arrives_with_its_limits_or_not_at_all() {
    let directory = tempdir("agreement-correlation");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());
    let pairs = pairs_from(&result).unwrap();

    let together = correlation_with_limits(&pairs, DispersionEstimator::Sample)
        .expect("the pairs support both");
    // There is no accessor for the correlation on its own: the only way out is `both()`, so
    // the refusal is structural rather than a runtime check somebody can route around.
    let (correlation, limits) = together.both();
    println!(
        "r = {correlation:.6}, bias {:.6}, limits {:.6} to {:.6}, n = {}",
        limits.bias,
        limits.lower,
        limits.upper,
        together.n()
    );
    assert_eq!(together.n(), limits.n);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn ordinary_least_products_runs_over_the_same_pairs() {
    let directory = tempdir("agreement-olp");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    let fit = olp(&result, DispersionEstimator::Sample).expect("the pairs support a fit");
    println!(
        "slope {:.6}, intercept {:.6}, n = {} of {}",
        fit.slope, fit.intercept, fit.n, result.trial_count
    );
    assert_eq!(fit.n, copied);
    assert!(fit.slope.is_finite());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn headline_shape() {
    let directory = tempdir("agreement-headline");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let registry = registry();

    let result = compare(&set, &compare_request());
    let request = LimitsRequest::declared(Some("trial"), Some("sample")).unwrap();
    let limits = bland_altman(&set, &result, request).unwrap();

    // The digest comes from a run over the same set, so the figure names the registry it was
    // taken against rather than resting on a caller's word.
    let analysed = analyse(&set, &bound_request(), &registry).unwrap();

    println!(
        "bias {:.9} m, limits {:.9} to {:.9}, methods {} and {}, digest {}, n = {} of {copied}",
        limits.bias,
        limits.lower,
        limits.upper,
        TWO_ONSET_RULES[0],
        TWO_ONSET_RULES[1],
        analysed.run.registry_digest,
        limits.n
    );

    // The shape and the provenance are asserted, never the value. The number six committed
    // trials produce is not this project's headline, which is a median spread across ten
    // published methods on 244 trials, and asserting one here would be its first failure
    // mode committed in a test.
    assert_eq!(limits.n, copied);
    assert!(limits.lower <= limits.bias && limits.bias <= limits.upper);
    assert!(analysed.run.registry_digest.starts_with("content-"));
    assert_eq!(result.method_ids.len(), 2);
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn every_statistic_id_resolves_in_one_table_and_the_registry_carries_it() {
    let registry = registry();
    let ids = bound_statistic_ids();
    let present: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| registry.methods.contains_key(*id))
        .collect();
    println!(
        "{} of {} bound statistic ids resolve in the registry",
        present.len(),
        ids.len()
    );
    for id in &ids {
        println!(
            "  {id}  {}",
            if present.contains(id) {
                "resolves"
            } else {
                "no entry"
            }
        );
    }
    // A rule that reported one id when it worked and another when it did not is the defect
    // this table exists to prevent, so every id it holds is bindable.
    for id in &ids {
        assert!(bind_statistic(id).is_some(), "{id} resolves in the table");
    }
    assert!(!present.is_empty(), "the registry carries these entries");
}

#[test]
fn every_paired_value_reaches_the_rules_that_produced_it() {
    let directory = tempdir("agreement-paired-chain");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());
    assert!(
        copied > 0 && !result.paired.is_empty(),
        "the run produced pairs"
    );

    // A paired value with no chain is a number whose method nobody recorded, which is the
    // thing this product exists to prevent, one level up from a single trial.
    assert!(
        result
            .paired
            .iter()
            .all(|row| !row.provenance_id.is_empty()),
        "every paired row is keyed"
    );

    let distinct: std::collections::BTreeSet<&str> = result
        .paired
        .iter()
        .map(|row| row.provenance_id.as_str())
        .collect();
    println!(
        "{} distinct chains over {} paired rows from {} trials, {} provenance rows",
        distinct.len(),
        result.paired.len(),
        result.trial_count,
        result.provenance.len()
    );
    assert_eq!(
        distinct.len(),
        TWO_ONSET_RULES.len(),
        "one chain per swept rule, shared across every trial that ran it"
    );

    // Each chain reaches back to the onset rule the variant swept.
    for rule in TWO_ONSET_RULES {
        assert!(
            result
                .provenance
                .iter()
                .any(|entry| entry.method_id == rule),
            "{rule} is named in the chain"
        );
    }
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_compare_run_leaves_the_machine_with_its_record_beside_it() {
    let directory = tempdir("agreement-compare-writer");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());
    assert!(
        copied > 0 && !result.paired.is_empty(),
        "the run produced pairs"
    );

    let registry = registry();
    let out = directory.join("out");
    let written = result
        .write_csv(&out, &registry.content_digest, "content-request")
        .expect("the directory takes them");

    let names: Vec<String> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    println!("wrote {}", names.join(", "));
    assert!(names.contains(&"compare-run.json".to_string()));
    assert!(names.contains(&"paired.csv".to_string()));

    // The table joins back to the chain, so a reader of paired.csv reaches the rules.
    let paired = std::fs::read_to_string(out.join("paired.csv")).unwrap();
    let header = paired.lines().next().unwrap();
    println!("{header}");
    assert!(header.contains("provenance_id"), "{header}");
    assert!(header.contains("method_ids"), "{header}");
    assert_eq!(
        paired.lines().count(),
        1 + result.paired.len(),
        "the header and one row per paired value"
    );

    // Every count in the record is one the run measured rather than one it was told.
    let run: plateforce_batch::agreement::CompareRunRow =
        serde_json::from_str(&std::fs::read_to_string(out.join("compare-run.json")).unwrap())
            .unwrap();
    println!(
        "compare over {} trials: {} paired rows, {} failed, {} complete pairs, {} chains",
        run.trial_count,
        run.paired_rows,
        run.failed_rows,
        run.complete_pairs,
        run.distinct_provenance_count
    );
    assert_eq!(run.trial_count, copied);
    assert_eq!(run.paired_rows, result.paired.len());
    assert_eq!(run.method_ids.len(), 2);
    assert_eq!(run.distinct_provenance_count, 2);
    assert!(run.registry_digest.starts_with("content-"));
    std::fs::remove_dir_all(&directory).ok();
}

/// Every refusal this crate emits answers to a published code, so a caller reading a batch
/// row and a caller reading a compare result meet the same word for the same failure. The
/// list is built from the variants rather than typed, so a new one cannot ship uncoded.
#[test]
fn every_agreement_refusal_carries_a_published_code() {
    let every = [
        AgreementRefusal::RequiredParametersUnstated {
            parameters: vec!["unit_of_analysis".to_string()],
            legal: vec!["trial".to_string()],
        },
        AgreementRefusal::NotTheSameRepetition {
            pairs: vec!["a against b".to_string()],
        },
        AgreementRefusal::SubjectUnitWithoutGrouping,
        AgreementRefusal::ConventionsDiffer {
            left: "sample".to_string(),
            right: "population".to_string(),
        },
        AgreementRefusal::NotEnoughPairs { had: 1, needs: 2 },
    ];

    let published: std::collections::BTreeSet<&str> = plateforce_core::RefusalCode::ALL
        .iter()
        .map(|code| code.wire_name())
        .collect();
    // The pairing itself, because a code that resolves and discriminates can still be the
    // wrong one: swapping `not_enough_observations` for `trace_too_short` keeps both of those
    // properties and tells a caller their recording is short when their group is small.
    let expected = [
        "required_parameter_unstated",
        "observations_not_paired",
        "required_parameter_unstated",
        "conventions_not_comparable",
        "not_enough_observations",
    ];
    assert_eq!(every.len(), expected.len(), "one expectation per variant");

    for (refusal, want) in every.iter().zip(expected) {
        let code = refusal.code();
        println!("{:38} {}", code.wire_name(), refusal.message());
        assert!(
            published.contains(code.wire_name()),
            "{} is not a code this build publishes",
            code.wire_name()
        );
        assert_eq!(
            code.wire_name(),
            want,
            "this fault answers to {want}: {}",
            refusal.message()
        );
        assert!(!refusal.message().is_empty(), "and it says what happened");
    }

    // Distinctness as well, so a mapping that answered everything the same way would fail
    // here even if every expectation above were rewritten to match it.
    let distinct: std::collections::BTreeSet<&str> =
        every.iter().map(|r| r.code().wire_name()).collect();
    println!(
        "{} distinct codes over {} refusals",
        distinct.len(),
        every.len()
    );
    assert!(
        distinct.len() >= 4,
        "these five faults are not one fault: {distinct:?}"
    );
}

/// The limits entry names its dispersion as required with no default, so a request that
/// states one must be computed under it. Stating a value the binding then ignores is the same
/// fault as defaulting through the requirement, one step later and harder to see.
#[test]
fn the_dispersion_the_request_states_is_the_one_the_limits_use() {
    use plateforce_core::agreement::limits_of_agreement;

    let directory = tempdir("agreement-dispersion");
    copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());
    let pairs = pairs_from(&result).expect("the run produced pairs");

    // Stated the way a caller states them, through the same parsing a request goes through.
    for (name, stated) in [
        ("sample", DispersionEstimator::Sample),
        ("population", DispersionEstimator::Population),
    ] {
        let expected = limits_of_agreement(&pairs, stated).expect("the pairs support limits");
        let request = LimitsRequest::declared(Some("trial"), Some(name)).expect("both stated");
        let through = bland_altman(&set, &result, request).expect("trials are available");
        println!(
            "{stated:?}: limits {:.9} to {:.9}",
            through.lower, through.upper
        );
        assert_eq!(
            through.upper, expected.upper,
            "{name} was stated and the limits were taken under something else"
        );
    }

    // The two estimators have to differ on this many pairs, or the check above could not
    // tell them apart and would pass whichever one the binding used.
    let sample = limits_of_agreement(&pairs, DispersionEstimator::Sample).unwrap();
    let population = limits_of_agreement(&pairs, DispersionEstimator::Population).unwrap();
    println!("{} pairs, and the two estimators differ", pairs.len());
    assert_ne!(sample.upper, population.upper);
    std::fs::remove_dir_all(&directory).ok();
}

/// A variant that failed is listed with its reason and stays in the denominator. A sweep that
/// dropped it would report agreement between the methods that happened to work, over a count
/// nobody could check.
#[test]
fn a_variant_that_could_not_run_stays_in_the_denominator_with_its_reason() {
    let directory = tempdir("compare-failed-variant");
    // A trace that stands on the plate and never leaves it, so an onset rule looking for a
    // departure finds none and its variant fails while the run continues.
    let standing: String = (0..3600)
        .map(|sample| format!("{}\n", 700.0 + ((sample % 7) as f64) * 0.05))
        .collect();
    std::fs::write(directory.join("standing.force.txt"), standing).unwrap();
    let copied = copy_committed_fixtures(&directory);

    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = compare(&set, &compare_request());

    let trials = copied + 1;
    let failed: Vec<&str> = result
        .paired
        .iter()
        .filter(|row| !row.failure_reason.is_empty())
        .map(|row| row.trial_id.as_str())
        .collect();
    println!(
        "{} paired rows over {} trials x {} methods, {} carrying a reason",
        result.paired.len(),
        trials,
        TWO_ONSET_RULES.len(),
        failed.len()
    );

    assert_eq!(
        result.paired.len(),
        trials * TWO_ONSET_RULES.len(),
        "every trial contributes a row per method, whether or not the method ran"
    );
    assert!(
        !failed.is_empty(),
        "the standing trace gives at least one variant nothing to find, or this check is \
         watching a run where nothing failed"
    );
    for row in result.paired.iter().filter(|r| r.trial_id == "standing") {
        assert!(
            row.value.is_some() || !row.failure_reason.is_empty(),
            "a row either carries a value or says why it does not: {row:?}"
        );
        // The code beside the sentence, so a script reading this export branches on it
        // rather than matching prose. Held against the engine's own vocabulary, so a column
        // filled with a word no surface publishes is a failure rather than a value.
        if !row.failure_reason.is_empty() {
            assert!(
                plateforce_core::RefusalCode::ALL
                    .iter()
                    .any(|code| code.wire_name() == row.failure_code),
                "'{}' is not a code this build publishes: {row:?}",
                row.failure_code
            );
        }
        if row.value.is_some() {
            assert!(row.failure_code.is_empty(), "a value that arrived: {row:?}");
        }
    }
    std::fs::remove_dir_all(&directory).ok();
}

/// A compare run says what it was pointed at, what it read, and what it swept.
///
/// A folder of traces beside files that are not traces is the ordinary case, and the record
/// a compare run leaves behind is read on its own, away from the folder, so a run that
/// reported only the traces would describe its own narrowing as the folder's contents. The
/// swept step belongs on the same record: naming the rules without naming the step they
/// filled says which rules ran and not what they were compared as.
#[test]
fn a_compare_run_records_what_it_walked_and_which_step_it_swept() {
    let directory = tempdir("compare-record");
    let copied = copy_committed_fixtures(&directory);
    let strays = ["README.md", "session.log"];
    for name in strays {
        std::fs::write(directory.join(name), "not a trace\n").unwrap();
    }
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();

    let result = compare(&set, &compare_request());
    let line = result.coverage();
    println!("{line}");
    assert!(
        line.contains(&format!("files {},", copied + strays.len())),
        "{line}"
    );
    assert!(
        line.contains(&format!(
            "{copied} carrying a declared trial suffix and {} not",
            strays.len()
        )),
        "{line}"
    );

    let record = result.run_row("content-registry", "content-request");
    assert_eq!(record.slot, "onset", "the record names the step it swept");
    assert_eq!(record.files_found, copied);
    assert_eq!(record.files_without_declared_suffix, strays.len());
    assert_eq!(record.trial_count, copied);
    std::fs::remove_dir_all(&directory).ok();
}

/// A file the identity could not name is refused by name on a compare run as it is on an
/// analyse run. Dropping it would narrow the population a paired statistic rests on with
/// nothing on the record saying so.
#[test]
fn a_compare_run_refuses_an_unnamed_file_rather_than_dropping_it() {
    let directory = tempdir("compare-unnamed");
    plateforce_batch::synthetic::write_corpus(&directory, 3, 3, 7).unwrap();
    std::fs::write(directory.join("stray_trace.txt"), "600.0\n600.0\n").unwrap();
    let set = TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap();

    let result = compare(&set, &compare_request());
    println!("{}", result.coverage());
    assert_eq!(set.unidentified.len(), 1, "the pattern names eight of nine");
    assert_eq!(result.files_unidentified, 1);

    let named: Vec<&plateforce_batch::RefusalRow> = result
        .refusals
        .iter()
        .filter(|row| row.parameter.contains("stray_trace") || row.message.contains("stray_trace"))
        .collect();
    assert_eq!(
        named.len(),
        1,
        "the file the pattern did not match is on the record: {:?}",
        result.refusals
    );
    assert_eq!(named[0].code, "trial_identity_unparsed");
    std::fs::remove_dir_all(&directory).ok();
}
