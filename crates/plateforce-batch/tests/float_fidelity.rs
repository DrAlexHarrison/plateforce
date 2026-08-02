//! A number read back is the number that was written.
//!
//! serde_json writes the shortest string that round-trips and its own parser does not always
//! read that string back to the same double. The write side was never the problem, which is
//! what makes this invisible to anyone who inspects only what was written, and what lets a
//! byte-equality check across surfaces pass while the parsed values differ.
//!
//! These assertions run over the shapes the surfaces actually exchange: a typed struct, a
//! `Vec<f64>`, and the batch envelope itself. An earlier version of this test measured
//! `serde_json::Value`, which is the one path the first attempted fix repaired and the one
//! path no surface here reads through.

mod common;

use common::{bound_request, committed_format, copy_committed_fixtures, registry, tempdir};
use plateforce_batch::{analyse, BatchResult, TrialIdentity, TrialSet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TypedRow {
    quantity: f64,
    optional: Option<f64>,
    series: Vec<f64>,
}

/// Deterministic doubles spanning ordinary magnitudes and the awkward exponents a plate can
/// produce, so the sweep is not quietly restricted to well-behaved values.
fn sweep(count: usize) -> Vec<f64> {
    let mut state = 1u64;
    let mut values = Vec::with_capacity(count);
    while values.len() < count {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let candidate = f64::from_bits(state);
        if candidate.is_finite() {
            values.push(candidate);
        }
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let ordinary = f64::from_bits((state >> 12) | (1023u64 << 52)) * 3.0;
        if ordinary.is_finite() && values.len() < count {
            values.push(ordinary);
        }
    }
    values
}

#[test]
fn a_typed_struct_round_trips_every_double_exactly() {
    let values = sweep(50_000);
    let mut lost = 0usize;
    for value in &values {
        let row = TypedRow {
            quantity: *value,
            optional: Some(*value),
            series: vec![*value],
        };
        let text = serde_json::to_string(&row).expect("a finite double serialises");
        let back: TypedRow = serde_json::from_str(&text).expect("and reads back");
        if back != row {
            lost += 1;
        }
    }
    println!(
        "typed struct round trip lost {lost} of {} doubles",
        values.len()
    );
    assert_eq!(
        lost, 0,
        "a number that changes when it is read is a number without a method"
    );
}

#[test]
fn a_bare_float_and_a_vector_of_them_round_trip_exactly() {
    let values = sweep(50_000);

    let mut bare = 0usize;
    for value in &values {
        let text = serde_json::to_string(value).unwrap();
        if serde_json::from_str::<f64>(&text).unwrap() != *value {
            bare += 1;
        }
    }
    let text = serde_json::to_string(&values).unwrap();
    let back: Vec<f64> = serde_json::from_str(&text).unwrap();
    let vector = values
        .iter()
        .zip(back.iter())
        .filter(|(left, right)| left != right)
        .count();

    println!(
        "bare f64 lost {bare} of {}, Vec<f64> lost {vector} of {}",
        values.len(),
        values.len()
    );
    assert_eq!(
        bare, 0,
        "typed deserialisation is the path every surface uses"
    );
    assert_eq!(vector, 0, "and so is a sequence of them");
}

#[test]
fn the_batch_envelope_survives_the_shapes_its_surfaces_read() {
    let directory = tempdir("float-envelope");
    let copied = copy_committed_fixtures(&directory);
    let set = TrialSet::walk(&directory, &committed_format(), &TrialIdentity::FileStem).unwrap();
    let result = analyse(&set, &bound_request(), &registry()).unwrap();
    assert_eq!(result.coverage.computed, copied, "every trial computed");

    // `from_json` reads the relations as typed structs, which is what the library, the
    // browser and Python each do with this string.
    let back = BatchResult::from_json(&result.to_json()).expect("the envelope reads back");
    assert_eq!(back.results, result.results);
    assert_eq!(back.provenance, result.provenance);
    assert_eq!(back.run, result.run);

    let compared: usize = result
        .results
        .iter()
        .map(|row| row.values.values().filter(|value| value.is_some()).count())
        .sum();
    println!("{compared} computed values across {copied} trials, all identical after a round trip");
    assert!(compared > 0, "there were values to compare");
    std::fs::remove_dir_all(&directory).ok();
}
