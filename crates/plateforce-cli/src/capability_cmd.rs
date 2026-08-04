//! What this surface can do, reported by executing this surface.
//!
//! Every surface answers the same question about itself and the answers are compared against
//! one committed file. The operations are read off this binary's own command tree, so a
//! command that goes away shortens the array and the comparison fails. A manifest generated
//! once inside the shared crate would agree with itself whatever any surface could actually
//! do, and would have caught nothing.

use clap::ValueEnum;
use plateforce_analysis::capability::{
    capability, AcquisitionIntake, Capability, Operation, OutputFormat,
};

use crate::exit::{Fault, Outcome};
use crate::out::Format;

#[derive(Debug, clap::Args)]
pub struct Args {}

/// What each command this binary offers can be asked to do, keyed by the name a reader types.
///
/// `serve` maps to nothing: serving the browser's own bundle is a transport rather than a
/// computation, and the manifest asserts over what every surface owes.
fn operations_named(command: &str) -> Option<&'static [Operation]> {
    match command {
        "analyse" => Some(&[
            Operation::Analyse,
            // Performed on the way to a result rather than under a name of its own, declared
            // here beside the command that exercises it.
            Operation::ParseForceFile,
            // The spread rides in every result rather than behind a flag, so the surface
            // does it on every run and the manifest says so.
            Operation::Spread,
        ]),
        // Both entry points, because batch has two: one loops the analysis and returns
        // results, the other loops the sweep and returns paired variants. A surface reaching
        // only the first would claim one operation where the software has two.
        "batch" => Some(&[Operation::Batch, Operation::Compare]),
        "capability" => Some(&[Operation::Capability]),
        "reach" => Some(&[Operation::Reach]),
        "registry" => Some(&[
            Operation::RegistryCensus,
            Operation::RegistryShow,
            Operation::RegistryValidate,
        ]),
        // Recording a plate's settings maps to nothing, for the reason `serve` does: it
        // writes and reads a file on this machine and computes nothing over a trace. What the
        // settings then do to a result is `analyse` and `batch`, which are already here.
        "plate" => Some(&[]),
        // The sweep under a name of its own, for a quantity other than the one `analyse`
        // reports without being asked.
        "spread" => Some(&[Operation::Spread]),
        "serve" => Some(&[]),
        "version" => Some(&[Operation::Version]),
        "help" => Some(&[]),
        _ => None,
    }
}

/// Read off clap's own command tree rather than from a list kept beside it.
pub fn commands_offered() -> Vec<String> {
    crate::command_tree()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect()
}

pub fn every_operation() -> Vec<Operation> {
    commands_offered()
        .iter()
        .filter_map(|name| operations_named(name))
        .flat_map(|operations| operations.iter().copied())
        .collect()
}

/// What this binary writes a result into, taken from what it writes rather than from one
/// flag.
///
/// `--format` answers for the rendered document alone. A folder run writes its tables
/// whatever that flag says, so a manifest built from the flag reports fewer containers than
/// the surface produces, which is the direction a comparison against a committed file cannot
/// see. `tests/capability.rs` holds this against the writer calls themselves.
pub fn every_output_format() -> Vec<OutputFormat> {
    let mut written: Vec<OutputFormat> = Format::value_variants()
        .iter()
        .map(|format| match format {
            Format::Text => OutputFormat::Text,
            Format::Json => OutputFormat::Json,
        })
        .collect();
    written.push(OutputFormat::Csv);
    written
}

/// The flag a command carries to be told what the plate and its settings were.
const ACQUISITION_FLAG: &str = "acquisition";

/// Whether a caller of this binary can state the acquisition block, read off clap's own
/// command tree.
///
/// A declaration kept beside the flag would go on saying yes after the flag went away, and
/// the surface that lost the flag is exactly the one whose manifest has to say so. Nested,
/// because the flag rides on the commands that analyse rather than on the binary.
pub fn commands_taking_the_acquisition_block() -> Vec<String> {
    crate::command_tree()
        .get_subcommands()
        .filter(|command| {
            command
                .get_arguments()
                .any(|argument| argument.get_id() == ACQUISITION_FLAG)
        })
        .map(|command| command.get_name().to_string())
        .collect()
}

pub fn manifest() -> Capability {
    let intake = if commands_taking_the_acquisition_block().is_empty() {
        AcquisitionIntake::AbsentFromThisSurface
    } else {
        AcquisitionIntake::StatedByCaller
    };
    capability(&every_operation(), &every_output_format(), intake)
}

pub fn run(_args: &Args, _format: Format) -> Outcome {
    // A manifest read by a person is the same document read by a diff, so `--format text`
    // renders what `--format json` renders rather than a second thing to keep true.
    match serde_json::to_value(manifest()) {
        Ok(value) => Outcome::complete(crate::registry_cmd::canonical(&value)),
        Err(error) => Outcome::declined_line(Fault::Internal, format!("{error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command added without a decision about the manifest would otherwise report nothing
    /// and the gate would pass while the surfaces diverged.
    #[test]
    fn every_command_this_binary_offers_says_what_it_can_do() {
        let offered = commands_offered();
        let unmapped: Vec<&String> = offered
            .iter()
            .filter(|name| operations_named(name).is_none())
            .collect();
        println!(
            "commands offered {}, mapped to operations {} of {}",
            offered.len(),
            offered.len() - unmapped.len(),
            offered.len()
        );
        assert!(unmapped.is_empty(), "unmapped: {unmapped:?}");
    }

    /// The manifest's answer is derived from the tree, and a walk that found no flag at all
    /// would report the same `false` a binary without the flag reports.
    #[test]
    fn the_commands_that_take_the_acquisition_block_are_named_rather_than_counted() {
        let taking = commands_taking_the_acquisition_block();
        println!(
            "commands taking the acquisition block: {} of {} offered: {taking:?}",
            taking.len(),
            commands_offered().len()
        );
        assert!(
            taking.contains(&"analyse".to_string()),
            "the walk found {taking:?}, so its verdict is about the walk rather than the tree"
        );
        assert!(taking.contains(&"batch".to_string()), "{taking:?}");
        assert!(manifest().acquisition.stated_by_caller);
    }
}
