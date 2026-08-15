//! The parity manifest, checked from the surface that produced it.
//!
//! The shell harness asks every surface; this asks one, so a change to what this binary can
//! do fails a plain `cargo test` rather than waiting for a workflow.

use std::process::Command;

fn reported() -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["capability", "--format", "json"])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("the manifest is UTF-8");
    serde_json::from_str::<serde_json::Value>(&text).expect("the manifest parses")["ok"].clone()
}

fn manifest() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CAPABILITY.json"
    ))
    .expect("CAPABILITY.json is committed");
    serde_json::from_str(&text).expect("the manifest parses")
}

/// What this surface answered last time it was asked, rather than a document every surface
/// shares: which entry points a surface dispatches is a fact about that surface.
fn committed() -> serde_json::Value {
    manifest()["surfaces"]["cli"].clone()
}

/// Name the field that moved rather than printing both documents: the arrays run to
/// thousands of characters and a reader comparing them by eye is reading the wrong thing.
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

/// A committed record rather than a comparison against another surface: two surfaces wrong
/// the same way agree with each other, and every change here is a diff a reviewer sees.
#[test]
fn this_surface_reports_what_is_committed_for_it() {
    let moved = differences(&committed(), &reported());
    assert!(
        moved.is_empty(),
        "regenerate with scripts/capability.sh --write and audit the diff:\n  {}",
        moved.join("\n  ")
    );
}

#[test]
fn the_manifest_names_one_method_per_rule_this_build_runs() {
    let committed = committed();
    let methods = committed["methods"]
        .as_array()
        .expect("methods is an array");
    println!(
        "methods in the manifest: {} of {} bindings",
        methods.len(),
        plateforce_analysis::BINDINGS.len()
    );
    assert_eq!(methods.len(), plateforce_analysis::BINDINGS.len());
}

/// The arrays are what a surface may be asked for. An interaction state cannot be one of
/// them, so a surface with no provisional mode is not a surface that fails this gate.
#[test]
fn a_surface_answers_for_the_arrays_and_no_others() {
    let committed = committed();
    let mut keys: Vec<&str> = committed
        .as_object()
        .expect("a surface's record holds an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "acquisition",
            "methods",
            "operations",
            "operators",
            "output_formats",
            "plateforce_version",
            "refusal_codes",
            "schema"
        ]
    );
}

/// Every operator entry this build composes is on the wire, under the construct whose rules
/// reach it, with the names a caller states to reach it.
///
/// A caller never names an operator: it arrives because the rule they picked composed it. So
/// the manifest is the only place a chooser learns that stating `selection` on a takeoff rule
/// reaches the takeoff crossing entry and not the onset one, and before this it learned
/// nothing, because no id containing `.op.` appeared anywhere in the document.
#[test]
fn every_operator_entry_this_build_composes_is_published_with_the_names_that_reach_it() {
    let published = reported();
    let rows = published["operators"]
        .as_array()
        .expect("the surface publishes the operators its rules compose");

    let declared: usize = plateforce_analysis::ONSET_OPERATOR_IDS.len()
        + plateforce_analysis::TAKEOFF_OPERATOR_IDS.len();
    println!(
        "{} operator rows published, {declared} declared",
        rows.len()
    );
    assert_eq!(rows.len(), declared);

    // Named rather than counted. A row per entry with the wrong construct on it counts the
    // same and sends a chooser to state a name on a rule that never had it.
    let reached: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["construct"].as_str().expect("a construct"),
                row["entry"].as_str().expect("an entry"),
            )
        })
        .collect();
    assert!(
        reached.contains(&("takeoff", "takeoff.op.crossing_selection")),
        "{reached:?}"
    );
    assert!(
        reached.contains(&("movement_onset", "onset.op.crossing_selection")),
        "{reached:?}"
    );

    let names_for = |entry: &str| -> Vec<String> {
        rows.iter()
            .find(|row| row["entry"] == entry)
            .expect("the entry is published")["states"]
            .as_array()
            .expect("the names that reach it")
            .iter()
            .map(|name| name.as_str().expect("a name").to_string())
            .collect()
    };
    assert_eq!(names_for("takeoff.op.crossing_selection"), ["selection"]);
    // The one entry two names reach, so a row carrying a single name would read as complete.
    assert_eq!(
        names_for("onset.op.search_upper_bound"),
        ["bound", "search_bound_seconds"]
    );
}

/// Every member the acquisition block holds, written out rather than read from
/// `Acquisition::MEMBERS`.
///
/// Both sides of a comparison against the same const agree while five members become four,
/// and a manifest naming four sends a reader to find four. A member the block gains passes
/// this list and is caught by the second direction in the test below.
const MEMBERS_A_READER_IS_TOLD_TO_FIND: [&str; 5] = [
    "filter_at_capture",
    "tare_state",
    "plate_natural_frequency_hz",
    "floor_surface",
    "firmware_version",
];

/// What the plate and its settings were, which no reanalysis recovers, named in full by the
/// surface a reader is standing at.
#[test]
fn the_manifest_names_every_member_of_the_acquisition_block() {
    let record = committed();
    let published: Vec<&str> = record["acquisition"]["members"]
        .as_array()
        .expect("the acquisition block names its members")
        .iter()
        .map(|member| member.as_str().expect("a member is a name"))
        .collect();

    let unpublished: Vec<&&str> = MEMBERS_A_READER_IS_TOLD_TO_FIND
        .iter()
        .filter(|member| !published.contains(member))
        .collect();
    assert!(
        unpublished.is_empty(),
        "{} of {} members are named nowhere in the manifest: {unpublished:?}",
        unpublished.len(),
        MEMBERS_A_READER_IS_TOLD_TO_FIND.len()
    );

    let undeclared: Vec<&&str> = plateforce_core::Acquisition::MEMBERS
        .iter()
        .filter(|member| !published.contains(member))
        .collect();
    let invented: Vec<&&str> = published
        .iter()
        .filter(|member| !plateforce_core::Acquisition::MEMBERS.contains(member))
        .collect();
    println!(
        "acquisition members published: {} of {} the block declares",
        published.len(),
        plateforce_core::Acquisition::MEMBERS.len()
    );
    assert!(
        undeclared.is_empty(),
        "the block holds {undeclared:?} and the manifest does not name them"
    );
    assert!(
        invented.is_empty(),
        "the manifest names {invented:?} and the block holds no such member"
    );
}

/// The block this binary says it takes is the block it reads.
///
/// A flag clap draws and nothing reads drops the value on the floor while the record still
/// reports what the user typed. The manifest is derived from clap's tree alone, so the call
/// that consumes the flag is the one witness outside that derivation.
#[test]
fn the_block_this_binary_declares_is_the_block_it_reads() {
    let source_directory = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut source = String::new();
    for entry in std::fs::read_dir(source_directory).expect("the crate has sources") {
        let path = entry.expect("a readable entry").path();
        if path.extension().is_some_and(|kind| kind == "rs") {
            source.push_str(&std::fs::read_to_string(&path).expect("a readable source"));
        }
    }

    // A control first: a scan that read nothing reports the flag as never read, which looks
    // exactly like a binary that draws it and drops it.
    assert!(
        source.contains("long = \"acquisition\""),
        "the scan read no source, so its verdict means nothing"
    );

    let read_by_a_command = source.contains("acquisition_arg::stated_acquisition(");
    let declared = committed()["acquisition"]["stated_by_caller"]
        .as_bool()
        .expect("a surface says whether its caller can state the block");
    println!("acquisition block declared: {declared}; read by a command: {read_by_a_command}");
    assert_eq!(
        declared,
        read_by_a_command,
        "the manifest says the block is {}, and {} command reads one",
        if declared {
            "stated here"
        } else {
            "absent here"
        },
        if read_by_a_command { "a" } else { "no" }
    );
}

/// Every way this software can decline maps onto one of the three `sysexits` values a shell
/// reads, and this binary never exits 2 for any of them.
///
/// Membership rather than a value per code: which of 64 and 65 a given refusal deserves is a
/// ruling that belongs to whoever owns the variant, and a guard that pinned each one would be
/// asserting an answer it does not own.
#[test]
fn every_refusal_code_carries_an_exit_status_a_shell_reads() {
    let committed = committed();
    let codes = committed["refusal_codes"]
        .as_array()
        .expect("refusal_codes is an array");
    let named: Vec<i64> = codes
        .iter()
        .map(|record| {
            record["exit_code"]
                .as_i64()
                .expect("an exit code is a number")
        })
        .collect();
    let recognised = named
        .iter()
        .filter(|code| matches!(code, 64 | 65 | 66 | 78))
        .count();
    println!(
        "refusal codes: {}; carrying a sysexits status: {} of {}",
        codes.len(),
        recognised,
        named.len()
    );
    assert_eq!(codes.len(), plateforce_core::RefusalCode::ALL.len());
    assert_eq!(recognised, named.len(), "{codes:#?}");
    assert!(!named.contains(&2), "this binary never exits 2");
}

/// The one array that is a ruling rather than a measurement. Regenerating it from the
/// surfaces would make it the union of whatever they happen to do, which none could fail.
#[test]
fn every_operation_owed_is_one_this_surface_reaches() {
    let manifest = manifest();
    let required: Vec<&str> = manifest["required_operations"]
        .as_array()
        .expect("required_operations is an array")
        .iter()
        .map(|name| name.as_str().expect("an operation is a name"))
        .collect();
    let here = committed();
    let reached: Vec<&str> = here["operations"]
        .as_array()
        .expect("operations is an array")
        .iter()
        .map(|name| name.as_str().expect("an operation is a name"))
        .collect();
    let unmet: Vec<&&str> = required
        .iter()
        .filter(|name| !reached.contains(name))
        .collect();
    println!(
        "operations owed by every surface: {}; reached here: {} of {}",
        required.len(),
        required.len() - unmet.len(),
        required.len()
    );
    assert!(!required.is_empty(), "an empty obligation asserts nothing");
    assert!(unmet.is_empty(), "unmet: {unmet:?}");
}

/// The one array that cannot be derived from a table, held against the calls that produce
/// it rather than against a list kept beside it.
///
/// A container this binary writes and does not declare is invisible to a comparison against
/// a committed file, because both sides of that comparison come from the declaration. The
/// only thing outside it is the call.
#[test]
fn every_container_this_binary_writes_is_one_it_declares() {
    let source_directory = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut source = String::new();
    for entry in std::fs::read_dir(source_directory).expect("the crate has sources") {
        let path = entry.expect("a readable entry").path();
        if path.extension().is_some_and(|kind| kind == "rs") {
            source.push_str(&std::fs::read_to_string(&path).expect("a readable source"));
        }
    }

    // A control first: a pattern that finds nothing reports every container as undeclared
    // and a pattern that finds everything reports none, and the two read the same here.
    assert!(
        source.contains("write_csv("),
        "the scan read no source, so its verdict means nothing"
    );

    let calls = [
        ("csv", source.contains("write_csv(")),
        ("parquet", source.contains("write_parquet(")),
        (
            "json",
            source.contains("write_json(") || source.contains("Format::Json"),
        ),
        ("markdown", source.contains("markdown::result(")),
    ];
    let declared: Vec<String> = committed()["output_formats"]
        .as_array()
        .expect("output_formats is an array")
        .iter()
        .map(|name| name.as_str().expect("a container").to_string())
        .collect();

    let mut faults = Vec::new();
    for (container, called) in calls {
        let named = declared.iter().any(|name| name == container);
        if called && !named {
            faults.push(format!("{container} is written and not declared"));
        }
        if named && !called {
            faults.push(format!("{container} is declared and never written"));
        }
    }
    println!(
        "containers declared: {declared:?}; writers found: {:?}",
        calls
            .iter()
            .filter(|(_, called)| *called)
            .map(|(name, _)| *name)
            .collect::<Vec<&str>>()
    );
    assert!(faults.is_empty(), "{faults:?}");
}

/// What a rule declines without is a claim about the engine, so it is held against the engine
/// rather than against the table it was read from.
///
/// One rule of the nineteen, driven both ways through the built binary: it declines with the
/// name missing, and produces its number with the name supplied. A comparison against
/// `required_options` would be the table checked against itself.
///
/// Parsed, never searched. A run that states the name legitimately carries it in the record of
/// what the rule was bound to, so a substring check for the name reads a successful run as a
/// refusal, which is what the first draft of this did.
#[test]
fn a_name_a_rule_declines_without_is_a_name_the_engine_declines_without() {
    const RULE: &str = "impulse.epoch_from_onset";
    const NAME: &str = "convention";
    const QUANTITY: &str = "epoch_impulse_newton_seconds";

    let requires: Vec<String> = reported()["methods"]
        .as_array()
        .expect("the surface publishes its rules")
        .iter()
        .find(|row| row["id"] == RULE)
        .expect("the rule is published")["requires"]
        .as_array()
        .expect("what it declines without")
        .iter()
        .map(|name| name.as_str().expect("a name").to_string())
        .collect();
    assert_eq!(requires, [NAME]);

    let analysed = |extra: &[&str]| -> serde_json::Value {
        let derive = format!("epoch_impulse={RULE}");
        let mut line: Vec<&str> = vec![
            "--registry",
            "../../registry",
            "--format",
            "json",
            "analyse",
            "../plateforce-conformance/fixtures/subject01_trial1.force.txt",
            "--column",
            "0",
            "--sample-rate-hz",
            "1200",
            "--sentinel",
            "none",
            "--delimiter",
            "\t",
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
            "--set",
            "takeoff.threshold_n=20.0",
            "--derive",
            &derive,
        ];
        line.extend_from_slice(extra);
        let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
            .args(&line)
            .env("NO_COLOR", "1")
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("the built binary runs");
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        serde_json::from_str::<serde_json::Value>(&text)
            .unwrap_or_else(|_| panic!("the run produced a document:\n{text}"))["ok"]
            .clone()
    };

    let declined_over = |document: &serde_json::Value| -> Vec<String> {
        document["refusals"]
            .as_array()
            .expect("a run names what declined")
            .iter()
            .filter(|refusal| refusal["method_id"] == RULE)
            .map(|refusal| refusal["parameter"].as_str().unwrap_or("").to_string())
            .collect()
    };
    let reported = |document: &serde_json::Value| -> bool {
        document["metrics"]
            .as_array()
            .expect("a run reports its quantities")
            .iter()
            .any(|metric| metric["key"] == QUANTITY && !metric["value"].is_null())
    };

    let silent = analysed(&[]);
    assert_eq!(
        declined_over(&silent),
        [NAME],
        "the manifest names a requirement the engine does not have"
    );
    assert!(
        !reported(&silent),
        "the rule declined over {NAME} and reported {QUANTITY} anyway"
    );

    // The other direction, so the assertion above cannot be satisfied by a rule that declines
    // whatever the caller states.
    let answered = analysed(&["--choose", "epoch_impulse.convention=net"]);
    assert!(
        declined_over(&answered).is_empty(),
        "stating {NAME} left the rule declining: {:?}",
        declined_over(&answered)
    );
    assert!(
        reported(&answered),
        "stating {NAME} cleared the refusal and produced no {QUANTITY}"
    );
}
