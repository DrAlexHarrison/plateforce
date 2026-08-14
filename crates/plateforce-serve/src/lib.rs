//! `plateforce serve`: the browser interface, from this binary, to this machine only.
//!
//! A locked-down enterprise Linux box has no `webkit2gtk-4.1`, so no desktop artefact runs
//! on it, and an air-gapped one cannot open the interface from a file because browsers
//! refuse to instantiate WebAssembly from a `file://` URL. Both have a browser.

use std::process::ExitCode;

mod assets;
mod content_types;
mod http;

pub use assets::{asset_for, assets, carries_the_browser_bundle, not_part_of_the_interface, Asset};
pub use content_types::content_type_for;
pub use http::{listen, serve};

// The statuses this path returns, and their one home. `crates/plateforce-cli/src/exit.rs`
// reads them rather than restating them: that crate depends on this one and the dependency
// cannot run both ways, so a number written in both places would be free to disagree in one.
// This crate carries no dependencies at all, which is what puts the numbers on this side.
pub const A_REQUEST_THAT_CANNOT_BE_HONOURED: u8 = 64;
pub const A_PORT_ANOTHER_PROGRAM_HOLDS: u8 = 69;
pub const AN_INVARIANT_THIS_SOFTWARE_BREAKS: u8 = 70;
pub const A_PORT_THIS_PROCESS_MAY_NOT_HAVE: u8 = 77;

const WHAT_SERVE_TAKES: &str = "\
plateforce serve - serve the browser interface to this machine only

USAGE:
    plateforce serve [OPTIONS]

OPTIONS:
    --port <PORT>    the port to listen on, or --port=<PORT>. Left out, the
                     operating system picks a free one and prints the address
    --open           also open the printed address in a browser
";

struct Options {
    port: u16,
    port_was_stated: bool,
    open_a_browser: bool,
}

pub fn run(arguments: &[&str]) -> ExitCode {
    // Answered before anything else, so asking what the options are works on a machine where
    // the port is taken and on a build that carries no interface.
    if arguments
        .iter()
        .any(|word| *word == "--help" || *word == "-h")
    {
        print!("{WHAT_SERVE_TAKES}");
        return ExitCode::SUCCESS;
    }

    let options = match parse_options(arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("plateforce: {message}");
            eprint!("{WHAT_SERVE_TAKES}");
            return ExitCode::from(A_REQUEST_THAT_CANNOT_BE_HONOURED);
        }
    };

    // A page whose module is absent loads to a blank window and a console line nobody is
    // watching.
    if !carries_the_browser_bundle() {
        eprintln!("plateforce: this program carries no browser interface to serve");
        eprintln!("run scripts/build-web.sh release, then build plateforce again");
        return ExitCode::from(AN_INVARIANT_THIS_SOFTWARE_BREAKS);
    }

    let listener = match listen(options.port) {
        Ok(listener) => listener,
        // A port another program holds, and a port this process may not have, are facts about
        // the machine rather than invariants this software broke. On a shared laboratory
        // computer they are the likeliest way this command ends, and both are one flag from
        // working, so neither sends the operator to a maintainer.
        Err(error) => return declined_the_port(options.port, options.port_was_stated, &error),
    };
    let address = match listener.local_addr() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("plateforce: the port could not be read back: {error}");
            return ExitCode::FAILURE;
        }
    };

    let url = format!("http://{address}");
    // The first line is the URL and nothing else, so a script can read it.
    println!("{url}");
    println!("Serving to this machine only. Press Ctrl-C to stop.");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    if options.open_a_browser {
        open_a_browser_at(&url);
    }

    serve(listener);
    ExitCode::SUCCESS
}

/// Both spellings of the flag. A value that went missing, and two ports on one line, are
/// refused rather than resolved to the default or to whichever came last, matching what
/// `--registry` already does one crate over.
fn parse_options(arguments: &[&str]) -> Result<Options, String> {
    let mut port: Option<u16> = None;
    let mut open_a_browser = false;
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index];
        let joined = argument.strip_prefix("--port=");
        if argument == "--open" {
            open_a_browser = true;
            index += 1;
            continue;
        }
        if argument != "--port" && joined.is_none() {
            return Err(format!("serve has no option {argument}"));
        }
        if port.is_some() {
            return Err("--port was given more than once".to_string());
        }
        let value = match joined {
            Some(value) => {
                index += 1;
                value
            }
            None => {
                let value = arguments
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or("--port needs a port number")?;
                index += 2;
                value
            }
        };
        port = Some(
            value
                .parse::<u16>()
                .map_err(|_| format!("--port needs a port number, not {value}"))?,
        );
    }

    Ok(Options {
        // Port 0 asks the operating system for a free one, which is then printed. A fixed
        // default would collide with whatever else on the machine had claimed it.
        port: port.unwrap_or(0),
        port_was_stated: port.is_some(),
        open_a_browser,
    })
}

/// What the operator is told when the port did not open, and the status a script reads.
///
/// The next act is named because there is one: every case here is answered by `--port` or by
/// leaving it out, and a status of its own lets a workflow retry a busy port without retrying
/// a forbidden one.
fn declined_the_port(port: u16, port_was_stated: bool, error: &std::io::Error) -> ExitCode {
    match error.kind() {
        std::io::ErrorKind::AddrInUse => {
            eprintln!("plateforce serve: port {port} is already in use.");
            eprintln!(
                "Choose another with --port, or leave it out and take whichever port is free."
            );
            ExitCode::from(A_PORT_ANOTHER_PROGRAM_HOLDS)
        }
        std::io::ErrorKind::PermissionDenied => {
            let (reason, next) = permission_denied_message(port, port_was_stated);
            eprintln!("{reason}");
            eprintln!("{next}");
            ExitCode::from(A_PORT_THIS_PROCESS_MAY_NOT_HAVE)
        }
        _ => {
            eprintln!("plateforce serve: cannot listen on port {port}: {error}");
            ExitCode::from(AN_INVARIANT_THIS_SOFTWARE_BREAKS)
        }
    }
}

fn permission_denied_message(port: u16, port_was_stated: bool) -> (String, String) {
    if port_was_stated {
        return (
            format!("plateforce serve: this account is not permitted to open port {port}."),
            "Choose another port from 1024 to 65535 with --port, or leave --port out to let the operating system choose."
                .to_string(),
        );
    }
    (
        "plateforce serve: this account was not permitted to open an automatically selected port."
            .to_string(),
        "Run plateforce serve --port 8000. If that port is unavailable, replace 8000 with another number from 1024 to 65535."
            .to_string(),
    )
}

/// Opt-in, because opening a browser is the kind of thing a tool does on somebody's behalf
/// without recording it, and on a headless or air-gapped box the opener may not exist. A
/// missing opener is reported and the server keeps running.
fn open_a_browser_at(url: &str) {
    let (program, leading) = if cfg!(target_os = "macos") {
        ("open", Vec::new())
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/c", "start", ""])
    } else {
        ("xdg-open", Vec::new())
    };

    let mut command = std::process::Command::new(program);
    command.args(leading).arg(url);
    if let Err(error) = command.status() {
        eprintln!("plateforce: {program} could not open a browser: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(line: &[&str]) -> Result<(u16, bool), String> {
        parse_options(line).map(|parsed| (parsed.port, parsed.open_a_browser))
    }

    #[test]
    fn no_options_asks_the_operating_system_for_a_port_and_opens_nothing() {
        assert_eq!(options(&[]), Ok((0, false)));
        assert!(!parse_options(&[]).unwrap().port_was_stated);
        assert!(parse_options(&["--port", "8000"]).unwrap().port_was_stated);
    }

    #[test]
    fn the_joined_spelling_names_the_same_port() {
        assert_eq!(options(&["--port", "8000"]), Ok((8000, false)));
        assert_eq!(options(&["--port=8000"]), Ok((8000, false)));
        assert_eq!(options(&["--open", "--port=8000"]), Ok((8000, true)));
        assert_eq!(options(&["--port", "8000", "--open"]), Ok((8000, true)));
    }

    #[test]
    fn a_port_that_went_missing_is_refused_rather_than_resolved_to_the_default() {
        assert!(options(&["--port"]).is_err());
        assert!(options(&["--port", "--open"]).is_err());
        assert!(options(&["--port="]).is_err());
        assert!(options(&["--port", "notanumber"]).is_err());
        assert!(options(&["--port", "70000"]).is_err());
    }

    #[test]
    fn two_ports_on_one_line_are_refused() {
        assert!(options(&["--port", "8000", "--port", "8001"]).is_err());
        assert!(options(&["--port=8000", "--port=8000"]).is_err());
    }

    #[test]
    fn an_option_this_command_does_not_have_is_named_rather_than_dropped() {
        let refusal = options(&["--verbose"]).unwrap_err();
        assert!(refusal.contains("--verbose"), "{refusal}");
    }

    /// A mistyped option and a port somebody else is already using are different faults, and
    /// the codes are the ones the command line declares for the whole binary.
    #[test]
    fn a_usage_error_and_a_runtime_failure_carry_different_codes() {
        assert_eq!(A_REQUEST_THAT_CANNOT_BE_HONOURED, 64);
        assert_eq!(AN_INVARIANT_THIS_SOFTWARE_BREAKS, 70);
        assert_ne!(
            A_REQUEST_THAT_CANNOT_BE_HONOURED,
            AN_INVARIANT_THIS_SOFTWARE_BREAKS
        );
    }

    /// What the server takes, not what the command line around it takes.
    #[test]
    fn the_usage_names_the_options_this_command_reads() {
        for option in ["--port", "--open"] {
            assert!(WHAT_SERVE_TAKES.contains(option), "{option} is unnamed");
        }
        for elsewhere in ["--registry", "--format", "--out", "--color"] {
            assert!(
                !WHAT_SERVE_TAKES.contains(elsewhere),
                "{elsewhere} is not this command's to offer"
            );
        }
    }

    #[test]
    fn a_denied_automatic_port_names_a_choice_the_reader_has_not_already_made() {
        let automatic = permission_denied_message(0, false);
        let automatic = format!("{}\n{}", automatic.0, automatic.1);
        println!("{automatic}");
        assert!(automatic.contains("automatically selected port"));
        assert!(automatic.contains("plateforce serve --port 8000"));
        for misleading in ["port 0", "below 1024", "leave it out"] {
            assert!(!automatic.contains(misleading), "{automatic}");
        }

        let explicit = permission_denied_message(80, true);
        assert!(explicit.0.contains("port 80"));
        assert_ne!(automatic, format!("{}\n{}", explicit.0, explicit.1));
    }
}
