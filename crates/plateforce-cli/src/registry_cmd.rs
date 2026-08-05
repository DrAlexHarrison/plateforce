//! Reading the registry: what it holds, whether it loads, and one entry in full.

use std::fmt::Write as _;
use std::path::Path;

use plateforce_core::Refusal;
use plateforce_registry::{
    Census, Citation, CitationRole, Method, Protocol, Provenance, Registry, Status,
};
use serde_json::json;

use crate::exit::{Declined, Fault, Outcome};
use crate::out::Format;
use crate::render::{Renderer, Role};
use crate::verdict;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Count the registry, per population, with denominators
    Census,
    /// Load the registry and report every rule violation
    Validate,
    /// Print one method or protocol entry in full
    Show {
        /// The entry id, as the registry spells it
        id: String,
    },
}

pub fn run(
    command: &Command,
    directory: Option<&Path>,
    format: Format,
    renderer: &Renderer,
) -> Outcome {
    let registry = match crate::registry_source::load(directory) {
        Ok(registry) => registry,
        Err(error) => {
            return Outcome::declined(Declined::recorded(Refusal::registry_invalid(format!(
                "{error}"
            ))))
        }
    };

    match command {
        Command::Census => census(&registry, format),
        Command::Validate => validate(&registry, directory, format),
        Command::Show { id } => show(&registry, id, format, renderer),
    }
}

fn census(registry: &Registry, format: Format) -> Outcome {
    // Destructured without `..`, so a population added to the registry is a compile error
    // here rather than a row this command quietly stops printing.
    let Census {
        constructs,
        computation_entries: computation,
        protocol_entries,
        preset_entries,
    } = registry.census();
    let debates = registry.genuine_debates().count();
    let can_fail = registry.methods_that_can_fail().count();

    if format == Format::Json {
        return Outcome::complete(canonical(&json!({
            "constructs": constructs,
            "computation_entries": computation,
            "genuine_debates_of_computation_entries": debates,
            "can_find_the_wrong_event_of_computation_entries": can_fail,
            "protocol_entries": protocol_entries,
            "preset_entries": preset_entries,
        })));
    }

    // Every population is reported apart and none is summed with another. Both derived counts
    // are taken over the computation entries and say so, because indentation under the wrong
    // line is how a count loses its denominator.
    let mut document = String::new();
    let _ = writeln!(document, "{:<36}{}", "constructs", constructs);
    let _ = writeln!(document, "{:<36}{computation}", "computation entries");
    let _ = writeln!(
        document,
        "{:<36}{debates} of {computation}",
        "  of which genuine debates"
    );
    let _ = writeln!(
        document,
        "{:<36}{can_fail} of {computation}",
        "  of which can find the wrong event"
    );
    let _ = writeln!(document, "{:<36}{}", "protocol entries", protocol_entries);
    let _ = write!(document, "{:<36}{}", "preset entries", preset_entries);
    Outcome::complete(document)
}

fn validate(registry: &Registry, directory: Option<&Path>, format: Format) -> Outcome {
    let census = registry.census();
    if format == Format::Json {
        return Outcome::complete(canonical(&json!({
            "registry_source": crate::registry_source::describe(directory),
            "registry_digest": registry.content_digest,
            "computation_entries": census.computation_entries,
            "protocol_entries": census.protocol_entries,
            "constructs": census.constructs,
        })));
    }
    // The digest names which registry this was, measured from the bytes read rather than
    // declared beside them, so an id quoted in a methods section resolves to a version.
    Outcome::complete(format!(
        "{} is valid: {} computation entries, {} protocol entries, {} constructs\nregistry digest: {}",
        crate::registry_source::in_prose(directory),
        census.computation_entries,
        census.protocol_entries,
        census.constructs,
        registry.content_digest
    ))
}

/// The registry holds two populations, and an id belongs to one of them: the validator
/// refuses a registry where the same id appears in both.
fn show(registry: &Registry, id: &str, format: Format, renderer: &Renderer) -> Outcome {
    if let Some(method) = registry.methods.get(id) {
        return match format {
            Format::Json => Outcome::complete(canonical(&json!({ "method": method }))),
            Format::Text => Outcome::complete(show_method(method, renderer)),
        };
    }
    if let Some(protocol) = registry.protocols.get(id) {
        return match format {
            Format::Json => Outcome::complete(canonical(&json!({ "protocol": protocol }))),
            Format::Text => Outcome::complete(show_protocol(protocol, renderer)),
        };
    }
    // A lookup in a data file rather than a rule that declined, so it carries no published
    // code: the vocabulary names what a rule or a reader refused, and an id absent from the
    // registry reached neither.
    Outcome::declined_line(Fault::Request, format!("no entry with id {id}"))
}

/// Sorted keys and no spacing, so a document written here and one written by another surface
/// are the same string rather than two renderings of one object.
pub fn canonical(value: &serde_json::Value) -> String {
    canonical_under("ok", value)
}

/// The refusal envelope, written by the same function as the result so the two cannot drift
/// into two spellings of one document.
pub fn canonical_refusal(value: &serde_json::Value) -> String {
    canonical_under("refusal", value)
}

fn canonical_under(key: &str, value: &serde_json::Value) -> String {
    serde_json::to_string(&sorted(&json!({ key: value })))
        .expect("a value already in memory serialises")
}

/// `serde_json::Map` preserves insertion order unless the `preserve_order` feature is off,
/// in which case it is already a `BTreeMap`. Sorting here makes the output independent of
/// which of the two a build selected.
fn sorted(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for key in keys {
                out.insert(key.clone(), sorted(&map[key]));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(sorted).collect())
        }
        other => other.clone(),
    }
}

/// Where a labelled line's value starts, which is where anything written under one lines up.
const VALUE_INDENT_COLUMNS: usize = 14;

/// A rule the field has moved past is worth noticing on the row rather than in the reader's
/// memory of what the five statuses mean.
fn status(method: &Method, renderer: &Renderer) -> String {
    let spelling = method.status.to_string();
    match method.status {
        Status::Legacy | Status::Deprecated => renderer.paint(Role::NotCurrent, &spelling),
        Status::Recommended | Status::Accepted | Status::Contested => spelling,
    }
}

/// One parameter, and whether the reader has to answer it.
///
/// A name alone reads the same whether the rule will supply a value or the reader must, and
/// many registry parameters are the second: required, with nothing behind them. Both shapes
/// of default are read, so a parameter whose options are named rather than numbered needs no
/// second edit here.
fn describe_parameter(parameter: &plateforce_registry::Parameter) -> String {
    let mut described = parameter.name.clone();
    if let Some(value) = parameter.default {
        described.push_str(&format!(" = {value:?}"));
    } else if let Some(key) = &parameter.default_key {
        described.push_str(&format!(" = {key}"));
    }
    if parameter.required {
        described.push_str(", required");
    }
    if !parameter.published_values.is_empty() {
        described.push_str(&format!(
            ", published {}",
            join_numbers(&parameter.published_values)
        ));
    }
    described
}

/// What the registry says about a parameter beyond its name and its numbers, indented under
/// it.
///
/// 185 of the registry's 241 parameters carry one, and they hold the part a reader cannot
/// recover from the name: which of four studies disagreed about a window width and whether they
/// disagreed about the measurement or the acceptance criterion, that omitting a backtrack "is
/// not choosing 0 ms, it is failing to implement the cited method", which instant a rule reports
/// where it read a landmark off the trace. The browser has drawn them since it had a drawer;
/// the terminal is one of the four surfaces and was printing the name alone.
fn note_lines(notes: Option<&str>, renderer: &Renderer) -> Vec<String> {
    match notes.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => renderer.wrap(text, VALUE_INDENT_COLUMNS),
        None => Vec::new(),
    }
}

/// The values a parameter takes, one to a line under it. These are the values the rule
/// accepts, which is a different fact from `published_values`, the numbers a paper printed.
fn value_lines(parameter: &plateforce_registry::Parameter, renderer: &Renderer) -> Vec<String> {
    let key_columns = parameter
        .named_values
        .iter()
        .map(|value| value.key.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::new();
    for value in &parameter.named_values {
        let label = value.label.as_deref().unwrap_or_default().trim();
        if label.is_empty() {
            lines.extend(renderer.wrap(&value.key, VALUE_INDENT_COLUMNS));
            continue;
        }
        // Continuations align under the label rather than under the key, so a long label
        // reads as one column instead of wrapping back into the key beside it.
        let mut wrapped = renderer.wrap(label, VALUE_INDENT_COLUMNS + key_columns + 2);
        match wrapped.first_mut() {
            Some(first) => {
                first.replace_range(
                    ..VALUE_INDENT_COLUMNS + key_columns,
                    &format!(
                        "{}{:<key_columns$}",
                        " ".repeat(VALUE_INDENT_COLUMNS),
                        value.key
                    ),
                );
                lines.extend(wrapped);
            }
            None => lines.push(format!("{}{}", " ".repeat(VALUE_INDENT_COLUMNS), value.key)),
        }
    }
    lines
}

/// TOML floats carry a decimal point and `f64`'s Display drops it on a whole number, so `20`
/// on screen would not match the `20.0` in the file a reader goes on to search.
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

fn show_citations(citations: &[Citation], renderer: &Renderer, document: &mut String) {
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
        let reference = format!(
            "{} ({}) {}{doi}{obtained}",
            citation.key,
            citation_role_as_registry_spells_it(citation.role),
            citation.reference,
        );
        for line in renderer.field_wrapped("citation", &reference) {
            let _ = writeln!(document, "{line}");
        }
    }
}

fn show_method(method: &Method, renderer: &Renderer) -> String {
    let mut document = String::new();
    let _ = writeln!(document, "{}", renderer.paint(Role::Heading, &method.id));
    for line in renderer.field_wrapped("title", method.title.trim()) {
        let _ = writeln!(document, "{line}");
    }
    let _ = writeln!(
        document,
        "{}",
        renderer.field("construct", &method.construct)
    );
    let _ = writeln!(
        document,
        "{}",
        renderer.field("status", &status(method, renderer))
    );
    let _ = writeln!(
        document,
        "{}",
        renderer.field("confidence", &method.confidence.to_string())
    );
    for line in verdict::lines(method, renderer) {
        let _ = writeln!(document, "{line}");
    }
    for line in renderer.field_wrapped("rule", method.rule.trim()) {
        let _ = writeln!(document, "{line}");
    }

    for parameter in &method.parameters {
        for line in renderer.field_wrapped("parameter", &describe_parameter(parameter)) {
            let _ = writeln!(document, "{line}");
        }
        for line in note_lines(parameter.notes.as_deref(), renderer) {
            let _ = writeln!(document, "{line}");
        }
        for line in value_lines(parameter, renderer) {
            let _ = writeln!(document, "{line}");
        }
    }
    for bias in &method.biases {
        let described = format!(
            "{} {} against {}{}",
            bias.magnitude,
            bias.unit,
            bias.criterion,
            if bias.conditional_on_success {
                ", conditional on the rule not failing"
            } else {
                ""
            }
        );
        for line in renderer.field_wrapped("bias", &described) {
            let _ = writeln!(document, "{line}");
        }
    }
    if let Some(failure) = &method.failure {
        let _ = writeln!(
            document,
            "  FAILS       {} of {} trials ({:.1}%), {}, on {}",
            failure.numerator,
            failure.denominator,
            failure.rate * 100.0,
            failure.detectability,
            failure.corpus
        );
        for line in renderer.wrap(failure.definition.trim(), VALUE_INDENT_COLUMNS) {
            let _ = writeln!(document, "{line}");
        }
    }
    show_citations(&method.citations, renderer, &mut document);
    let _ = document.pop();
    document
}

/// A protocol entry carries no rule, no parameters and no bias, so a method-shaped block
/// with those lines blank would report absent fields as empty ones.
fn show_protocol(protocol: &Protocol, renderer: &Renderer) -> String {
    let mut document = String::new();
    let _ = writeln!(document, "{}", renderer.paint(Role::Heading, &protocol.id));
    for line in renderer.field_wrapped("title", protocol.title.trim()) {
        let _ = writeln!(document, "{line}");
    }
    let _ = writeln!(document, "{}", renderer.field("area", &protocol.area));
    let _ = writeln!(
        document,
        "{}",
        renderer.field(
            "provenance",
            provenance_as_registry_spells_it(protocol.provenance)
        )
    );
    for line in renderer.field_wrapped("description", protocol.description.trim()) {
        let _ = writeln!(document, "{line}");
    }
    for affected in &protocol.affects {
        let _ = writeln!(document, "{}", renderer.field("affects", affected));
    }
    show_citations(&protocol.citations, renderer, &mut document);
    let _ = document.pop();
    document
}
