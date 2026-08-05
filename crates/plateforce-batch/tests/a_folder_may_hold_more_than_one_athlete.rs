//! One mass per athlete, over a folder holding several.
//!
//! A folder took one mass, reasoned as one athlete on one day. Every other field on this set
//! already spanned athletes: `TrialSet` says so in its own words, and `Session::group` takes
//! every reliability figure over the subject a declared pattern named. So a squad session was
//! already a folder this software understood, in every field but the mass.
//!
//! Run on generated traces. `synthetic` exists because the public fixtures are one subject,
//! and nothing it produces is athlete data, so a guard needing two distinct athletes can only
//! live here.

mod common;

use std::collections::BTreeMap;

use common::{bound_request, declared_pattern, registry, synthetic_format, tempdir};
use plateforce_batch::{analyse, TrialSet};

const SUBJECTS: usize = 3;
const TRIALS_EACH: usize = 2;

/// Masses far enough apart that a trial running under the wrong one is a different number
/// wherever a rule divides by it, rather than a rounding difference.
fn a_squads_masses() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("01".to_string(), 58.0),
        ("02".to_string(), 74.5),
        ("03".to_string(), 91.0),
    ])
}

fn a_folder(test: &str) -> TrialSet {
    let directory = tempdir(&format!("squad-{test}"));
    plateforce_batch::synthetic::write_corpus(&directory, SUBJECTS, TRIALS_EACH, 11).unwrap();
    TrialSet::walk(&directory, &synthetic_format(), &declared_pattern()).unwrap()
}

/// The record carries a mass per athlete, and carries it once.
#[test]
fn the_record_names_a_mass_for_each_athlete() {
    let set = a_folder("recorded");
    let result = analyse(
        &set,
        &bound_request().massing(a_squads_masses()),
        &registry(),
    )
    .expect("every choice was made");

    let masses = &result.run.body_mass_kilograms_by_subject;
    let named: Vec<(&str, f64)> = masses
        .iter()
        .map(|(subject, row)| (subject.as_str(), row.value))
        .collect();
    println!(
        "{} of {SUBJECTS} athletes carry a mass: {named:?}",
        masses.len()
    );
    assert_eq!(masses.len(), SUBJECTS);
    assert_eq!(masses["01"].value, 58.0);
    assert_eq!(masses["03"].value, 91.0);
    assert_eq!(masses["02"].unit, "kilograms");
    assert_eq!(
        masses["02"].source,
        plateforce_core::provenance::ParameterSource::Stated,
        "a mass the operator typed is not the software's own"
    );

    // One home. A folder stating masses per athlete states no mass for the folder, so a
    // reader cannot meet two answers to one question.
    let folder_wide: Vec<&str> = result
        .run
        .bound_globals
        .iter()
        .map(|row| row.name.as_str())
        .filter(|name| name.contains("body_mass"))
        .collect();
    println!("folder-wide mass rows: {folder_wide:?}");
    assert!(folder_wide.is_empty(), "{folder_wide:?}");
}

/// The mass each trial's analysis actually ran under, not the mass the record says it ran
/// under.
///
/// The two guards either side of this one read the record, and the record would say the same
/// thing if every trial ran under one athlete's mass or none. `declaration.computed_on_object`
/// is the rule that turns the stated mass into a number, so binding it puts the value where a
/// table can be read for it: an athlete's trials answer their own mass and nobody else's.
#[test]
fn each_athletes_trials_run_under_that_athletes_mass() {
    let set = a_folder("computed");
    let mut request = bound_request().massing(a_squads_masses());
    request.analysis.derived.insert(
        "mechanical_object".to_string(),
        plateforce_analysis::MethodChoice {
            method_id: "declaration.computed_on_object".to_string(),
            options: BTreeMap::from([("object".to_string(), "body".to_string())]),
            ..Default::default()
        },
    );
    let result = analyse(&set, &request, &registry()).expect("every choice was made");

    // The subject a trial belongs to comes from the set that named it, so the expectation is
    // built from the folder rather than from the trial ids this test happens to know.
    let mut answered: BTreeMap<String, Vec<Option<f64>>> = BTreeMap::new();
    for row in &result.results {
        let subject = set
            .get(&row.trial_id)
            .and_then(|entry| entry.subject.as_ref())
            .map(|key| key.subject.clone())
            .expect("a declared pattern named every trial's athlete");
        answered
            .entry(subject)
            .or_default()
            .push(row.values["mechanical_object_mass_kilograms"]);
    }

    println!("mass computed per athlete: {answered:?}");
    assert_eq!(answered.len(), SUBJECTS, "{answered:?}");
    for (subject, kilograms) in a_squads_masses() {
        let mine = &answered[&subject];
        assert_eq!(mine.len(), TRIALS_EACH, "{subject}: {mine:?}");
        assert!(
            mine.iter().all(|value| *value == Some(kilograms)),
            "{subject} should answer {kilograms}, and answered {mine:?}"
        );
    }
}

/// The other shape. A folder of one athlete still states one mass, on the row it always used.
#[test]
fn a_folder_of_one_athlete_still_states_one_mass() {
    let set = a_folder("one-athlete");
    let mut request = bound_request();
    request.analysis.body_mass_kilograms = Some(61.5);
    let result = analyse(&set, &request, &registry()).expect("every choice was made");

    let folder_wide: Vec<f64> = result
        .run
        .bound_globals
        .iter()
        .filter(|row| row.name.contains("body_mass"))
        .map(|row| row.value)
        .collect();
    println!("folder-wide mass rows: {folder_wide:?}");
    assert_eq!(folder_wide, vec![61.5]);
    assert!(result.run.body_mass_kilograms_by_subject.is_empty());
}

/// Two squads at two sets of masses are two runs. Without this the masses could be recorded
/// beside the numbers and reach nothing that identifies them.
#[test]
fn two_squads_at_different_masses_are_two_requests() {
    let mut heavier = a_squads_masses();
    heavier.insert("02".to_string(), 80.0);

    let one = analyse(
        &a_folder("digest-light"),
        &bound_request().massing(a_squads_masses()),
        &registry(),
    )
    .expect("every choice was made");
    let other = analyse(
        &a_folder("digest-heavy"),
        &bound_request().massing(heavier),
        &registry(),
    )
    .expect("every choice was made");

    println!("one   {}", one.run.request_digest);
    println!("other {}", other.run.request_digest);
    assert_ne!(
        one.run.request_digest, other.run.request_digest,
        "one athlete's mass moved and the request that ran answered the same way"
    );
}

/// A mass written against a name the folder does not hold applies to nothing. Refused rather
/// than accepted, because a typo that silently covers no trial is the failure this record
/// exists to stop.
#[test]
fn a_mass_for_an_athlete_who_is_not_here_is_refused() {
    let mut mistyped = a_squads_masses();
    mistyped.insert("13".to_string(), 66.0);

    let refused = analyse(
        &a_folder("unknown-subject"),
        &bound_request().massing(mistyped),
        &registry(),
    )
    .expect_err("a mass for nobody is refused");

    println!("{}", refused.message);
    assert_eq!(refused.code, plateforce_core::RefusalCode::ValueNotAccepted);
    assert!(refused.message.contains("13"), "{}", refused.message);
    assert!(refused.message.contains("01"), "{}", refused.message);
}

/// And the other direction. An athlete the masses do not cover would run at no mass while the
/// record beside them lists one for everybody else, which reads as coverage.
#[test]
fn an_athlete_the_masses_do_not_cover_is_refused() {
    let mut partial = a_squads_masses();
    partial.remove("02");

    let refused = analyse(
        &a_folder("uncovered-subject"),
        &bound_request().massing(partial),
        &registry(),
    )
    .expect_err("an athlete with no mass is refused");

    println!("{}", refused.message);
    assert_eq!(
        refused.code,
        plateforce_core::RefusalCode::RequiredParameterUnstated
    );
    assert!(
        refused.message.contains("1 of 3 subjects"),
        "the count carries its denominator: {}",
        refused.message
    );
    assert!(refused.message.contains("02"), "{}", refused.message);
}

/// Masses keyed by subject over a folder whose identity names no subject cover nothing, and
/// the refusal says which of the two the reader has to change.
#[test]
fn masses_by_subject_over_a_folder_with_no_pattern_are_refused() {
    let directory = tempdir("squad-no-pattern");
    plateforce_batch::synthetic::write_corpus(&directory, SUBJECTS, TRIALS_EACH, 11).unwrap();
    let set = TrialSet::walk(
        &directory,
        &synthetic_format(),
        &plateforce_batch::TrialIdentity::FileStem,
    )
    .unwrap();

    let refused = analyse(
        &set,
        &bound_request().massing(a_squads_masses()),
        &registry(),
    )
    .expect_err("a folder with no subject cannot key a mass by one");

    println!("{}", refused.message);
    assert!(refused.message.contains("pattern"), "{}", refused.message);
}

/// Every row names the athlete it belongs to, so a cohort question can group the table.
///
/// The run resolved the subject to route the mass; the table dropped it, so anyone grouping by
/// athlete had to re-parse `trial_id` against the pattern, which is this software's own
/// identity rule reimplemented by its caller and free to disagree with it.
#[test]
fn every_row_names_the_athlete_it_belongs_to() {
    let set = a_folder("subject-column");
    let result = analyse(
        &set,
        &bound_request().massing(a_squads_masses()),
        &registry(),
    )
    .expect("the folder runs");

    let named: Vec<&str> = result
        .results
        .iter()
        .map(|row| row.subject.as_str())
        .collect();
    assert!(
        named.iter().all(|subject| !subject.is_empty()),
        "{} of {} rows name no athlete, so grouping by athlete drops them silently",
        named.iter().filter(|subject| subject.is_empty()).count(),
        named.len(),
    );

    let distinct: std::collections::BTreeSet<&str> = named.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        SUBJECTS,
        "the table names {} athletes over a folder holding {SUBJECTS}: {distinct:?}",
        distinct.len(),
    );

    // The column is the run's own answer rather than a re-reading of the file name, so it
    // agrees with the mass routing, which is the other consumer of the same resolution.
    for row in &result.results {
        assert!(
            a_squads_masses().contains_key(&row.subject),
            "the table names an athlete the mass map does not: {}",
            row.subject,
        );
    }
    println!(
        "rows {} over {} athletes, every row named",
        result.results.len(),
        distinct.len()
    );
}

/// A run that declared no pattern names no athlete, rather than inventing one per trial.
///
/// The control for the test above: without it, a column filled with the trial id would satisfy
/// every assertion there and would be wrong in the one way that matters.
#[test]
fn a_run_with_no_declared_pattern_names_no_athlete_rather_than_inventing_one() {
    let directory = tempdir("no-pattern");
    plateforce_batch::synthetic::write_corpus(&directory, SUBJECTS, TRIALS_EACH, 11).unwrap();
    let set = TrialSet::walk(
        &directory,
        &synthetic_format(),
        &plateforce_batch::TrialIdentity::FileStem,
    )
    .unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("the folder runs");

    assert!(
        !result.results.is_empty(),
        "the run produced no rows, so this proves nothing about the column",
    );
    let invented: Vec<&str> = result
        .results
        .iter()
        .map(|row| row.subject.as_str())
        .filter(|subject| !subject.is_empty())
        .collect();
    assert!(
        invented.is_empty(),
        "{} rows name an athlete over a run that declared no grouping: {invented:?}",
        invented.len(),
    );
}
