//! What a result says about itself when a person reads it.
//!
//! The sentence is generated here and nowhere else, so a number carries the same account
//! of itself in a notebook, a browser tab, an R session and a file somebody emailed on.

use crate::provenance::ProvenanceChain;
use crate::{Measured, Provenance};

/// Multi-line account of a value and every choice behind it, upstream steps included.
///
/// The chain's root provenance is the one on `measured`. The chain is a separate argument
/// because `Measured` carries a single step and has no field for what fed it.
pub fn describe(measured: &Measured, chain: &ProvenanceChain) -> String {
    let mut lines = vec![format!("{} {}", measured.value, measured.unit)];
    describe_step(chain, 0, &mut lines);
    if !measured.provenance.acquisition_complete {
        lines.push(
            "  acquisition block incomplete, so this result cannot be declared to match another lab's"
                .to_string(),
        );
    }
    lines.join("\n")
}

fn describe_step(chain: &ProvenanceChain, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth + 1);
    lines.push(format!(
        "{indent}{} {}",
        chain.provenance.method_id,
        format_parameters(&chain.provenance.bound_parameters)
    ));
    for (name, value) in &chain.enumerated_choices {
        lines.push(format!("{indent}  {name} = {value}"));
    }
    if depth == 0 {
        if let Some(named) = registry_line(&chain.provenance) {
            lines.push(format!("{indent}{named}"));
        }
    }
    for input in &chain.depends_on {
        describe_step(input, depth + 1, lines);
    }
}

/// The values a rule was bound to, as a reader sees them.
pub fn format_parameters(parameters: &[(String, f64)]) -> String {
    if parameters.is_empty() {
        return "{}".to_string();
    }
    let body = parameters
        .iter()
        .map(|(name, value)| format!("'{name}': {value}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{body}}}")
}

/// Names the registry behind a result: the pinned revision, the measured digest, or both.
/// None when the result was computed without reading a registry.
fn registry_line(provenance: &Provenance) -> Option<String> {
    match (
        provenance.registry_version.as_deref(),
        provenance.registry_digest.as_deref(),
    ) {
        (Some(version), Some(digest)) => Some(format!("registry {version} ({digest})")),
        (Some(version), None) => Some(format!("registry {version}")),
        (None, Some(digest)) => Some(format!("registry {digest}")),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance(method_id: &str, bound_parameters: Vec<(String, f64)>) -> Provenance {
        Provenance {
            method_id: method_id.to_string(),
            bound_parameters,
            registry_version: Some("fixture-1".to_string()),
            registry_digest: Some("content-abc".to_string()),
            acquisition_complete: true,
        }
    }

    fn jump_height_chain() -> (Measured, ProvenanceChain) {
        let weighing = ProvenanceChain::leaf(provenance(
            "bwepoch.fixed_window",
            vec![("duration".to_string(), 1.0)],
        ));
        let onset = ProvenanceChain::with_inputs(
            provenance(
                "onset.threshold.noise_relative",
                vec![("k".to_string(), 5.0)],
            ),
            vec![weighing],
        )
        .choosing(vec![("dispersion".to_string(), "sample".to_string())]);
        let top = provenance("jumpheight.takeoff.impulse_momentum", Vec::new());
        let chain = ProvenanceChain::with_inputs(top.clone(), vec![onset]);
        let measured = Measured {
            value: 0.34,
            unit: "meters",
            provenance: top,
        };
        (measured, chain)
    }

    #[test]
    fn describe_names_the_value_and_every_step() {
        let (measured, chain) = jump_height_chain();
        assert_eq!(
            describe(&measured, &chain),
            "0.34 meters\n  \
             jumpheight.takeoff.impulse_momentum {}\n  \
             registry fixture-1 (content-abc)\n    \
             onset.threshold.noise_relative {'k': 5}\n      \
             dispersion = sample\n      \
             bwepoch.fixed_window {'duration': 1}"
        );
    }

    #[test]
    fn an_incomplete_acquisition_block_says_so() {
        let (mut measured, chain) = jump_height_chain();
        measured.provenance.acquisition_complete = false;
        assert!(describe(&measured, &chain).ends_with(
            "acquisition block incomplete, so this result cannot be declared to match another lab's"
        ));
    }

    #[test]
    fn a_result_computed_without_a_registry_names_none() {
        let mut step = provenance("bwepoch.fixed_window", Vec::new());
        step.registry_version = None;
        step.registry_digest = None;
        let measured = Measured {
            value: 812.0,
            unit: "newtons",
            provenance: step.clone(),
        };
        let described = describe(&measured, &ProvenanceChain::leaf(step));
        assert_eq!(described, "812 newtons\n  bwepoch.fixed_window {}");
    }
}
