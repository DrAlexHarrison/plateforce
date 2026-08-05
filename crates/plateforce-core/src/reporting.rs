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

impl Fingerprint {
    /// The digest a surface writes, and nothing at all when the acquisition block was not
    /// filled.
    ///
    /// The only way out of this type, so the wire cannot disagree with `PartialEq` above.
    /// `Acquisition::missing` names what to go and find, and a surface reporting a null
    /// digest beside it says which runs would fingerprint once somebody does.
    pub fn published(&self) -> Option<&str> {
        self.complete.then_some(self.digest.as_str())
    }
}

/// What proves two labs computed the same quantity: the whole chain of methods and their
/// bound values, the sample a hand supplied for any landmark it placed, plus the acquisition
/// block.
///
/// The chain is taken over `depends_on` rather than the top step alone, because the parameter
/// that moved the number usually sits upstream of the method that reported it. Each value
/// carries its source, since two runs that reached one number from a stated value and from a
/// registry default did not compute it the same way.
///
/// What is deliberately not material, each for the same reason: it moves no number, or the
/// material already carries what moves it.
///
/// - `not_read` is the names the request carried that this rule ignored. A caller who typed a
///   name the rule never reads changed nothing about the number.
/// - `registry_entry` and `composed_from` are facts about which registry row files this id.
///   Two runs whose `registry_digest` agrees answer them identically, so hashing them would
///   record one fact twice.
/// - `preset` names the published pipeline a caller adopted. The values it put on the path are
///   in `parameters` and `choices` already, each carrying `cited` as its source, so two labs
///   reaching one rule and one set of values from two pipelines computed the same quantity.
/// - `registry_declared_version` is the registry's claim about itself. Two labs whose rule
///   bytes are identical computed the same quantity whatever their VERSION files say, and
///   hashing the claim would break every recorded match on a VERSION-only edit.
pub fn fingerprint(
    provenance: &Provenance,
    acquisition: &Acquisition,
    sample_rate_hz: f64,
) -> Fingerprint {
    let mut material: Vec<(String, String)> = Vec::new();
    for (depth, step) in provenance.flattened().iter().enumerate() {
        let at = |field: &str| format!("analysis/{depth:04}/{field}");
        material.push((at("method_id"), step.method_id.clone()));
        // Sources enter as their wire names, so renaming a variant cannot move a digest.
        material.push((
            at("method_source"),
            step.method_source.wire_name().to_string(),
        ));
        // The sample and not the fact of a hand, because two hands placing two different
        // samples give two numbers. Written only where a hand placed one, so a run nobody
        // touched keeps the digest it already had.
        if let Some(sample) = step.placed_by_hand_at_sample {
            material.push((at("placed_by_hand_at_sample"), sample.to_string()));
        }
        for parameter in &step.parameters {
            material.push((
                at(&format!("parameter/{}", parameter.name)),
                format!("{} {}", parameter.value, parameter.source.wire_name()),
            ));
        }
        for choice in &step.choices {
            material.push((
                at(&format!("choice/{}", choice.name)),
                format!("{} {}", choice.value, choice.source.wire_name()),
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

/// Names the registry behind a result: what the caller pinned, what the registry claims about
/// itself, and the measured digest, in that order and only the ones there are. None when the
/// result was computed without reading a registry.
///
/// The two revisions are worded apart rather than printed as one number, because a reader who
/// cannot tell them apart reads the registry's claim as the author's citation.
fn registry_line(provenance: &Provenance) -> Option<String> {
    let mut said = String::new();
    if let Some(pinned) = provenance.registry_version.as_deref() {
        said.push_str(&format!(" pinned to {pinned}"));
    }
    if let Some(declared) = provenance.registry_declared_version.as_deref() {
        said.push_str(&format!(" declaring {declared}"));
    }
    if let Some(digest) = provenance.registry_digest.as_deref() {
        // Bare when it stands alone, parenthesised behind a revision it qualifies.
        said.push_str(&if said.is_empty() {
            format!(" {digest}")
        } else {
            format!(" ({digest})")
        });
    }
    if said.is_empty() {
        return None;
    }
    Some(format!("registry{said}"))
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
    fn the_material_spells_sources_by_wire_name_not_by_variant() {
        the_material_is(&chain(5.0, ParameterSource::Stated), &[]);
    }

    /// A hand placement adds one key and moves nothing else, so a run nobody touched keeps the
    /// digest it had.
    ///
    /// Held by hand-building both materials rather than by pinning a digest, because a digest
    /// written into a committed file is a registry digest to every reader and to
    /// `digests_in_prose`, and this is not one.
    #[test]
    fn a_hand_placement_adds_one_key_to_the_material_and_moves_nothing_else() {
        the_material_is(
            &placed_by_hand(5.0, 1180),
            &[("analysis/0001/placed_by_hand_at_sample", "1180")],
        );
    }

    /// The material for the two-step chain above, with `extra` merged in, hand-spelled rather
    /// than read back from the function under test.
    ///
    /// Goes red if the material reverts to Debug formatting, where "Stated" replaces "stated",
    /// and red if a key joins it that no caller here asked for: a placement written
    /// unconditionally would appear on the run that has none and move every digest ever
    /// recorded.
    fn the_material_is(provenance: &Provenance, extra: &[(&str, &str)]) {
        let block = filled_block();
        let printed = fingerprint(provenance, &block, 1200.0);

        let mut material: Vec<(String, String)> = vec![
            (
                "analysis/0000/method_id".to_string(),
                "jumpheight.takeoff.impulse_momentum".to_string(),
            ),
            (
                "analysis/0000/method_source".to_string(),
                "assumed".to_string(),
            ),
            ("analysis/0000/registry_digest".to_string(), String::new()),
            ("analysis/0000/registry_version".to_string(), String::new()),
            (
                "analysis/0001/method_id".to_string(),
                "onset.threshold.noise_relative".to_string(),
            ),
            (
                "analysis/0001/method_source".to_string(),
                "assumed".to_string(),
            ),
            (
                "analysis/0001/parameter/k".to_string(),
                "5 stated".to_string(),
            ),
            ("analysis/0001/registry_digest".to_string(), String::new()),
            ("analysis/0001/registry_version".to_string(), String::new()),
            ("acquisition/sample_rate_hz".to_string(), "1200".to_string()),
        ];
        for (member, value) in block.members_as_text() {
            material.push((format!("acquisition/{member}"), value));
        }
        for (key, value) in extra {
            material.push(((*key).to_string(), (*value).to_string()));
        }

        let expected = content_digest(
            material
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        assert_eq!(printed.digest, expected);
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

    /// The same chain with the onset sample placed by a hand rather than found by the rule.
    /// One field apart from `chain`, so a digest that moves moved for that field.
    fn placed_by_hand(k: f64, at_sample: usize) -> Provenance {
        let mut root = chain(k, ParameterSource::Stated);
        root.depends_on[0].placed_by_hand_at_sample = Some(at_sample);
        root
    }

    /// A hand placing the sample a rule would have found is still not that detection.
    ///
    /// The pair is one field apart. An engine-level comparison cannot ask this: a dragged marker
    /// rests on nothing, so its chain loses a step and two digests differ whether or not the
    /// placement is material at all.
    #[test]
    fn a_landmark_a_hand_placed_is_not_the_detection_that_found_the_same_sample() {
        let block = filled_block();
        let detected = fingerprint(&chain(5.0, ParameterSource::Stated), &block, 1200.0);
        let by_hand = fingerprint(&placed_by_hand(5.0, 1180), &block, 1200.0);

        assert!(detected.complete && by_hand.complete);
        assert_ne!(
            detected, by_hand,
            "a rule finding the sample and a hand supplying it fingerprint as one result"
        );
    }

    /// Two hands placing two samples give two numbers, so the material carries the sample and
    /// not the fact of a hand. A flag satisfies the guard above and leaves this one red.
    #[test]
    fn two_hands_placing_two_samples_are_two_results() {
        let block = filled_block();
        assert_ne!(
            fingerprint(&placed_by_hand(5.0, 1180), &block, 1200.0),
            fingerprint(&placed_by_hand(5.0, 1120), &block, 1200.0),
            "two hand placements 60 samples apart fingerprint as one result"
        );
    }

    /// The control on both, so neither passes on a build where every fingerprint differs from
    /// every other. One placement repeated is one result.
    #[test]
    fn one_hand_placing_one_sample_twice_is_one_result() {
        let block = filled_block();
        assert_eq!(
            fingerprint(&placed_by_hand(5.0, 1180), &block, 1200.0),
            fingerprint(&placed_by_hand(5.0, 1180), &block, 1200.0)
        );
    }

    /// A hand placing a landmark at sample zero is a hand placing a landmark. Written apart from
    /// the guards above because zero is the sample a flag derived from the value gets wrong, and
    /// every other sample in this file would pass a build that read the placement as a boolean
    /// the wrong way round.
    #[test]
    fn a_landmark_placed_at_sample_zero_is_still_placed_by_a_hand() {
        let block = filled_block();
        assert_ne!(
            fingerprint(&chain(5.0, ParameterSource::Stated), &block, 1200.0),
            fingerprint(&placed_by_hand(5.0, 0), &block, 1200.0),
            "a landmark placed at sample zero fingerprints as the detection"
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

    /// What a surface writes has to agree with what `PartialEq` says, because a reader
    /// comparing two records compares the written values and never calls `eq`.
    ///
    /// The guard is over two incomplete fingerprints taken over *different* material. Written
    /// over one, it would pass against a `published` that returned the digest, since one value
    /// equals itself either way, and the case it exists to catch would be out of reach.
    #[test]
    fn two_incomplete_fingerprints_publish_nothing_to_compare() {
        let mut one_plate = filled_block();
        one_plate.firmware_version = None;
        let mut another_plate = filled_block();
        another_plate.firmware_version = None;
        another_plate.floor_surface = Some("sprung".to_string());

        let left = fingerprint(&chain(5.0, ParameterSource::Stated), &one_plate, 1200.0);
        let right = fingerprint(&chain(5.0, ParameterSource::Stated), &another_plate, 1200.0);

        // The two took different material, so the digests differ and the incompleteness is
        // not being read off two runs that were the same anyway.
        assert_ne!(left.digest, right.digest);
        assert_ne!(left, right, "an incomplete fingerprint matches nothing");
        assert_eq!(left.published(), None);
        assert_eq!(right.published(), None);
    }

    /// The other half, so the guard above cannot be satisfied by a `published` that returns
    /// nothing for everything.
    #[test]
    fn a_filled_block_publishes_the_digest_it_measured() {
        let taken = fingerprint(
            &chain(5.0, ParameterSource::Stated),
            &filled_block(),
            1200.0,
        );

        assert_eq!(taken.published(), Some(taken.digest.as_str()));
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
            // Unlike the pin above. A fixture whose registry claimed the same revision the
            // caller pinned would read the same whichever field a value came out of.
            registry_declared_version: Some("fixture-declares-2".to_string()),
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
             registry pinned to fixture-1 declaring fixture-declares-2 (content-abc)\n    \
             onset.threshold.noise_relative {'k': 5}\n      \
             dispersion = sample\n      \
             bwepoch.fixed_window {'duration': 1}"
        );
    }

    /// Each revision is worded as whose it is, and a reader is never handed one of them
    /// bare. A line that printed the revision alone reads identically whether the caller
    /// cited it or the registry claimed it about itself.
    #[test]
    fn the_registry_line_says_whose_each_revision_is() {
        let (measured, chain) = jump_height_chain();

        let mut nobody_pinned = chain.clone();
        nobody_pinned.provenance.registry_version = None;
        let said = describe(&measured, &nobody_pinned);
        assert!(
            said.contains("registry declaring fixture-declares-2 (content-abc)"),
            "{said}"
        );
        assert!(!said.contains("pinned to"), "{said}");

        let mut claims_nothing = chain.clone();
        claims_nothing.provenance.registry_declared_version = None;
        let said = describe(&measured, &claims_nothing);
        assert!(
            said.contains("registry pinned to fixture-1 (content-abc)"),
            "{said}"
        );
        assert!(!said.contains("declaring"), "{said}");
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
        // A registry that was never read claims nothing about itself either. Leaving this
        // set is a registry the result half-names, and the line below would print it.
        step.registry_declared_version = None;
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
