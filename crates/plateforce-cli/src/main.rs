//! The plateforce command line.
//!
//! Output is ASCII by default and colour is opt-in through the terminal's own
//! signals. Windows ConHost does not enable ANSI unless a registry value says so, and
//! a scientific tool that prints escape codes into a log file has failed at its job.

use std::process::ExitCode;

const USAGE: &str = "\
plateforce - force-plate analysis with a method registry

USAGE:
    plateforce <COMMAND> [OPTIONS]

COMMANDS:
    registry census      count the registry, per population, with denominators
    registry validate    load the registry and report every rule violation
    registry show <ID>   print one entry, its citations, bias and known failures
    version              print the version

OPTIONS:
    --registry <DIR>     path to the registry directory (default: ./registry)
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();

    match words.as_slice() {
        [] | ["-h"] | ["--help"] | ["help"] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["version"] | ["--version"] | ["-V"] => {
            println!("plateforce {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["registry", rest @ ..] => match registry_path(&arguments) {
            Ok(path) => registry_command(rest, &path),
            Err(message) => {
                eprintln!("plateforce: {message}");
                ExitCode::FAILURE
            }
        },
        other => {
            eprintln!("plateforce: unknown command: {}", other.join(" "));
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// A flag whose value went missing must not resolve itself to the default, quietly, in
/// the tool that exists to document what silent defaults cost.
fn registry_path(arguments: &[String]) -> Result<String, String> {
    let Some(flag) = arguments.iter().position(|argument| argument == "--registry") else {
        return Ok("registry".to_string());
    };
    match arguments.get(flag + 1) {
        Some(path) if !path.starts_with('-') => Ok(path.clone()),
        _ => Err("--registry needs the path to a registry directory".to_string()),
    }
}

fn registry_command(words: &[&str], path: &str) -> ExitCode {
    let loaded = plateforce_registry::Registry::load(path);

    match (words.first(), loaded) {
        (Some(&"validate"), Ok(registry)) => {
            let census = registry.census();
            println!(
                "registry at {path} is valid: {} computation entries, {} protocol entries, {} constructs",
                census.computation_entries, census.protocol_entries, census.constructs
            );
            ExitCode::SUCCESS
        }
        (Some(&"census"), Ok(registry)) => {
            let census = registry.census();
            // Populations are reported apart and never summed. Both of this project's
            // headline counts were assertions until somebody recounted them.
            println!("constructs                      {}", census.constructs);
            println!("computation entries             {}", census.computation_entries);
            println!("protocol entries                {}", census.protocol_entries);
            println!(
                "  of which genuine debates      {}",
                registry.genuine_debates().count()
            );
            println!(
                "  of which can find the wrong event  {}",
                registry.methods_that_can_fail().count()
            );
            ExitCode::SUCCESS
        }
        (Some(&"show"), Ok(registry)) => match words.get(1) {
            None => {
                eprintln!("plateforce: registry show needs an entry id");
                ExitCode::FAILURE
            }
            Some(id) => match registry.methods.get(*id) {
                Some(method) => {
                    show_method(method);
                    ExitCode::SUCCESS
                }
                None => {
                    eprintln!("plateforce: no entry with id {id}");
                    ExitCode::FAILURE
                }
            },
        },
        (_, Err(error)) => {
            eprintln!("plateforce: {error}");
            ExitCode::FAILURE
        }
        (None, Ok(_)) => {
            eprintln!("plateforce: registry needs a command");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
        (Some(command), Ok(_)) => {
            eprintln!("plateforce: unknown registry command: {command}");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn join_numbers(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn show_method(method: &plateforce_registry::Method) {
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
                .map(|d| format!(" = {d}"))
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
}
