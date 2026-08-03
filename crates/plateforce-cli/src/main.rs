//! The plateforce command line.
//!
//! Output is ASCII by default and colour is opt-in through the terminal's own signals.
//! Windows ConHost does not enable ANSI unless a registry value says so, and a scientific
//! tool that prints escape codes into a log file has failed at its job.

mod analyse;
mod batch;
mod capability_cmd;
mod decisions;
mod exit;
mod out;
mod preset;
mod reach;
mod registry_cmd;
mod registry_source;
mod render;
mod spread_cmd;
mod verdict;
mod version_cmd;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{error::ErrorKind, CommandFactory, Parser, Subcommand};

use exit::{code_for, stream_for, Fault, Outcome};
use out::Format;
use render::{Colour, Renderer};

#[derive(Parser)]
#[command(
    name = "plateforce",
    version,
    about = "Force-plate analysis with a method registry",
    disable_help_subcommand = false
)]
struct Invocation {
    /// Read the registry in this directory rather than the one compiled in
    #[arg(long, global = true, action = clap::ArgAction::Append, value_name = "DIR")]
    registry: Vec<PathBuf>,
    /// Write the result as readable text or as JSON
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Write the result to this path
    #[arg(long, global = true, value_name = "PATH")]
    out: Option<PathBuf>,
    /// When to colour the output
    #[arg(long, global = true, value_enum, default_value_t = Colour::Auto)]
    color: Colour,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compute every number one trace supports, with the rule behind each
    Analyse(analyse::Args),
    /// Run every trial in a folder under one request
    Batch(batch::Args),
    /// Report what this build can do, for comparison against every other surface
    Capability(capability_cmd::Args),
    /// Report what this registry reaches, per construct, and what stands in the way of the rest
    Reach,
    /// Read the registry
    #[command(subcommand)]
    Registry(registry_cmd::Command),
    /// Sweep a quantity over every rule this build runs for each step on its path
    Spread(spread_cmd::Args),
    /// Serve the browser interface to this machine
    // Help is answered by the server, which owns the options, so this level does not
    // intercept the flag on its way there.
    #[command(disable_help_flag = true)]
    Serve {
        /// Passed to the server: --port <PORT>, --port=<PORT>, --open
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "OPTION"
        )]
        options: Vec<String>,
    },
    /// Print the version
    Version,
}

/// clap's own view of what this binary offers, which is what the parity manifest reports.
pub fn command_tree() -> clap::Command {
    Invocation::command()
}

fn main() -> ExitCode {
    let invocation = match Invocation::try_parse() {
        Ok(invocation) => invocation,
        Err(error) => return report_parse_failure(error),
    };

    let registry_directory = match one_registry_directory(
        &invocation.registry,
        times_written(std::env::args_os(), "--registry"),
    ) {
        Ok(directory) => directory,
        Err(message) => {
            return report_parse_failure(
                Invocation::command().error(ErrorKind::ArgumentConflict, message),
            )
        }
    };

    let renderer = Renderer::for_stdout(invocation.color, invocation.out.is_some());

    let outcome = match &invocation.command {
        Command::Registry(command) => registry_cmd::run(
            command,
            registry_directory.as_deref(),
            invocation.format,
            &renderer,
        ),
        Command::Analyse(args) => analyse::run(
            args,
            registry_directory.as_deref(),
            invocation.format,
            &renderer,
        ),
        Command::Reach => reach::run(registry_directory.as_deref(), invocation.format, &renderer),
        // The server holds the process rather than handing back a document, and it reads its
        // own options, so the one parser for them stays in the crate that acts on them.
        Command::Serve { options } => {
            let borrowed: Vec<&str> = options.iter().map(String::as_str).collect();
            return plateforce_serve::run(&borrowed);
        }
        // A run's result is a set of files, so `--out` names the folder they go in and the
        // summary is not a second document written to the same path.
        Command::Batch(args) => {
            return deliver(
                batch::run(
                    args,
                    registry_directory.as_deref(),
                    invocation.format,
                    invocation.out.as_deref(),
                    &renderer,
                ),
                None,
                invocation.color,
                invocation.format,
            )
        }
        Command::Spread(args) => spread_cmd::run(
            args,
            registry_directory.as_deref(),
            invocation.format,
            &renderer,
        ),
        Command::Capability(args) => capability_cmd::run(args, invocation.format),
        Command::Version => version_cmd::run(invocation.format),
    };

    deliver(
        outcome,
        invocation.out.as_deref(),
        invocation.color,
        invocation.format,
    )
}

/// clap's own `Error::exit` prints to stderr and terminates with 2 for a usage error, and
/// two exit codes for one class of fault is the split this crate exists to close. Nothing
/// here ever exits 2.
fn report_parse_failure(error: clap::Error) -> ExitCode {
    let _ = error.print();
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::SUCCESS,
        // A bare invocation is a reader asking what this program does.
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand | ErrorKind::MissingSubcommand
            if std::env::args_os().len() <= 1 =>
        {
            ExitCode::SUCCESS
        }
        _ => ExitCode::from(Fault::Request.code()),
    }
}

/// A flag whose value went missing must not resolve itself to the default, quietly, in the
/// tool that exists to document what silent defaults cost. Two of them naming two
/// directories must not resolve to whichever came last either.
///
/// The occurrences are counted off the command line rather than read back from the parse.
/// A global argument is propagated to the subcommand it precedes, and both levels then hold
/// the last value alone, so a line naming two directories parses as one and the other
/// disappears.
///
/// Naming none reads the registry this build carries, rather than a relative `registry`
/// directory that resolves differently depending on where the operator is standing.
fn one_registry_directory(parsed: &[PathBuf], written: usize) -> Result<Option<PathBuf>, String> {
    if written > 1 {
        return Err(format!(
            "--registry names {written} directories, and an entry read under one of them would carry the other's id"
        ));
    }
    match parsed {
        [only] => Ok(Some(only.clone())),
        _ => Ok(None),
    }
}

fn times_written(arguments: impl Iterator<Item = std::ffi::OsString>, flag: &str) -> usize {
    let joined = format!("{flag}=");
    arguments
        .filter(|argument| {
            argument
                .to_str()
                .is_some_and(|word| word == flag || word.starts_with(&joined))
        })
        .count()
}

fn deliver(
    outcome: Outcome,
    destination: Option<&std::path::Path>,
    colour: Colour,
    format: Format,
) -> ExitCode {
    let code = code_for(&outcome);
    let stream = stream_for(&outcome);

    if let Some(document) = &outcome.document {
        if let Err((fault, message)) = out::deliver(document, destination, stream, colour) {
            eprintln!("plateforce: {message}");
            return ExitCode::from(fault.code());
        }
    }

    // A refusal with no result to sit beside travels on its own stream, so a reader who
    // redirected the result to a file still learns why a number is absent. A caller who
    // asked for JSON gets the record rather than a sentence to parse back apart, in the
    // envelope every surface returns.
    if outcome.document.is_none() {
        for refusal in &outcome.refusals {
            match format {
                Format::Json => {
                    eprintln!("{}", registry_cmd::canonical_refusal(&refusal.record()))
                }
                Format::Text => eprintln!("plateforce: {}", refusal.terminal()),
            }
        }
    }

    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &[&str]) -> Result<Invocation, clap::Error> {
        let mut arguments = vec!["plateforce"];
        arguments.extend_from_slice(line);
        Invocation::try_parse_from(arguments)
    }

    fn directory_of(line: &[&str]) -> Result<Option<PathBuf>, String> {
        let invocation = parse(line).map_err(|error| error.to_string())?;
        let written = times_written(line.iter().map(std::ffi::OsString::from), "--registry");
        one_registry_directory(&invocation.registry, written)
    }

    /// Where the flag sits is the difference between an entry id and a directory path
    /// being read as the same word, which is the confusing failure this ordering removes.
    #[test]
    fn the_flag_means_the_same_thing_wherever_it_sits() {
        let expected = Some(PathBuf::from("elsewhere"));
        assert_eq!(
            directory_of(&["registry", "show", "an.id", "--registry", "elsewhere"]).unwrap(),
            expected
        );
        assert_eq!(
            directory_of(&["registry", "show", "--registry", "elsewhere", "an.id"]).unwrap(),
            expected
        );
        assert_eq!(
            directory_of(&["registry", "--registry", "elsewhere", "show", "an.id"]).unwrap(),
            expected
        );
        assert_eq!(
            directory_of(&["--registry", "elsewhere", "registry", "show", "an.id"]).unwrap(),
            expected
        );
    }

    /// The joined spelling is what a user reaching for a long flag tends to type, and
    /// reading it as a command name would answer a path with "unknown command".
    #[test]
    fn the_joined_spelling_names_the_same_directory() {
        let expected = Some(PathBuf::from("elsewhere"));
        assert_eq!(
            directory_of(&["registry", "show", "an.id", "--registry=elsewhere"]).unwrap(),
            expected
        );
        assert_eq!(
            directory_of(&["--registry=elsewhere", "registry", "show", "an.id"]).unwrap(),
            expected
        );
        assert!(directory_of(&["registry", "census", "--registry="]).is_err());
        assert!(directory_of(&[
            "--registry=here",
            "registry",
            "census",
            "--registry",
            "there"
        ])
        .is_err());
    }

    /// Naming no directory names no directory. It used to resolve to the relative path
    /// `registry`, so the same command read a different set of methods depending on where
    /// the operator stood, and reported a different digest without saying why.
    #[test]
    fn no_flag_names_no_directory_and_reads_what_this_build_carries() {
        assert_eq!(directory_of(&["registry", "census"]).unwrap(), None);
        assert!(registry_source::load(None).is_ok());
    }

    #[test]
    fn a_flag_with_no_value_is_refused_rather_than_resolved_to_the_default() {
        assert!(directory_of(&["registry", "show", "an.id", "--registry"]).is_err());
        assert!(directory_of(&["registry", "show", "--registry", "--verbose"]).is_err());
        assert!(directory_of(&["--registry"]).is_err());
    }

    /// Two directories on one line is a question, and answering it with whichever came
    /// last is the silent choice this tool exists to make visible.
    #[test]
    fn two_registries_on_one_line_are_refused() {
        assert!(directory_of(&[
            "--registry",
            "here",
            "registry",
            "census",
            "--registry",
            "there"
        ])
        .is_err());
        assert!(directory_of(&["--registry", "here", "--registry", "here"]).is_err());
    }

    #[test]
    fn the_declared_command_surface_parses() {
        Invocation::command().debug_assert();
    }
}
