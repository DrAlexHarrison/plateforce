//! A folder run that binds a rule for something computed from the landmarks.
//!
//! Four of the thirteen constructs computed from the landmarks reach a folder run under the
//! rule the spine picks. Without the flag below the other nine reach it under no rule at all,
//! so the run a squad does cannot ask for a phase model, a braking start, a propulsion
//! boundary, peak force or an analysis window, and cannot name an alternative to the four it
//! gets. This is the flag that asks, and the guards that the answers reach the reader.

use std::collections::BTreeMap;
use std::process::Output;

/// What a run over this folder exits with when a requested number could not be produced.
///
/// Most of these recordings end while the athlete is still off the plate, so the flight-time
/// height declines by name on five of the seven trials. A check that accepted every exit code
/// would pass on a build that cannot run at all.
const A_FOLDER_RUN_THAT_WROTE_ITS_TABLES: i32 = 0;
const THE_REQUEST_COULD_NOT_BE_READ: i32 = 64;

fn batch(out_dir: &std::path::Path, extra: &[&str]) -> Output {
    let named = out_dir.display().to_string();
    let mut arguments: Vec<&str> = vec![
        "--registry",
        "../../registry",
        "batch",
        "../plateforce-conformance/fixtures",
        "--out-dir",
        &named,
        "--trial-suffix",
        ".force.txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--weighing",
        "bwepoch.fixed_window",
        "--onset",
        "onset.threshold.noise_relative",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "weighing.duration=1.0",
        "--set",
        "onset.k=5",
        "--set",
        "takeoff.threshold_n=20",
    ];
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// The same binding asked of one trial, so the two commands can be held to one answer.
fn analyse(extra: &[&str]) -> Output {
    let mut arguments: Vec<&str> = vec![
        "--registry",
        "../../registry",
        "analyse",
        "../plateforce-conformance/fixtures/subject01_trial1.force.txt",
        "--column",
        "0",
        "--sample-rate-hz",
        "1200",
        "--sentinel",
        "none",
        "--weighing",
        "bwepoch.fixed_window",
        "--onset",
        "onset.threshold.noise_relative",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "weighing.duration=1.0",
        "--set",
        "onset.k=5",
        "--set",
        "takeoff.threshold_n=20",
    ];
    arguments.extend(extra.iter().copied());
    std::process::Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(&arguments)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("plateforce-derive-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch directory");
    path
}

/// One relation as a header and rows of named cells.
fn table(out_dir: &std::path::Path, file: &str) -> (Vec<String>, Vec<BTreeMap<String, String>>) {
    let text = std::fs::read_to_string(out_dir.join(file))
        .unwrap_or_else(|_| panic!("{file} is written beside the record"));
    let mut lines = text.lines();
    let header: Vec<String> = split_row(lines.next().expect("a header"));
    let rows = lines
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            header
                .iter()
                .cloned()
                .zip(split_row(line))
                .collect::<BTreeMap<String, String>>()
        })
        .collect();
    (header, rows)
}

/// A message carries commas, so a field opened with a quote runs to its closing quote.
fn split_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    for character in line.chars() {
        match character {
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            other => field.push(other),
        }
    }
    fields.push(field);
    fields
}

/// A rule nothing would otherwise choose is chosen, runs on every trial in the folder, and
/// its number and its chain both reach the reader.
///
/// Two rules rather than one, because the second reads what the first placed. A flag that
/// bound one rule per run would answer peak force with a refusal naming the window.
#[test]
fn a_rule_computed_from_the_landmarks_reaches_the_table_and_the_record() {
    let out = scratch("bound");
    let output = batch(
        &out,
        &[
            "--derive",
            "analysis_window=window_end.takeoff.detected",
            "--derive",
            "net_peak_force=force.peak.net",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(A_FOLDER_RUN_THAT_WROTE_ITS_TABLES),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let (header, rows) = table(&out, "results.csv");
    for column in [
        "analysis_window_start_seconds",
        "analysis_window_end_seconds",
        "net_peak_force_newtons",
    ] {
        assert!(header.contains(&column.to_string()), "{header:?}");
        let answered = rows
            .iter()
            .filter(|row| !row[column].is_empty())
            .filter(|row| row[column].parse::<f64>().is_ok_and(f64::is_finite))
            .count();
        println!("{column} answered on {answered} of {} trials", rows.len());
        assert_eq!(answered, rows.len(), "{column} on {} trials", rows.len());
    }

    // A number without its chain names nothing that produced it, so both ids are looked for in
    // the record rather than only in the table.
    let (_, chain) = table(&out, "provenance.csv");
    let recorded: std::collections::BTreeSet<&str> = chain
        .iter()
        .map(|row| row["method_id"].as_str())
        .collect::<std::collections::BTreeSet<&str>>();
    for id in ["window_end.takeoff.detected", "force.peak.net"] {
        assert!(recorded.contains(id), "{id} is missing from {recorded:?}");
    }
}

/// A value stated against a construct computed from the landmarks reaches the rule under the
/// same word that bound it, and the record says the operator stated it.
///
/// `--set` read against the three landmark steps alone refuses a name qualified by a derived
/// construct as a step this run does not have.
#[test]
fn a_value_stated_against_a_derived_construct_is_recorded_as_stated() {
    let out = scratch("stated");
    let output = batch(
        &out,
        &[
            "--derive",
            "analysis_window=window_end.takeoff.detected",
            "--derive",
            "peak_force=force.peak.estimator",
            "--set",
            "peak_force.averaging_window_seconds=0.05",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(A_FOLDER_RUN_THAT_WROTE_ITS_TABLES),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let (_, chain) = table(&out, "provenance.csv");
    let stated: Vec<&BTreeMap<String, String>> = chain
        .iter()
        .filter(|row| row["method_id"] == "force.peak.estimator")
        .filter(|row| row["parameter"] == "averaging_window_seconds")
        .collect();
    assert!(!stated.is_empty(), "the rule recorded what it read");
    println!(
        "{} rows for averaging_window_seconds, sources {:?}",
        stated.len(),
        stated
            .iter()
            .map(|row| row["source"].as_str())
            .collect::<std::collections::BTreeSet<&str>>()
    );
    for row in &stated {
        assert_eq!(row["source"], "stated", "{row:?}");
        assert_eq!(row["value"], "0.05", "{row:?}");
    }
}

/// A rule the caller bound that declined on every trial leaves the column blank rather than
/// leaving it out, and a refusal row per trial names the rule and what it was waiting for.
///
/// The column is what makes this different from the seven unattributable blanks audited on
/// this folder. A reader who scripts against `results.csv` meets an empty cell they can trace
/// through `refusals.csv`, rather than a missing key and a run that answered a question they
/// did not ask.
#[test]
fn a_rule_that_declined_on_every_trial_leaves_a_blank_column_and_a_refusal_beside_it() {
    let out = scratch("declined");
    // Peak force without the window it reads, which is the decline this build produces on
    // every trial for a reason outside the rule itself.
    let output = batch(&out, &["--derive", "net_peak_force=force.peak.net"]);
    assert_eq!(
        output.status.code(),
        Some(A_FOLDER_RUN_THAT_WROTE_ITS_TABLES),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let (header, rows) = table(&out, "results.csv");
    assert!(
        header.contains(&"net_peak_force_newtons".to_string()),
        "the column the caller asked for is in {header:?}"
    );
    let blank = rows
        .iter()
        .filter(|row| row["net_peak_force_newtons"].is_empty())
        .count();
    println!("{blank} of {} cells blank", rows.len());
    assert_eq!(blank, rows.len(), "the rule declined on every trial");

    // Every blank cell is accounted for, under the id of the rule that produced no number.
    let (_, refusals) = table(&out, "refusals.csv");
    let named: std::collections::BTreeSet<&str> = refusals
        .iter()
        .filter(|row| row["method_id"] == "force.peak.net")
        .map(|row| row["trial_id"].as_str())
        .collect();
    let blanked: std::collections::BTreeSet<&str> = rows
        .iter()
        .filter(|row| row["net_peak_force_newtons"].is_empty())
        .map(|row| row["trial_id"].as_str())
        .collect();
    assert_eq!(named, blanked, "a blank cell a reader cannot account for");
    // And the refusal says what it was waiting for, rather than only that it declined.
    let waiting: Vec<&str> = refusals
        .iter()
        .filter(|row| row["method_id"] == "force.peak.net")
        .map(|row| row["available"].as_str())
        .collect();
    assert!(
        waiting
            .iter()
            .all(|named| named.contains("analysis_window")),
        "{waiting:?}"
    );
}

/// A construct this build runs no rule for is refused before a file is opened, with the ones
/// it does run listed. A folder multiplies one unmakeable binding by its own size.
#[test]
fn a_construct_this_build_runs_no_rule_for_is_refused_before_a_trial_is_read() {
    let out = scratch("nosuch");
    let output = batch(&out, &["--derive", "not_a_construct=anything"]);
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(THE_REQUEST_COULD_NOT_BE_READ));
    assert!(said.contains("phase_model"), "{said}");
    assert!(
        !out.join("results.csv").exists(),
        "no trial was read before the refusal"
    );
}

/// An id that is a rule, named for a construct it is not filed under. Checking that the id
/// exists somewhere would bind an onset rule to peak force and cite its author for it.
#[test]
fn an_id_filed_under_another_construct_is_refused_with_the_ones_filed_under_this_one() {
    let out = scratch("wronghome");
    let output = batch(
        &out,
        &["--derive", "peak_force=onset.threshold.absolute_force"],
    );
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(THE_REQUEST_COULD_NOT_BE_READ));
    assert!(said.contains("force.peak."), "{said}");
    assert!(
        !said.contains("onset.threshold.noise_relative"),
        "the alternatives are the ones filed under the construct that was named: {said}"
    );
    assert!(
        !out.join("results.csv").exists(),
        "no trial was read before the refusal"
    );
}

/// The shape of the line is a fault in the line and carries no published code, the same
/// answer `--set` gives, so a reader meets one grammar across the flags that take a pair.
#[test]
fn an_assignment_carrying_no_equals_is_refused_as_a_line_rather_than_as_a_rule() {
    let out = scratch("malformed");
    let output = batch(&out, &["--derive", "peak_force"]);
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(THE_REQUEST_COULD_NOT_BE_READ));
    assert!(said.contains("--derive takes"), "{said}");
    assert!(
        !out.join("results.csv").exists(),
        "no trial was read before the refusal"
    );
}

/// A construct written twice is refused, exactly as `--set` and `--choose` refuse on the same
/// command line.
///
/// `clap` refuses a repeated `--onset` with `the argument '--onset <METHOD>' cannot be used
/// multiple times`, so a `--derive` that kept the last value would accept one shape of line
/// that the flag beside it refuses.
///
/// The sentence is asserted against the one `--choose` produces on the same run rather than
/// against a copy of the wording, so the two cannot drift into refusing differently for the
/// same shape of line.
#[test]
fn a_construct_written_twice_is_refused_the_way_the_other_repeatable_flags_refuse() {
    let out = scratch("twice");
    let output = batch(
        &out,
        &[
            "--derive",
            "analysis_window=window_end.takeoff.detected",
            "--derive",
            "analysis_window=window_end.fixed_duration.isometric",
        ],
    );
    let said = String::from_utf8(output.stderr).expect("the refusal is UTF-8");
    println!("{}", said.lines().next().unwrap_or_default());
    assert_eq!(output.status.code(), Some(THE_REQUEST_COULD_NOT_BE_READ));
    assert!(
        !out.join("results.csv").exists(),
        "no trial was read before the refusal"
    );

    let chosen = scratch("twice-choose");
    let beside = batch(
        &chosen,
        &[
            "--choose",
            "onset.selection=first",
            "--choose",
            "onset.selection=last",
        ],
    );
    let other = String::from_utf8(beside.stderr).expect("the refusal is UTF-8");
    assert_eq!(beside.status.code(), Some(THE_REQUEST_COULD_NOT_BE_READ));

    // The same sentence with the flag and the names swapped in. Compared by the shape the two
    // share rather than by a literal, which is what makes this catch a second wording.
    let shape = |line: &str| {
        line.split_once(" was given ")
            .map(|(_, rest)| format!(" was given {rest}"))
            .unwrap_or_else(|| panic!("no shared shape in {line}"))
    };
    let mine = shape(said.lines().next().unwrap_or_default());
    let theirs = shape(other.lines().next().unwrap_or_default());
    println!("--derive {mine}\n--choose {theirs}");
    assert!(mine.ends_with(", and a name takes one value"), "{mine}");
    assert!(theirs.ends_with(", and a name takes one value"), "{theirs}");
    assert!(
        said.contains("--derive analysis_window") && other.contains("--choose onset.selection"),
        "each names its own flag and its own name"
    );
}

/// The folder run and the single trial answer `--derive` with one sentence and one status.
///
/// The predicate behind both is `plateforce_batch::derive`, which is the one home for whether
/// this build runs a rule for a construct. Each command kept its own reading of that question
/// until now, and a construct added to one would have been answered by the other as absent
/// until it was edited too. Nothing in the types holds the two together, so the property is a
/// single-site fact rather than a compile-time one, and this is what says so out loud.
///
/// Both halves of the split are checked, because they arrive differently: a line the reader
/// rewrites from the grammar carries no published code, and a name they rewrite from a list
/// carries one. A guard over either alone would pass on a surface that had re-grown its own
/// copy of the other.
#[test]
fn the_folder_and_the_single_trial_refuse_a_derived_binding_with_one_sentence() {
    for (name, line) in [
        ("malformed", "peak_force"),
        ("unknown", "not_a_construct=anything"),
    ] {
        let out = scratch(&format!("one-answer-{name}"));
        let folder = batch(&out, &["--derive", line]);
        let alone = analyse(&["--derive", line]);

        let said = |output: &Output| {
            String::from_utf8(output.stderr.clone())
                .expect("the refusal is UTF-8")
                .lines()
                .next()
                .unwrap_or_default()
                .to_string()
        };
        let (folder_said, alone_said) = (said(&folder), said(&alone));
        println!("--derive {line}\n  folder: {folder_said}\n  trial:  {alone_said}");

        assert_eq!(folder.status.code(), Some(THE_REQUEST_COULD_NOT_BE_READ));
        assert_eq!(alone.status.code(), folder.status.code());
        assert!(!folder_said.is_empty(), "the folder run said something");
        assert_eq!(
            alone_said, folder_said,
            "one question answered two ways for '--derive {line}'"
        );
    }
}
