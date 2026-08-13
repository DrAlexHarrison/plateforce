//! The rules this build runs, under the words a caller writes to reach them.
//!
//! Every flag that takes a rule takes a name out of data, so the set moves when the registry
//! moves and no help string can carry it. What the help can carry is the one command that
//! prints it, and this is that command. The forced-decision refusal and the refusal that
//! meets an unknown id both list candidates already; a reader who has not made a mistake yet
//! reaches the same list from here.
//!
//! Read off `BINDINGS` rather than off the registry's own populations, because the two answer
//! different questions. The registry holds every rule the literature publishes; this build
//! runs a subset, and a listing of the wider set would put names in front of a reader that
//! their line cannot use.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use plateforce_analysis::binding::{conditioning_constructs, Binding};
use plateforce_analysis::{BINDINGS, ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT, WEIGHING_CONSTRUCT};
use plateforce_core::Refusal;
use plateforce_registry::Registry;
use serde_json::json;

use crate::decisions::label_of;
use crate::exit::{Declined, Fault, Outcome};
use crate::out::Format;
use crate::registry_cmd::canonical;
use crate::render::{Renderer, Role};

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Narrow to one step, which is the word `--set` and `--place` write. Absent names every
    /// step
    #[arg(long, value_name = "SLOT")]
    pub slot: Option<String>,
}

/// Where a rule id ends and its title starts, when the ids are short enough to share a line.
const WIDEST_ID_THAT_SHARES_A_LINE: usize = 44;

/// How a caller reaches one rule: the exact text they write, minus the rule's own name.
///
/// Three shapes rather than one, because the three landmark rules have flags of their own and
/// everything else is reached by construct through `--derive` or `--condition`. The engine
/// refuses a construct named through the wrong one of those two, so a listing that printed a
/// single shape would send half its readers to a refusal.
fn written_as(binding: &Binding, conditioning: &[&str]) -> String {
    match binding.construct {
        WEIGHING_CONSTRUCT | ONSET_CONSTRUCT | TAKEOFF_CONSTRUCT => {
            format!("--{} <METHOD>", binding.slot)
        }
        construct if conditioning.contains(&construct) => {
            format!("--condition {construct}=<METHOD>")
        }
        construct => format!("--derive {construct}=<METHOD>"),
    }
}

/// One heading's worth of rules: the words that reach them, and what the field calls the thing
/// they compute.
struct Step {
    construct: &'static str,
    slot: &'static str,
    written_as: String,
    label: String,
    rules: Vec<&'static Binding>,
}

/// The steps in the order a trace passes through them, which is the order `BINDINGS` declares
/// and the order a reader building a command line meets them in. Sorting alphabetically would
/// put the rule that conditions the signal after the rule that reads what it produced.
fn steps(registry: &Registry) -> Vec<Step> {
    let conditioning = conditioning_constructs();
    let mut order: Vec<&'static str> = Vec::new();
    let mut collected: BTreeMap<&'static str, Vec<&'static Binding>> = BTreeMap::new();
    for binding in BINDINGS {
        if !order.contains(&binding.construct) {
            order.push(binding.construct);
        }
        collected
            .entry(binding.construct)
            .or_default()
            .push(binding);
    }

    order
        .into_iter()
        .map(|construct| {
            let rules = collected.remove(construct).unwrap_or_default();
            let first = rules.first().expect("a construct reached here from a rule");
            Step {
                construct,
                slot: first.slot,
                written_as: written_as(first, &conditioning),
                label: label_of(registry, construct),
                rules,
            }
        })
        .collect()
}

/// The steps a caller reaches by a flag of their own, read off what each heading says rather
/// than by comparing the slot word with the construct: `takeoff` spells both the same way, so
/// that comparison finds two of the three landmarks and reads as though it found them all.
fn landmark_slots(steps: &[Step]) -> Vec<&str> {
    steps
        .iter()
        .filter(|step| !step.written_as.contains('='))
        .map(|step| step.slot)
        .collect()
}

pub fn run(
    args: &Args,
    registry_directory: Option<&Path>,
    format: Format,
    renderer: &Renderer,
) -> Outcome {
    let registry = match crate::registry_source::load(registry_directory) {
        Ok(registry) => registry,
        Err(error) => {
            return Outcome::declined(Declined::recorded(Refusal::registry_invalid(format!(
                "{error}"
            ))))
        }
    };

    let every = steps(&registry);
    let shown: Vec<&Step> = match &args.slot {
        None => every.iter().collect(),
        Some(named) => every
            .iter()
            .filter(|step| step.slot == named || step.construct == named)
            .collect(),
    };

    // A filter matching nothing is answered by name rather than with an empty document. The
    // three landmark steps are named because they are the ones a reader reaches for and there
    // are three of them; the rest are a listing rather than a sentence, so the refusal names
    // the command that prints it.
    if shown.is_empty() {
        let named = args.slot.as_deref().unwrap_or_default();
        return Outcome::declined_line(
            Fault::Request,
            format!(
                "--slot {named} matches no step. The steps with a flag of their own are {}, and \
                 `plateforce methods` names every step, including the ones reached by construct",
                landmark_slots(&every).join(", ")
            ),
        );
    }

    // A pipeline is the other thing a caller may name where a rule would go, and the only
    // route to the set was mistyping one. Shown with the whole listing rather than under a
    // step, because a pipeline binds several steps at once and belongs to none of them.
    let pipelines: Vec<(&str, &str)> = match args.slot {
        Some(_) => Vec::new(),
        None => registry
            .presets
            .values()
            .map(|preset| (preset.id.as_str(), preset.title.as_str()))
            .collect(),
    };

    match format {
        Format::Markdown => crate::out::markdown_wants_a_result("methods"),
        Format::Json => Outcome::complete(canonical(&as_data(&shown, &pipelines))),
        Format::Text => Outcome::complete(as_prose(&shown, &pipelines, BINDINGS.len(), renderer)),
    }
}

fn as_data(shown: &[&Step], pipelines: &[(&str, &str)]) -> serde_json::Value {
    json!({
        "rule_count": BINDINGS.len(),
        "pipelines": pipelines
            .iter()
            .map(|(id, title)| json!({ "preset_id": id, "title": title }))
            .collect::<Vec<_>>(),
        "steps": shown
            .iter()
            .map(|step| json!({
                "construct": step.construct,
                "slot": step.slot,
                "label": step.label,
                "written_as": step.written_as,
                "rules": step
                    .rules
                    .iter()
                    .map(|rule| json!({
                        "method_id": rule.id,
                        "title": rule.title,
                        "composed_from": rule.composed_from,
                        "records_under": rule.records_under,
                    }))
                    .collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>(),
    })
}

/// One block per step: the words that reach it, what the field calls it, then a rule to a line.
///
/// The population is stated once at the top, as a fact about the listing, and each step then
/// carries its own count against it. Said on every heading instead it became forty lines
/// describing the software to somebody who came to find out what they may type.
fn as_prose(
    shown: &[&Step],
    pipelines: &[(&str, &str)],
    total: usize,
    renderer: &Renderer,
) -> String {
    let mut document = String::new();
    let _ = writeln!(
        document,
        "{}\n",
        pluralised(total, "rule", "grouped by the flag that reaches each")
    );
    for (position, step) in shown.iter().enumerate() {
        if position > 0 {
            document.push('\n');
        }
        let _ = writeln!(
            document,
            "{}",
            renderer.paint(Role::Heading, &step.written_as)
        );
        for line in renderer.wrap(&step.label, 2) {
            let _ = writeln!(document, "{line}");
        }
        let _ = writeln!(document, "  {}", pluralised(step.rules.len(), "rule", ""));
        for rule in &step.rules {
            for line in rule_lines(rule, renderer) {
                let _ = writeln!(document, "{line}");
            }
        }
    }
    if !pipelines.is_empty() {
        let _ = writeln!(
            document,
            "\n{}",
            renderer.paint(Role::Heading, "--preset <NAME>")
        );
        for line in renderer.wrap(
            "A published pipeline, which binds the rules and the values its source stated",
            2,
        ) {
            let _ = writeln!(document, "{line}");
        }
        let _ = writeln!(
            document,
            "  {}",
            pluralised(pipelines.len(), "pipeline", "")
        );
        for (id, title) in pipelines {
            for line in named_lines(id, title, renderer) {
                let _ = writeln!(document, "{line}");
            }
        }
    }

    // `registry show` is where the rule's own words, its citations and every value it takes
    // live, so the listing hands a reader the next command rather than restating any of it.
    let _ = write!(
        document,
        "\n`plateforce registry show <METHOD>` prints one rule in full, with every value it takes"
    );
    document
}

fn rule_lines(rule: &Binding, renderer: &Renderer) -> Vec<String> {
    named_lines(rule.id, rule.title, renderer)
}

/// A count and its noun, with a trailing clause where there is one. One rule reads as one
/// rule rather than as 1 rules, which is the kind of seam a reader notices before the content.
fn pluralised(count: usize, noun: &str, tail: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    match tail.is_empty() {
        true => format!("{count} {noun}{plural}"),
        false => format!("{count} {noun}{plural}, {tail}"),
    }
}

/// A name a caller writes, and what it is, beside it where the width allows and under it where
/// it does not. An id cut to fit a column resolves nowhere, so a long one keeps its own line.
fn named_lines(id: &str, title: &str, renderer: &Renderer) -> Vec<String> {
    let indent = 4;
    if id.chars().count() > WIDEST_ID_THAT_SHARES_A_LINE {
        let mut lines = vec![format!("{}{id}", " ".repeat(indent))];
        lines.extend(renderer.wrap(title, indent + 2));
        return lines;
    }
    let title_indent = indent + WIDEST_ID_THAT_SHARES_A_LINE + 2;
    let mut wrapped = renderer.wrap(title, title_indent);
    match wrapped.first_mut() {
        Some(first) => {
            first.replace_range(
                ..title_indent - 2,
                &format!("{}{id:<WIDEST_ID_THAT_SHARES_A_LINE$}", " ".repeat(indent)),
            );
            wrapped
        }
        None => vec![format!("{}{id}", " ".repeat(indent))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        crate::registry_source::load(None).expect("this build carries a registry")
    }

    /// Every rule this build runs is reachable from this listing, so a reader who reads it has
    /// met the whole set rather than the part that happens to be grouped under a known word.
    #[test]
    fn every_rule_this_build_runs_appears_once() {
        let steps = steps(&registry());
        let listed: usize = steps.iter().map(|step| step.rules.len()).sum();
        println!(
            "rules listed {} of {} this build runs, under {} steps",
            listed,
            BINDINGS.len(),
            steps.len()
        );
        assert_eq!(listed, BINDINGS.len());
    }

    /// The words on each heading are the words the parser takes. A heading naming a flag that
    /// does not exist sends every reader of that block to `command_line_not_parsed`.
    #[test]
    fn every_heading_names_a_flag_this_binary_offers() {
        let tree = crate::command_tree();
        let analyse = tree
            .get_subcommands()
            .find(|command| command.get_name() == "analyse")
            .expect("analyse is offered");
        let flags: Vec<&str> = analyse
            .get_arguments()
            .filter_map(|argument| argument.get_long())
            .collect();
        for step in steps(&registry()) {
            let written = step
                .written_as
                .trim_start_matches("--")
                .split([' ', '='])
                .next()
                .expect("a heading names a flag")
                .to_string();
            assert!(
                flags.contains(&written.as_str()),
                "{} names --{written}, which analyse does not take, and its flags are {flags:?}",
                step.written_as
            );
        }
    }

    /// The three landmark steps are reached by a flag of their own and everything else by
    /// construct, so a listing that printed one shape would be wrong for one group or the
    /// other. Asserted in both directions: at least one step of each shape.
    #[test]
    fn the_landmark_steps_and_the_derived_steps_are_written_differently() {
        let steps = steps(&registry());
        let landmark = steps
            .iter()
            .filter(|step| !step.written_as.contains('='))
            .count();
        let by_construct = steps
            .iter()
            .filter(|step| step.written_as.contains('='))
            .count();
        println!("steps reached by a flag {landmark}, by construct {by_construct}");
        assert_eq!(
            landmark, 3,
            "weighing, onset and takeoff have their own flag"
        );
        assert!(by_construct > 0);
    }
}
