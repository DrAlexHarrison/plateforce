//! A file held apart by runs of blanks is readable, and the record says so in the file's
//! own terms rather than as a character the file does not contain.

mod common;

use common::{bound_request, registry};
use plateforce_batch::{analyse, SourceFormat, TrialIdentity, TrialSet};
use plateforce_core::read::FieldSeparator;

fn whitespace_format() -> SourceFormat {
    SourceFormat {
        delimiter: FieldSeparator::Whitespace,
        force_column_index: 1,
        sample_rate_hz: 1200.0,
        trial_file_suffixes: vec!["txt".to_string()],
        sentinel: None,
    }
}

/// Column-aligned with runs of spaces, the shape lab exports pad into.
fn padded_rows() -> String {
    (0..2400)
        .map(|index| format!("{:>8}   {:>10.4}\n", index, 586.2 + (index % 7) as f64))
        .collect()
}

#[test]
fn a_column_behind_runs_of_spaces_is_read_and_recorded_as_whitespace() {
    let set = TrialSet::from_sources(
        vec![("padded.txt".to_string(), padded_rows())],
        &whitespace_format(),
        &TrialIdentity::FileStem,
    )
    .unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("the run answers");

    println!(
        "trials {}, computed {}, run.delimiter {:?}",
        result.run.trial_count, result.run.computed_count, result.run.delimiter
    );
    assert_eq!(result.run.trial_count, 1);
    assert_eq!(
        result.run.delimiter, "whitespace",
        "the record names the class the file is held apart by"
    );
    // The second column was read as force: a padded index column read as force would put
    // the trace nowhere near a plate's newtons.
    let row = &result.results[0];
    assert!(!row.provenance_id.is_empty(), "{row:?}");
}

/// The wire spellings, both directions. A tab's request and a terminal's construction meet
/// this one field, so the spellings are pinned where they are parsed.
#[test]
fn the_wire_takes_a_character_the_word_whitespace_or_nothing() {
    let read = |spelled: &str| -> Result<SourceFormat, String> {
        serde_json::from_str(&format!(
            r#"{{"delimiter":{spelled},"force_column_index":0,"sample_rate_hz":1200.0,"sentinel":null,"trial_file_suffixes":["txt"]}}"#
        ))
        .map_err(|error| error.to_string())
    };

    assert_eq!(
        read(r#""\t""#).unwrap().delimiter,
        FieldSeparator::Character('\t')
    );
    assert_eq!(
        read(r#""whitespace""#).unwrap().delimiter,
        FieldSeparator::Whitespace
    );
    assert_eq!(read(r#""""#).unwrap().delimiter, FieldSeparator::WholeRow);
    assert_eq!(
        read(r#""\u0000""#).unwrap().delimiter,
        FieldSeparator::WholeRow
    );

    let refused = read(r#""  ""#).expect_err("two characters name no separator");
    assert!(refused.contains("whitespace"), "{refused}");

    // Out again the way it came in, so a saved request replays as the request it was.
    for separator in ["\"\\t\"", "\"whitespace\"", "\"\""] {
        let format = read(separator).unwrap();
        let written = serde_json::to_string(&format).unwrap();
        let back: SourceFormat = serde_json::from_str(&written).unwrap();
        assert_eq!(
            back.delimiter, format.delimiter,
            "{separator} did not round-trip"
        );
    }
}

/// The suffix filter and the identity are unchanged by the separator kind, so the run's
/// population arithmetic still holds under the new spelling.
#[test]
fn a_whitespace_run_keeps_its_population_arithmetic() {
    let set = TrialSet::from_sources(
        vec![
            ("padded.txt".to_string(), padded_rows()),
            ("README.md".to_string(), String::new()),
        ],
        &whitespace_format(),
        &TrialIdentity::FileStem,
    )
    .unwrap();
    let result = analyse(&set, &bound_request(), &registry()).expect("the run answers");
    result.run.check_invariants().expect("the run's own counts");
    assert_eq!(result.run.files_without_declared_suffix, 1);
}
