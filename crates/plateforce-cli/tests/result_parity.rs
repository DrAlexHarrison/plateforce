//! One result, and whether it survives the trip out and back.
//!
//! Two different properties, and a check for one is not a check for the other. Byte equality
//! says every surface wrote the same characters. Value equality says a caller who reads those
//! characters holds the number that was written. `serde_json`'s writer emits the shortest
//! string that round-trips, so the text is always right; its parser was not correctly rounded
//! until `float_roundtrip`, so the double read back could differ on about one value in ten.
//!
//! The consequence for a parity gate is exact: **the wire text does not change either way**,
//! so a comparison of bytes across surfaces passes while the surfaces hold different numbers.
//! A baseline captured under a broken parser would match too. So this asserts on bit patterns.

use std::process::Command;

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

/// Every rule on the path named, so nothing here is a refusal and every metric carries a
/// value to compare.
fn result_json() -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
            "--registry",
            "../../registry",
            "--format",
            "json",
            "analyse",
            FIXTURE,
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
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
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the result is UTF-8")
}

/// Every number anywhere in the document, with the path that reaches it, so a failure names
/// the field rather than an offset.
fn numbers(value: &serde_json::Value, path: &str, found: &mut Vec<(String, f64)>) {
    match value {
        serde_json::Value::Number(number) => {
            if let Some(as_f64) = number.as_f64() {
                found.push((path.to_string(), as_f64));
            }
        }
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                numbers(child, &format!("{path}.{key}"), found);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                numbers(child, &format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

/// Every number literal as it stands in the text, read by the standard library's own
/// correctly-rounded parser. This is the second opinion the check below needs.
fn literals_as_written(document: &str) -> Vec<u64> {
    let mut found = Vec::new();
    let bytes = document.as_bytes();
    let mut index = 0;
    let mut inside_a_string = false;
    while index < bytes.len() {
        let byte = bytes[index];
        // Citation keys carry years and method ids carry digits, so a scan that does not
        // track string boundaries reads `owen2014` as the number 2014.
        if byte == b'"' && !(index > 0 && bytes[index - 1] == b'\\') {
            inside_a_string = !inside_a_string;
            index += 1;
            continue;
        }
        if inside_a_string {
            index += 1;
            continue;
        }
        let starts_a_number = byte.is_ascii_digit()
            || (byte == b'-' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit));
        if !starts_a_number {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_digit() || matches!(bytes[index], b'.' | b'e' | b'E'))
        {
            // An exponent sign belongs to the number; a minus anywhere else starts the next.
            if matches!(bytes[index], b'e' | b'E')
                && matches!(bytes.get(index + 1), Some(b'+') | Some(b'-'))
            {
                index += 1;
            }
            index += 1;
        }
        if let Ok(value) = document[start..index].parse::<f64>() {
            found.push(value.to_bits());
        }
    }
    found.sort_unstable();
    found
}

/// Two parsers over one text, compared on bit patterns.
///
/// Reading the document twice and comparing the results proves nothing: both reads go
/// through the same parser, so a parser that is wrong is wrong identically and the two agree.
/// The second opinion has to come from somewhere else, and `str::parse` is correctly rounded
/// where `serde_json`'s own parser was not until `float_roundtrip`. Bit patterns rather than
/// a tolerance, because the defect moves a value by one place in the last digit and every
/// tolerance a person would reach for forgives it.
#[test]
fn a_reader_of_this_result_holds_the_number_that_was_written() {
    let written = result_json();
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("the result parses");

    let mut through_serde_json = Vec::new();
    numbers(&parsed, "ok", &mut through_serde_json);
    let mut serde_bits: Vec<u64> = through_serde_json
        .iter()
        .map(|(_, value)| value.to_bits())
        .collect();
    serde_bits.sort_unstable();

    let standard_library = literals_as_written(&written);
    println!(
        "numbers in the result: {}; read the same by both parsers: {} of {}",
        standard_library.len(),
        serde_bits
            .iter()
            .zip(standard_library.iter())
            .filter(|(left, right)| left == right)
            .count(),
        standard_library.len()
    );
    assert_eq!(serde_bits.len(), standard_library.len());
    assert_eq!(
        serde_bits, standard_library,
        "a value in this document does not survive being read back"
    );
}

/// The text is the same characters whichever parser is linked, which is why the check above
/// exists at all: this one passes even when the numbers do not survive.
#[test]
fn the_document_is_the_same_characters_after_a_round_trip() {
    let written = result_json();
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("the result parses");
    let rewritten = serde_json::to_string(&parsed).expect("a parsed value serialises");
    assert_eq!(written.trim_end(), rewritten);
}

/// A jump height that moves between two runs of the same request is a number without a
/// method, whatever produced it.
#[test]
fn the_same_request_twice_produces_the_same_document() {
    assert_eq!(result_json(), result_json());
}
