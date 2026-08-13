//! The plateforce command line.
//!
//! Output is ASCII by default and colour is opt-in through the terminal's own signals.
//! Windows ConHost does not enable ANSI unless a registry value says so.

mod acquisition_arg;
mod analyse;
mod batch;
mod capability_cmd;
mod decisions;
mod examples;
mod exit;
mod manual;
mod methods_cmd;
mod out;
mod plate_cmd;
mod plate_source;
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

use exit::{code_for, stream_for, Outcome};
use out::Format;
use render::{Colour, Renderer};

/// What `--help` says above the options, where `-h` shows the line alone.
///
/// The second paragraph is the reason the flags below take the shape they do, and a reader who
/// skips it meets the same fact as a refusal on their first run instead.
///
/// Wrapped here rather than by clap, which lays a long description out as it was written. The
/// width is the one `render.rs` falls back to when there is no terminal to ask.
const WHAT_THIS_IS: &str = "\
Force-plate analysis where every number carries the rule that produced it.

Published methods for one jump metric disagree by more than a training effect
does, so which rule produced a number decides the number. Every value here
travels with its rule, the values that rule was given, and where each of those
came from.

The rules are data rather than code, so the names the method flags take move when
the registry moves. `plateforce methods` prints them, under the words that reach them.";

/// What `capability --help` says, where the one-line about is what `-h` shows.
///
/// The second paragraph states the shape a caller gets rather than leaving them to find it:
/// this command answers in JSON whichever format was asked for, because the document is
/// compared byte for byte across surfaces and two renderings of it would be two documents.
const WHAT_CAPABILITY_IS: &str = "\
Report every operation, rule, value and refusal code, as one JSON document.

This is the call to make before writing anything against this software, because it describes
the copy in front of you rather than one somebody wrote about. It carries every operation
this surface dispatches, every rule it runs with the slot each fills, every value each rule
takes with the exact text that states it, the acquisition block's members, the containers
this surface writes, and every refusal code with its exit code.

It answers in JSON whichever format is asked for, so the same document reaches a reader and
a diff.

To read the rules rather than parse them, `plateforce methods` prints every rule under the
words that reach it, and `plateforce registry show <METHOD>` prints one in full.";

#[derive(Parser)]
#[command(
    name = "plateforce",
    version,
    about = "Force-plate analysis where every number carries the rule that produced it",
    long_about = WHAT_THIS_IS,
    after_help = examples::TOP_SHORT,
    after_long_help = examples::TOP_LONG,
    disable_help_subcommand = false
)]
struct Invocation {
    /// Read the registry in this directory rather than the one compiled in
    #[arg(long, global = true, action = clap::ArgAction::Append, value_name = "DIR")]
    registry: Vec<PathBuf>,
    /// Keep saved plates in this directory rather than beside this machine's other settings
    #[arg(long, global = true, action = clap::ArgAction::Append, value_name = "DIR")]
    plates: Vec<PathBuf>,
    /// Write the result as readable text, as JSON, or as Markdown to paste into a chat
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
    #[command(
        after_help = examples::ANALYSE_SHORT,
        after_long_help = examples::ANALYSE_LONG
    )]
    Analyse(analyse::Args),
    /// Run every trial in a folder under one request
    #[command(after_help = examples::BATCH_SHORT, after_long_help = examples::BATCH_LONG)]
    Batch(batch::Args),
    /// Report every operation, rule, value and refusal code, as one JSON document
    #[command(
        long_about = WHAT_CAPABILITY_IS,
        after_help = examples::CAPABILITY_SHORT,
        after_long_help = examples::CAPABILITY_LONG
    )]
    Capability(capability_cmd::Args),
    /// Write the completion script a shell reads to finish these commands and their values
    #[command(
        after_help = examples::COMPLETIONS_SHORT,
        after_long_help = examples::COMPLETIONS_LONG
    )]
    Completions(manual::CompletionsArgs),
    /// Write one manual page per command, where this machine's `man` reads them
    #[command(after_help = examples::MAN_SHORT, after_long_help = examples::MAN_LONG)]
    Man(manual::ManArgs),
    /// Name every rule, under the words that reach them
    #[command(after_help = examples::METHODS_SHORT, after_long_help = examples::METHODS_LONG)]
    Methods(methods_cmd::Args),
    /// Record a plate's settings once, and read back the ones this machine holds
    #[command(
        subcommand,
        after_help = examples::PLATE_SHORT,
        after_long_help = examples::PLATE_LONG
    )]
    Plate(plate_cmd::Command),
    /// Report what this registry reaches, per construct, and what stands in the way of the rest
    #[command(after_help = examples::REACH_SHORT, after_long_help = examples::REACH_LONG)]
    Reach,
    /// Read the registry
    #[command(
        subcommand,
        after_help = examples::REGISTRY_SHORT,
        after_long_help = examples::REGISTRY_LONG
    )]
    Registry(registry_cmd::Command),
    /// Sweep a quantity over every rule on each step of its path
    #[command(after_help = examples::SPREAD_SHORT, after_long_help = examples::SPREAD_LONG)]
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
    #[command(after_help = examples::VERSION_SHORT)]
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

    let registry_directory = match one_directory(
        "--registry",
        &invocation.registry,
        times_written(std::env::args_os(), "--registry"),
        "an entry read under one of them would carry the other's id",
    ) {
        Ok(directory) => directory,
        Err(message) => {
            return report_parse_failure(
                Invocation::command().error(ErrorKind::ArgumentConflict, message),
            )
        }
    };

    let plates_directory = match one_directory(
        "--plates",
        &invocation.plates,
        times_written(std::env::args_os(), "--plates"),
        "a plate saved under one of them would be absent from the other",
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
            plates_directory.as_deref(),
            invocation.format,
            &renderer,
        ),
        Command::Plate(command) => {
            plate_cmd::run(command, plates_directory.as_deref(), invocation.format)
        }
        Command::Reach => reach::run(registry_directory.as_deref(), invocation.format, &renderer),
        Command::Methods(args) => methods_cmd::run(
            args,
            registry_directory.as_deref(),
            invocation.format,
            &renderer,
        ),
        // Both write a document for a program that is not this one, `man` and a shell, so
        // neither reads the registry and neither takes a result's containers.
        Command::Man(args) => manual::write_manual(args, invocation.format),
        Command::Completions(args) => manual::write_completions(args, invocation.format),
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
                    plates_directory.as_deref(),
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
        Command::Capability(args) => {
            capability_cmd::run(args, invocation.format, registry_directory.as_deref())
        }
        Command::Version => version_cmd::run(invocation.format),
    };

    deliver(
        outcome,
        invocation.out.as_deref(),
        invocation.color,
        invocation.format,
    )
}

/// Whether the line asked for JSON, read off the raw arguments.
///
/// A parse that failed produced no `--format` to read, and the caller who wrote it is exactly
/// the caller who most needs a machine-readable answer: a program that mistypes one flag would
/// otherwise get prose where every other decline is an object. Read from `args_os` because that
/// is all there is at this point, and matched on the pair rather than on the word alone so a
/// path named `json` does not turn a human's run into a document.
fn asked_for_json() -> bool {
    let written: Vec<String> = std::env::args_os()
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    written
        .windows(2)
        .any(|pair| (pair[0] == "--format" && pair[1] == "json") || pair[0] == "--format=json")
        || written.iter().any(|word| word == "--format=json")
}

/// clap's own `Error::exit` prints to stderr and terminates with 2 for a usage error, and
/// two exit codes for one class of fault is the split this crate exists to close. Nothing
/// here ever exits 2.
///
/// Argument parsing used to be a second refusal channel: it named the offending token, carried
/// no code, and shared its exit status with `decision_not_made`, so a program branching on the
/// status alone could not tell "there is no such operation" from "a decision on your path is
/// still open". It answers in the same vocabulary as everything else now, under
/// `command_line_not_parsed`, and clap's own sentence is carried through as the message because
/// it names the token and often the nearest thing this build does offer.
fn report_parse_failure(error: clap::Error) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        // A bare invocation is a reader asking what this program does, and an answer goes to
        // the stream an answer goes to: clap sends this kind to stderr, where `plateforce |
        // less` and `plateforce > what-is-this.txt` both lose it.
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand | ErrorKind::MissingSubcommand
            if std::env::args_os().len() <= 1 =>
        {
            let _ = Invocation::command().print_help();
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let refusal = plateforce_core::Refusal::command_line_not_parsed(
        error
            .render()
            .to_string()
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
            .collect::<Vec<&str>>()
            .join(" "),
    );

    if asked_for_json() {
        // The envelope every other refusal is written in, on the stream `stream_for` sends a
        // refusal carrying no document to, so one reader handles both channels the same way.
        // Written to stdout it would have been the only refusal in the program that was.
        match serde_json::to_string(&serde_json::json!({ "refusal": refusal })) {
            Ok(document) => eprintln!("{document}"),
            Err(_) => {
                let _ = error.print();
            }
        }
    } else {
        let _ = error.print();
    }
    ExitCode::from(refusal.exit_code() as u8)
}

/// A flag whose value went missing does not resolve itself to the default, and two of them
/// naming two directories do not resolve to whichever came last.
///
/// The occurrences are counted off the command line rather than read back from the parse. A
/// global argument is propagated to the subcommand it precedes, and both levels then hold the
/// last value alone, so a line naming two directories parses as one.
///
/// Naming none reads the registry this build carries, rather than a relative `registry`
/// directory that resolves differently depending on where the operator is standing.
fn one_directory(
    flag: &str,
    parsed: &[PathBuf],
    written: usize,
    consequence: &str,
) -> Result<Option<PathBuf>, String> {
    if written > 1 {
        return Err(format!(
            "{flag} names {written} directories, and {consequence}"
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
    // redirected the result to a file still learns why a number is absent. A caller who asked
    // for JSON gets the record, in the envelope every surface returns.
    if outcome.document.is_none() {
        for refusal in &outcome.refusals {
            match format {
                Format::Json => {
                    eprintln!("{}", registry_cmd::canonical_refusal(&refusal.record()))
                }
                // A refusal is not a result, so it reaches a reader as the sentence rather
                // than as a Markdown block with nothing in it.
                Format::Text | Format::Markdown => {
                    eprintln!("plateforce: {}", refusal.terminal())
                }
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
        one_directory(
            "--registry",
            &invocation.registry,
            written,
            "an entry read under one of them would carry the other's id",
        )
    }

    fn plates_of(line: &[&str]) -> Result<Option<PathBuf>, String> {
        let invocation = parse(line).map_err(|error| error.to_string())?;
        let written = times_written(line.iter().map(std::ffi::OsString::from), "--plates");
        one_directory(
            "--plates",
            &invocation.plates,
            written,
            "a plate saved under one of them would be absent from the other",
        )
    }

    /// The flag that names where saved plates live is held to what `--registry` is held to,
    /// because the failure is the same one: two folders on one line is a question, and
    /// answering it with whichever came last is the silent choice this tool refuses.
    #[test]
    fn two_plate_folders_on_one_line_are_refused() {
        assert_eq!(
            plates_of(&["--plates", "here", "plate", "list"]).unwrap(),
            Some(PathBuf::from("here"))
        );
        assert!(plates_of(&["--plates", "here", "plate", "list", "--plates", "there"]).is_err());
        assert!(plates_of(&["plate", "list", "--plates"]).is_err());
        assert_eq!(plates_of(&["plate", "list"]).unwrap(), None);
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

    /// Naming no directory names no directory. Resolving to the relative path `registry`
    /// would read a different set of methods depending on where the operator stood, and
    /// report a different digest without saying why.
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
