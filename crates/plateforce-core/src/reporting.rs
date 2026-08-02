//! What a result says about itself when a person reads it, and what it says about itself to
//! another result.
//!
//! The sentence is generated here and nowhere else, so a number carries the same account
//! of itself in a notebook, a browser tab, an R session and a file somebody emailed on.

use plateforce_registry::content_digest;

use crate::acquisition::Acquisition;
use crate::provenance::ProvenanceChain;
use crate::{Measured, Provenance};

/// Identity of a result, over every method that produced it and the plate it was captured on.
///
/// Equality is partial in the same sense a NaN's is. A fingerprint taken over an acquisition
/// block that could not be filled matches nothing, itself included, because the settings that
/// would decide whether two labs agree were never recorded. The most consequential setting in
/// one open tool is a contact debounce living in firmware, and knowing the trace does not
/// recover it.
#[derive(Debug, Clone)]
pub struct Fingerprint {
    /// False when the acquisition block was not fully filled.
    pub complete: bool,
    pub digest: String,
}

impl PartialEq for Fingerprint {
    fn eq(&self, other: &Self) -> bool {
        self.complete && other.complete && self.digest == other.digest
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.complete {
            write!(formatter, "{}", self.digest)
        } else {
            write!(formatter, "{}-incomplete", self.digest)
        }
    }
}

/// What proves two labs computed the same quantity: the whole chain of methods and their
/// bound values, plus the acquisition block.
///
/// The chain is taken over `depends_on` rather than the top step alone, because the parameter
/// that moved the number usually sits upstream of the method that reported it. Each value
/// carries its source, since two runs that reached one number from a stated value and from a
/// registry default did not compute it the same way.
pub fn fingerprint(
    provenance: &Provenance,
    acquisition: &Acquisition,
    sample_rate_hz: f64,
) -> Fingerprint {
    let mut material: Vec<(String, String)> = Vec::new();
    for (depth, step) in provenance.flattened().iter().enumerate() {
        let at = |field: &str| format!("analysis/{depth:04}/{field}");
        material.push((at("method_id"), step.method_id.clone()));
        material.push((at("method_source"), format!("{:?}", step.method_source)));
        for parameter in &step.parameters {
            material.push((
                at(&format!("parameter/{}", parameter.name)),
                format!("{} {:?}", parameter.value, parameter.source),
            ));
        }
        for choice in &step.choices {
            material.push((
                at(&format!("choice/{}", choice.name)),
                format!("{} {:?}", choice.value, choice.source),
            ));
        }
        material.push((
            at("registry_digest"),
            step.registry_digest.clone().unwrap_or_default(),
        ));
        material.push((
            at("registry_version"),
            step.registry_version.clone().unwrap_or_default(),
        ));
    }

    material.push((
        "acquisition/sample_rate_hz".to_string(),
        format!("{sample_rate_hz}"),
    ));
    for (member, value) in acquisition.members_as_text() {
        material.push((format!("acquisition/{member}"), value));
    }

    let digest = content_digest(
        material
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    Fingerprint {
        complete: acquisition.is_complete(),
        digest,
    }
}

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
        format_parameters(&chain.provenance.bound_parameters())
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
mod fingerprint_tests {
    use super::*;
    use crate::provenance::{ParameterRecord, ParameterSource};

    fn filled_block() -> Acquisition {
        Acquisition {
            filter_at_capture: Some("none".to_string()),
            tare_state: Some("tared_before_trial".to_string()),
            plate_natural_frequency_hz: Some(400.0),
            floor_surface: Some("concrete".to_string()),
            firmware_version: Some("2.4.1".to_string()),
        }
    }

    /// Jump height over onset, with `k` bound on the upstream step where it actually sits.
    fn chain(k: f64, source: ParameterSource) -> Provenance {
        let mut onset = Provenance::of("onset.threshold.noise_relative");
        onset.parameters.push(ParameterRecord {
            name: "k".to_string(),
            value: k,
            source,
        });
        let mut root = Provenance::of("jumpheight.takeoff.impulse_momentum");
        root.depends_on.push(onset);
        root
    }

    #[test]
    fn two_runs_that_computed_the_same_way_match() {
        let left = fingerprint(
            &chain(5.0, ParameterSource::Stated),
            &filled_block(),
            1200.0,
        );
        let right = fingerprint(
            &chain(5.0, ParameterSource::Stated),
            &filled_block(),
            1200.0,
        );
        assert!(left.complete);
        assert_eq!(left, right);
    }

    #[test]
    fn a_parameter_moved_upstream_is_a_different_result() {
        assert_ne!(
            fingerprint(
                &chain(5.0, ParameterSource::Stated),
                &filled_block(),
                1200.0
            ),
            fingerprint(
                &chain(3.0, ParameterSource::Stated),
                &filled_block(),
                1200.0
            )
        );
    }

    #[test]
    fn one_number_reached_two_ways_is_two_results() {
        assert_ne!(
            fingerprint(
                &chain(5.0, ParameterSource::Stated),
                &filled_block(),
                1200.0
            ),
            fingerprint(
                &chain(5.0, ParameterSource::Assumed),
                &filled_block(),
                1200.0
            )
        );
    }

    #[test]
    fn a_different_plate_or_rate_is_a_different_result() {
        let base = chain(5.0, ParameterSource::Stated);
        let mut other_plate = filled_block();
        other_plate.firmware_version = Some("2.4.2".to_string());
        assert_ne!(
            fingerprint(&base, &filled_block(), 1200.0),
            fingerprint(&base, &other_plate, 1200.0)
        );
        assert_ne!(
            fingerprint(&base, &filled_block(), 1200.0),
            fingerprint(&base, &filled_block(), 1000.0)
        );
    }

    #[test]
    fn a_block_that_could_not_be_filled_matches_nothing_including_itself() {
        let unfilled = fingerprint(
            &chain(5.0, ParameterSource::Stated),
            &Acquisition::default(),
            1200.0,
        );
        let same = fingerprint(
            &chain(5.0, ParameterSource::Stated),
            &Acquisition::default(),
            1200.0,
        );
        assert!(!unfilled.complete);
        assert_ne!(unfilled, same);
        assert_ne!(unfilled, unfilled);
        assert_ne!(
            unfilled,
            fingerprint(
                &chain(5.0, ParameterSource::Stated),
                &filled_block(),
                1200.0
            )
        );
    }

    #[test]
    fn a_partly_filled_block_is_still_incomplete() {
        let mut partial = filled_block();
        partial.firmware_version = None;
        let taken = fingerprint(&chain(5.0, ParameterSource::Stated), &partial, 1200.0);
        assert!(!taken.complete);
        assert_ne!(taken, taken);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance(method_id: &str, bound_parameters: Vec<(String, f64)>) -> Provenance {
        use crate::provenance::{ParameterRecord, ParameterSource};
        Provenance {
            parameters: bound_parameters
                .into_iter()
                .map(|(name, value)| ParameterRecord {
                    name,
                    value,
                    source: ParameterSource::Stated,
                })
                .collect(),
            registry_version: Some("fixture-1".to_string()),
            registry_digest: Some("content-abc".to_string()),
            acquisition_complete: true,
            ..Provenance::of(method_id)
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
