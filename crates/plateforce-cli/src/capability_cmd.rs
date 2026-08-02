//! What this surface can do, reported by executing this surface.
//!
//! Every surface answers the same question about itself and the answers are compared against
//! one committed file. The operations are read off this binary's own command tree, so a
//! command that goes away shortens the array and the comparison fails. A manifest generated
//! once inside the shared crate would agree with itself whatever any surface could actually
//! do, and would have caught nothing.

use clap::ValueEnum;
use plateforce_analysis::capability::{capability, Capability, Operation, OutputFormat};

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
        ]),
        "capability" => Some(&[Operation::Capability]),
        "registry" => Some(&[
            Operation::RegistryCensus,
            Operation::RegistryShow,
            Operation::RegistryValidate,
        ]),
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

/// What this binary can write a result into, read off `--format` so a container added there
/// appears here without a second edit.
pub fn every_output_format() -> Vec<OutputFormat> {
    Format::value_variants()
        .iter()
        .map(|format| match format {
            Format::Text => OutputFormat::Text,
            Format::Json => OutputFormat::Json,
        })
        .collect()
}

pub fn manifest() -> Capability {
    capability(&every_operation(), &every_output_format())
}

pub fn run(_args: &Args, _format: Format) -> Outcome {
    // A manifest read by a person is the same document read by a diff, so `--format text`
    // renders what `--format json` renders rather than a second thing to keep true.
    match serde_json::to_value(manifest()) {
        Ok(value) => Outcome::complete(crate::registry_cmd::canonical(&value)),
        Err(error) => Outcome::declined(Fault::Internal, format!("{error}")),
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
}
