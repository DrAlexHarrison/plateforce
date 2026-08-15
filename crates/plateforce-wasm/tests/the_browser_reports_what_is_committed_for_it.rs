//! The row committed for the browser, checked against the crate the bundle is built from.
//!
//! `scripts/capability.sh` asks the built bundle, which is the reader's own copy and can be
//! older than the crate beside it. That is the drift it exists to catch, and it needs a wasm
//! toolchain and a bundle to run. This asks the source, in a plain `cargo test`, so a change
//! to what this crate can do fails here on the machine that made it rather than in a workflow.
//!
//! The two together are what pins the bundle: each is compared against the same committed
//! row, so a bundle built from another commit differs from it rather than agreeing with a
//! source it was never built from.

use plateforce_wasm::capability_json;

fn reported() -> serde_json::Value {
    let text = capability_json().expect("the manifest serialises");
    serde_json::from_str::<serde_json::Value>(&text).expect("the manifest parses")["ok"].clone()
}

fn committed() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CAPABILITY.json"
    ))
    .expect("CAPABILITY.json is committed");
    serde_json::from_str::<serde_json::Value>(&text).expect("the manifest parses")["surfaces"]
        ["browser"]
        .clone()
}

/// Name the field that moved rather than printing both documents: the arrays run to thousands
/// of characters and a reader comparing them by eye is reading the wrong thing.
fn differences(committed: &serde_json::Value, reported: &serde_json::Value) -> Vec<String> {
    let empty = serde_json::Map::new();
    let left = committed.as_object().unwrap_or(&empty);
    let right = reported.as_object().unwrap_or(&empty);
    let mut keys: Vec<&String> = left.keys().chain(right.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|key| left.get(*key) != right.get(*key))
        .map(|key| match (left.get(key), right.get(key)) {
            (Some(serde_json::Value::Array(was)), Some(serde_json::Value::Array(now))) => {
                let gone: Vec<&serde_json::Value> =
                    was.iter().filter(|item| !now.contains(item)).collect();
                let new: Vec<&serde_json::Value> =
                    now.iter().filter(|item| !was.contains(item)).collect();
                format!("{key}: no longer reports {gone:?}, now reports {new:?}")
            }
            (was, now) => format!("{key}: committed {was:?}, reports {now:?}"),
        })
        .collect()
}

#[test]
fn this_crate_reports_what_is_committed_for_the_browser() {
    let committed = committed();
    // A control first: a row read from a key nobody carries is an empty object, and every
    // comparison against one reads as a document that moved rather than as a lookup that
    // missed.
    assert!(
        committed.get("operations").is_some(),
        "no row is committed under surfaces.browser, so this test compares against nothing"
    );

    let moved = differences(&committed, &reported());
    println!(
        "fields committed for the browser: {}; moved: {}",
        committed.as_object().map(serde_json::Map::len).unwrap_or(0),
        moved.len()
    );
    assert!(
        moved.is_empty(),
        "regenerate with scripts/capability.sh --write and audit the diff:\n  {}",
        moved.join("\n  ")
    );
}
