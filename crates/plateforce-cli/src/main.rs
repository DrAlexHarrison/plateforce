//! The plateforce command line.
//!
//! Output is ASCII by default and colour is opt-in through the terminal's own
//! signals. Windows ConHost does not enable ANSI unless a registry value says so, and
//! a scientific tool that prints escape codes into a log file has failed at its job.

use std::process::ExitCode;

use plateforce_registry::{Citation, CitationRole, Method, Protocol, Provenance, Registry};

const USAGE: &str = "\
plateforce - force-plate analysis with a method registry

USAGE:
    plateforce <COMMAND> [OPTIONS]

COMMANDS:
    registry census      count the registry, per population, with denominators
    registry validate    load the registry and report every rule violation
    registry show <ID>   print one method or protocol entry in full
    version              print the version

OPTIONS:
    --registry <DIR>     path to the registry directory (default: ./registry),
                         written either as two words or as --registry=<DIR>
";

const DEFAULT_REGISTRY_DIRECTORY: &str = "registry";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (registry_directory, words) = match split_off_registry_directory(&arguments) {
        Ok(split) => split,
        Err(message) => {
            eprintln!("plateforce: {message}");
            return ExitCode::FAILURE;
        }
    };

    match words.as_slice() {
        [] if arguments.is_empty() => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        [] => {
            eprintln!("plateforce: needs a command");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
        ["-h"] | ["--help"] | ["help"] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["version"] | ["--version"] | ["-V"] => {
            println!("plateforce {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["registry", rest @ ..] => registry_command(rest, registry_directory),
        other => {
            eprintln!("plateforce: unknown command: {}", other.join(" "));
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// Takes `--registry <DIR>` and `--registry=<DIR>` out of the line wherever either appears,
/// so a reader who writes the flag before the entry id means what a reader who writes it
/// after means.
///
/// A flag whose value went missing must not resolve itself to the default, quietly, in the
/// tool that exists to document what silent defaults cost. Two of them naming two
/// directories must not resolve to whichever came last either.
fn split_off_registry_directory(arguments: &[String]) -> Result<(&str, Vec<&str>), String> {
    let mut directory: Option<&str> = None;
    let mut words: Vec<&str> = Vec::new();
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index].as_str();
        let joined = argument.strip_prefix("--registry=");
        if argument != "--registry" && joined.is_none() {
            words.push(argument);
            index += 1;
            continue;
        }
        if directory.is_some() {
            return Err("--registry was given more than once".to_string());
        }
        match joined {
            Some(value) if !value.is_empty() => {
                directory = Some(value);
                index += 1;
            }
            Some(_) => return Err("--registry needs the path to a registry directory".to_string()),
            None => {
                match arguments.get(index + 1) {
                    Some(value) if !value.starts_with('-') => directory = Some(value),
                    _ => {
                        return Err("--registry needs the path to a registry directory".to_string())
                    }
                }
                index += 2;
            }
        }
    }

    Ok((directory.unwrap_or(DEFAULT_REGISTRY_DIRECTORY), words))
}

fn registry_command(words: &[&str], directory: &str) -> ExitCode {
    let registry = match Registry::load(directory) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("plateforce: {error}");
            return ExitCode::FAILURE;
        }
    };

    match words {
        ["validate"] => {
            let census = registry.census();
            println!(
                "registry at {directory} is valid: {} computation entries, {} protocol entries, {} constructs",
                census.computation_entries, census.protocol_entries, census.constructs
            );
            ExitCode::SUCCESS
        }
        ["census"] => {
            let census = registry.census();
            let computation = census.computation_entries;
            // Populations are reported apart and never summed. Both derived counts are taken
            // over the computation entries and say so, because indentation under the wrong
            // line is how a count loses its denominator. Both of this project's headline
            // counts were assertions until somebody recounted them.
            println!("{:<36}{}", "constructs", census.constructs);
            println!("{:<36}{computation}", "computation entries");
            println!(
                "{:<36}{} of {computation}",
                "  of which genuine debates",
                registry.genuine_debates().count()
            );
            println!(
                "{:<36}{} of {computation}",
                "  of which can find the wrong event",
                registry.methods_that_can_fail().count()
            );
            println!("{:<36}{}", "protocol entries", census.protocol_entries);
            ExitCode::SUCCESS
        }
        ["show", id] => show_entry(&registry, id),
        ["show"] => {
            eprintln!("plateforce: registry show needs an entry id");
            ExitCode::FAILURE
        }
        // A word the command has no use for is dropped by a tool that reads only as far as
        // it needs to, and a second entry id looks exactly like that.
        ["show", _, extra @ ..] => {
            eprintln!(
                "plateforce: registry show takes one entry id, so {} is left over",
                extra.join(" ")
            );
            ExitCode::FAILURE
        }
        [command @ ("validate" | "census"), extra @ ..] => {
            eprintln!(
                "plateforce: registry {command} takes no entry id, so {} is left over",
                extra.join(" ")
            );
            ExitCode::FAILURE
        }
        [] => {
            eprintln!("plateforce: registry needs a command");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
        [command, ..] => {
            eprintln!("plateforce: unknown registry command: {command}");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// The registry holds two populations, and an id belongs to one of them: the validator
/// refuses a registry where the same id appears in both.
fn show_entry(registry: &Registry, id: &str) -> ExitCode {
    if let Some(method) = registry.methods.get(id) {
        show_method(method);
        return ExitCode::SUCCESS;
    }
    if let Some(protocol) = registry.protocols.get(id) {
        show_protocol(protocol);
        return ExitCode::SUCCESS;
    }
    eprintln!("plateforce: no entry with id {id}");
    ExitCode::FAILURE
}

/// TOML floats carry a decimal point and `f64`'s Display drops it on a whole number, so
/// `20` on screen would not match the `20.0` in the file a reader goes on to search.
fn join_numbers(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn provenance_as_registry_spells_it(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::Published => "published",
        Provenance::ObservedFromCode => "observed_from_code",
        Provenance::VendorDocumented => "vendor_documented",
    }
}

fn citation_role_as_registry_spells_it(role: CitationRole) -> &'static str {
    match role {
        CitationRole::Proposes => "proposes",
        CitationRole::Uses => "uses",
        CitationRole::Evaluates => "evaluates",
        CitationRole::Disputes => "disputes",
    }
}

fn show_citations(citations: &[Citation]) {
    for citation in citations {
        let doi = citation
            .doi
            .as_ref()
            .map(|doi| format!(", doi {doi}"))
            .unwrap_or_default();
        // An unobtained source bars an entry from `recommended`, so it travels with the row.
        let obtained = if citation.obtained {
            ""
        } else {
            ", source not obtained"
        };
        println!(
            "  citation    {} ({}) {}{doi}{obtained}",
            citation.key,
            citation_role_as_registry_spells_it(citation.role),
            citation.reference,
        );
    }
}

fn show_method(method: &Method) {
    println!("{}", method.id);
    println!("  title       {}", method.title);
    println!("  construct   {}", method.construct);
    println!("  status      {}", method.status);
    println!("  confidence  {}", method.confidence);
    println!("  rule        {}", method.rule.trim());

    for parameter in &method.parameters {
        let published = if parameter.published_values.is_empty() {
            String::new()
        } else {
            format!("  published: {}", join_numbers(&parameter.published_values))
        };
        println!(
            "  parameter   {}{}{}",
            parameter.name,
            parameter
                .default
                .map(|value| format!(" = {value:?}"))
                .unwrap_or_default(),
            published
        );
    }
    for bias in &method.biases {
        println!(
            "  bias        {} {} against {}{}",
            bias.magnitude,
            bias.unit,
            bias.criterion,
            if bias.conditional_on_success {
                ", conditional on the rule not failing"
            } else {
                ""
            }
        );
    }
    if let Some(failure) = &method.failure {
        println!(
            "  FAILS       {} of {} trials ({:.1}%), {}, on {}",
            failure.numerator,
            failure.denominator,
            failure.rate * 100.0,
            failure.detectability,
            failure.corpus
        );
        println!("              {}", failure.definition);
    }
    show_citations(&method.citations);
}

/// A protocol entry carries no rule, no parameters and no bias, so a method-shaped block
/// with those lines blank would report absent fields as empty ones.
fn show_protocol(protocol: &Protocol) {
    println!("{}", protocol.id);
    println!("  title       {}", protocol.title);
    println!("  area        {}", protocol.area);
    println!(
        "  provenance  {}",
        provenance_as_registry_spells_it(protocol.provenance)
    );
    println!("  description {}", protocol.description.trim());
    for affected in &protocol.affects {
        println!("  affects     {affected}");
    }
    show_citations(&protocol.citations);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(line: &[&str]) -> Result<(String, Vec<String>), String> {
        let arguments: Vec<String> = line.iter().map(|word| word.to_string()).collect();
        split_off_registry_directory(&arguments).map(|(directory, words)| {
            (
                directory.to_string(),
                words.into_iter().map(str::to_string).collect(),
            )
        })
    }

    /// Where the flag sits is the difference between an entry id and a directory path
    /// being read as the same word, which is the confusing failure this ordering removes.
    #[test]
    fn the_flag_means_the_same_thing_wherever_it_sits() {
        let expected = Ok((
            "elsewhere".to_string(),
            vec![
                "registry".to_string(),
                "show".to_string(),
                "an.id".to_string(),
            ],
        ));
        assert_eq!(
            split(&["registry", "show", "an.id", "--registry", "elsewhere"]),
            expected
        );
        assert_eq!(
            split(&["registry", "show", "--registry", "elsewhere", "an.id"]),
            expected
        );
        assert_eq!(
            split(&["registry", "--registry", "elsewhere", "show", "an.id"]),
            expected
        );
        assert_eq!(
            split(&["--registry", "elsewhere", "registry", "show", "an.id"]),
            expected
        );
    }

    /// The joined spelling is what a user reaching for a long flag tends to type, and
    /// reading it as a command name would answer a path with "unknown command".
    #[test]
    fn the_joined_spelling_names_the_same_directory() {
        let expected = Ok((
            "elsewhere".to_string(),
            vec![
                "registry".to_string(),
                "show".to_string(),
                "an.id".to_string(),
            ],
        ));
        assert_eq!(
            split(&["registry", "show", "an.id", "--registry=elsewhere"]),
            expected
        );
        assert_eq!(
            split(&["--registry=elsewhere", "registry", "show", "an.id"]),
            expected
        );
        assert!(split(&["registry", "census", "--registry="]).is_err());
        assert!(split(&[
            "--registry=here",
            "registry",
            "census",
            "--registry",
            "there"
        ])
        .is_err());
    }

    #[test]
    fn no_flag_reads_the_default_directory() {
        let (directory, words) = split(&["registry", "census"]).unwrap();
        assert_eq!(directory, DEFAULT_REGISTRY_DIRECTORY);
        assert_eq!(words, vec!["registry".to_string(), "census".to_string()]);
    }

    #[test]
    fn a_flag_with_no_value_is_refused_rather_than_resolved_to_the_default() {
        assert!(split(&["registry", "show", "an.id", "--registry"]).is_err());
        assert!(split(&["registry", "show", "--registry", "--verbose"]).is_err());
        assert!(split(&["--registry"]).is_err());
    }

    /// Two directories on one line is a question, and answering it with whichever came
    /// last is the silent choice this tool exists to make visible.
    #[test]
    fn two_registries_on_one_line_are_refused() {
        assert!(split(&[
            "--registry",
            "here",
            "registry",
            "census",
            "--registry",
            "there"
        ])
        .is_err());
        assert!(split(&["--registry", "here", "--registry", "here"]).is_err());
    }
}
