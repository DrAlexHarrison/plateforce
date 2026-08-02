//! The chain of methods behind one number.

use crate::Provenance;

/// A provenance and the provenances of the results it was computed from.
///
/// Jump height moves with the onset rule and the weighing epoch as well as with the
/// jump-height formula, so a result that named only the last step would understate what
/// produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProvenanceChain {
    pub provenance: Provenance,
    /// Choices that select between named alternatives rather than between numbers, such
    /// as population against sample standard deviation. `Provenance::bound_parameters` is
    /// a list of `(String, f64)` and cannot hold one.
    pub enumerated_choices: Vec<(String, String)>,
    pub depends_on: Vec<ProvenanceChain>,
}

impl ProvenanceChain {
    pub fn leaf(provenance: Provenance) -> Self {
        Self {
            provenance,
            enumerated_choices: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    pub fn with_inputs(provenance: Provenance, depends_on: Vec<ProvenanceChain>) -> Self {
        Self {
            provenance,
            enumerated_choices: Vec::new(),
            depends_on,
        }
    }

    pub fn choosing(mut self, choices: Vec<(String, String)>) -> Self {
        self.enumerated_choices = choices;
        self
    }

    /// This step and every one upstream of it, depth first.
    ///
    /// The parameter that moved a downstream number usually sits on an upstream step: the
    /// k that placed onset is on the onset entry, not on the time to takeoff derived from it.
    pub fn flattened(&self) -> Vec<&ProvenanceChain> {
        let mut collected = Vec::new();
        self.collect_into(&mut collected);
        collected
    }

    fn collect_into<'a>(&'a self, into: &mut Vec<&'a ProvenanceChain>) {
        into.push(self);
        for input in &self.depends_on {
            input.collect_into(into);
        }
    }

    /// The step naming this method anywhere in this chain, or None when the chain does not
    /// include it.
    pub fn step_of(&self, method_id: &str) -> Option<&ProvenanceChain> {
        self.flattened()
            .into_iter()
            .find(|step| step.provenance.method_id == method_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(method_id: &str) -> Provenance {
        Provenance {
            method_id: method_id.to_string(),
            bound_parameters: Vec::new(),
            registry_version: None,
            registry_digest: None,
            acquisition_complete: true,
        }
    }

    #[test]
    fn flattened_reaches_every_depth() {
        let chain = ProvenanceChain::with_inputs(
            step("jumpheight.takeoff.impulse_momentum"),
            vec![ProvenanceChain::with_inputs(
                step("onset.threshold.noise_relative"),
                vec![ProvenanceChain::leaf(step("bwepoch.fixed_window"))],
            )],
        );

        let ids: Vec<&str> = chain
            .flattened()
            .iter()
            .map(|step| step.provenance.method_id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "jumpheight.takeoff.impulse_momentum",
                "onset.threshold.noise_relative",
                "bwepoch.fixed_window",
            ]
        );
    }

    #[test]
    fn a_method_three_deep_is_still_found() {
        let chain = ProvenanceChain::with_inputs(
            step("jumpheight.takeoff.impulse_momentum"),
            vec![ProvenanceChain::with_inputs(
                step("onset.threshold.noise_relative"),
                vec![ProvenanceChain::leaf(step("bwepoch.fixed_window"))],
            )],
        );

        assert!(chain.step_of("bwepoch.fixed_window").is_some());
        assert!(chain.step_of("takeoff.threshold.absolute_force").is_none());
    }
}
