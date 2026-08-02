//! Reading the registry: what it holds, whether it loads, and one entry in full.

use std::fmt::Write as _;
use std::path::Path;

use plateforce_registry::{Citation, CitationRole, Method, Protocol, Provenance, Registry, Status};
use serde_json::json;

use crate::exit::{Fault, Outcome};
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

pub fn run(command: &Command, directory: &Path, format: Format, renderer: &Renderer) -> Outcome {
    let registry = match Registry::load(directory) {
        Ok(registry) => registry,
        Err(error) => return Outcome::declined(Fault::Registry, format!("{error}")),
    };

    match command {
        Command::Census => census(&registry, format),
        Command::Validate => validate(&registry, directory, format),
        Command::Show { id } => show(&registry, id, format, renderer),
    }
}

fn census(registry: &Registry, format: Format) -> Outcome {
    let census = registry.census();
    let computation = census.computation_entries;
    let debates = registry.genuine_debates().count();
    let can_fail = registry.methods_that_can_fail().count();

    if format == Format::Json {
        return Outcome::complete(canonical(&json!({
            "constructs": census.constructs,
            "computation_entries": computation,
            "genuine_debates_of_computation_entries": debates,
            "can_find_the_wrong_event_of_computation_entries": can_fail,
            "protocol_entries": census.protocol_entries,
        })));
    }

    // Populations are reported apart and never summed. Both derived counts are taken over
    // the computation entries and say so, because indentation under the wrong line is how a
    // count loses its denominator. Both of this project's headline counts were assertions
    // until somebody recounted them.
    let mut document = String::new();
    let _ = writeln!(document, "{:<36}{}", "constructs", census.constructs);
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
    let _ = write!(
        document,
        "{:<36}{}",
        "protocol entries", census.protocol_entries
    );
    Outcome::complete(document)
}

fn validate(registry: &Registry, directory: &Path, format: Format) -> Outcome {
    let census = registry.census();
    if format == Format::Json {
        return Outcome::complete(canonical(&json!({
            "registry_directory": directory.display().to_string(),
            "registry_digest": registry.content_digest,
            "computation_entries": census.computation_entries,
            "protocol_entries": census.protocol_entries,
            "constructs": census.constructs,
        })));
    }
    // The digest names which registry this was, measured from the bytes read rather than
    // declared beside them, so an id quoted in a methods section resolves to a version.
    Outcome::complete(format!(
        "registry at {} is valid: {} computation entries, {} protocol entries, {} constructs\nregistry digest: {}",
        directory.display(),
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
    Outcome::declined(Fault::Request, format!("no entry with id {id}"))
}

/// Sorted keys and no spacing, so a document written here and one written by another surface
/// are the same string rather than two renderings of one object.
pub fn canonical(value: &serde_json::Value) -> String {
    serde_json::to_string(&sorted(&json!({ "ok": value })))
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

/// A rule the field has moved past is worth noticing on the row rather than in the reader's
/// memory of what the five statuses mean.
fn status(method: &Method, renderer: &Renderer) -> String {
    let spelling = method.status.to_string();
    match method.status {
        Status::Legacy | Status::Deprecated => renderer.paint(Role::NotCurrent, &spelling),
        Status::Recommended | Status::Accepted | Status::Contested => spelling,
    }
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
        let published = if parameter.published_values.is_empty() {
            String::new()
        } else {
            format!(", published {}", join_numbers(&parameter.published_values))
        };
        let described = format!(
            "{}{}{}",
            parameter.name,
            parameter
                .default
                .map(|value| format!(" = {value:?}"))
                .unwrap_or_default(),
            published
        );
        for line in renderer.field_wrapped("parameter", &described) {
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
        for line in renderer.wrap(failure.definition.trim(), 14) {
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
