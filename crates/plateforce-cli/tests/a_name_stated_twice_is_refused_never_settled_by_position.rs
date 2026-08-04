//! A caller who writes one name twice is told, rather than given the second value in silence.
//!
//! The terminal carries six assignment-bearing flags. Three of them cannot repeat, and clap
//! refuses a second `--weighing`, `--onset` or `--takeoff` with "cannot be used multiple
//! times". The other three may repeat, because a run states many values, so the parser cannot
//! tell a second value for one name from a first value for another. A parser that keeps the
//! last and drops the first records nothing: `--set weighing.duration=1.0 --set
//! weighing.duration=2.0` moves system weight from 587.1863 N to 586.5328 N with zero refusals
//! and zero warnings, and produces a document byte-identical to the run of a caller who had
//! only ever written `2.0`. The record cannot then tell a reader which of the caller's two
//! instructions the number rested on.
//!
//! Both halves are asserted here. Dropping the refusal turns the first half red. Refusing a
//! name written once, or refusing two different names under one slot, turns the second half
//! red, which is what stops the fix from being a parser that refuses whenever it sees a flag
//! twice at all.

use std::process::{Command, Output};

const FIXTURE: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn plateforce(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

/// A complete analysis, to which a case adds the lines it is about. Every rule is named, so
/// nothing here rests on a default that could move underneath the test.
fn analysing(extra: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = [
        "--registry",
        "../../registry",
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
        "--onset",
        "onset.threshold.noise_relative",
        "--set",
        "onset.k=5.0",
        "--takeoff",
        "takeoff.threshold.absolute_force",
        "--set",
        "takeoff.threshold_n=20",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect();
    line.extend(extra.iter().map(|word| word.to_string()));
    line
}

fn run(extra: &[&str]) -> Output {
    let arguments = analysing(extra);
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    plateforce(&borrowed)
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Every repeatable flag, with a name written twice and the two values it was written with.
/// One table rather than three tests, so a flag added to the terminal without this treatment
/// is a row somebody has to leave out on purpose.
const STATED_TWICE: &[(&str, &str, &str, &str)] = &[
    ("--set", "weighing.duration", "1.0", "2.0"),
    ("--choose", "onset.direction", "below_only", "two_sided"),
    (
        "--derive",
        "jump_height.takeoff_frame",
        "jumpheight.takeoff.impulse_momentum",
        "jumpheight.takeoff.flight_time",
    ),
];

#[test]
fn a_name_written_twice_is_refused_and_the_refusal_names_both_values() {
    for (flag, name, first, second) in STATED_TWICE {
        let output = run(&[
            flag,
            &format!("{name}={first}"),
            flag,
            &format!("{name}={second}"),
        ]);
        let said = stderr_of(&output);
        println!("{flag} {name}: {}", said.trim());

        assert!(
            !output.status.success(),
            "{flag} {name} written twice was accepted:\n{said}"
        );
        // Both values, because a refusal naming only the one it kept tells the caller nothing
        // about which of their two lines was dropped.
        for value in [first, second] {
            assert!(
                said.contains(value),
                "{flag} {name} was refused without naming '{value}':\n{said}"
            );
        }
        assert!(
            said.contains(name),
            "{flag} refused without naming {name}:\n{said}"
        );
    }
}

/// The control, and it is the half that stops this from becoming a flag counter.
///
/// Each flag is used twice in one line, against two different names. That is the ordinary way
/// a run is written and it has to keep working, so a parser refusing on repetition alone
/// fails here while the real fix passes.
#[test]
fn two_names_under_one_flag_are_not_a_name_written_twice() {
    let output = run(&[
        "--set",
        "weighing.duration=1.0",
        "--set",
        "onset.offset_ms=30",
        "--choose",
        "weighing.centre=mean",
        "--choose",
        "onset.direction=below_only",
    ]);
    let said = stderr_of(&output);
    println!("control: exit {:?}", output.status.code());
    assert!(
        output.status.success(),
        "a run writing two names under each flag was refused:\n{said}"
    );
}

/// The same name under two different slots is two names, and this is the case the per-slot
/// keying exists for. `weighing.centre` and a derived rule's `centre` are different questions
/// asked of different rules, and collapsing them would hand one rule the other's answer.
#[test]
fn one_name_under_two_slots_is_two_names() {
    let output = run(&[
        "--derive",
        "jump_height.takeoff_frame=jumpheight.takeoff.flight_time",
        "--set",
        "weighing.duration=1.0",
        "--set",
        "jump_height.takeoff_frame.gravity=9.81",
    ]);
    let said = stderr_of(&output);
    println!("two slots: exit {:?}", output.status.code());
    assert!(
        output.status.success(),
        "one name written against two different slots was refused:\n{said}"
    );
}
