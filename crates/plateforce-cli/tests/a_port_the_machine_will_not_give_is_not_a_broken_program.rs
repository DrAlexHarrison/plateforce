//! A port another program holds is reported as the machine being busy, not as this software
//! breaking its own invariant.
//!
//! `serve` exists for the locked-down laboratory computer, which is exactly the machine where
//! something else already holds the port. Both cases here are one flag from working, so the
//! operator is told which flag rather than being sent to whoever maintains this.
//!
//! The statuses are `sysexits` values and a workflow reads them without parsing a sentence: a
//! busy port is worth retrying and a forbidden one never is, so they do not share a number.

use std::net::TcpListener;
use std::process::{Command, Output};

/// EX_UNAVAILABLE. The request was answerable and the machine was not free to answer it.
const A_PORT_ANOTHER_PROGRAM_HOLDS: i32 = 69;

/// EX_NOPERM. The operating system declined, which no rewording of the request changes.
const A_PORT_THIS_PROCESS_MAY_NOT_HAVE: i32 = 77;

/// EX_SOFTWARE, which is what both of the above used to report.
const AN_INVARIANT_THIS_SOFTWARE_BREAKS: i32 = 70;

fn serving(port: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["serve", "--port", port])
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs")
}

fn refusal(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// The port is held for the length of the run by a listener this test owns, so the case is
/// created rather than waited for.
#[test]
fn a_port_another_program_holds_says_so_and_names_the_flag() {
    let held = TcpListener::bind("127.0.0.1:0").expect("a port to hold");
    let port = held.local_addr().expect("the port reads back").port();

    let output = serving(&port.to_string());

    assert_eq!(
        output.status.code(),
        Some(A_PORT_ANOTHER_PROGRAM_HOLDS),
        "{}",
        refusal(&output)
    );
    assert_ne!(
        output.status.code(),
        Some(AN_INVARIANT_THIS_SOFTWARE_BREAKS),
        "another program holding a port is not this software breaking"
    );
    assert_eq!(
        refusal(&output),
        format!(
            "plateforce serve: port {port} is already in use. Choose another with --port, or \
             leave it out and take whichever port is free."
        )
    );
    drop(held);
}

/// A stated port is named with the range the account can choose from, so the reader can change
/// the request without having to know what a privileged port is.
#[test]
fn a_port_this_process_may_not_open_is_told_apart_from_one_that_is_busy() {
    let output = serving("80");

    // A machine that grants low ports to any process cannot exercise this, and saying so beats
    // a green assertion that never ran.
    if output.status.code() == Some(0) || refusal(&output).contains("already in use") {
        println!("this machine grants port 80, so the permission case did not arise");
        return;
    }

    assert_eq!(
        output.status.code(),
        Some(A_PORT_THIS_PROCESS_MAY_NOT_HAVE),
        "{}",
        refusal(&output)
    );
    assert_eq!(
        refusal(&output),
        "plateforce serve: this account is not permitted to open port 80. Choose another port \
         from 1024 to 65535 with --port, or leave --port out to let the operating system choose."
    );
}

/// The two statuses are what a workflow branches on, so a build that gave them one number
/// would be a changed contract rather than a changed sentence.
#[test]
fn the_two_ways_a_port_fails_do_not_share_a_status() {
    assert_ne!(
        A_PORT_ANOTHER_PROGRAM_HOLDS,
        A_PORT_THIS_PROCESS_MAY_NOT_HAVE
    );
    assert_ne!(
        A_PORT_ANOTHER_PROGRAM_HOLDS,
        AN_INVARIANT_THIS_SOFTWARE_BREAKS
    );
    assert_ne!(
        A_PORT_THIS_PROCESS_MAY_NOT_HAVE,
        AN_INVARIANT_THIS_SOFTWARE_BREAKS
    );
}
