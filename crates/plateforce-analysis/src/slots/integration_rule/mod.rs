//! Net impulse under each quadrature rule in the registry.

pub mod rectangle;
pub mod simpson;
pub mod trapezoid;

use plateforce_core::{
    centre_of_mass_velocity_meters_per_second, QuadratureRule,
    STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
};

use crate::binding::{ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT};
use crate::centre_of_mass;
use crate::derived::{DerivedContext, DerivedOutcome};
use crate::request::MethodChoice;
use crate::resolution::Resolution;
use crate::slots::net_impulse;

pub const CONSTRUCT: &str = "net_impulse";

fn compute(
    context: &DerivedContext,
    choice: &MethodChoice,
    method_id: &'static str,
    quadrature: QuadratureRule,
) -> DerivedOutcome {
    let mut resolved = Resolution::over(&choice.parameters, &choice.options, choice.claims());
    let Some(landmarks) = context.landmarks() else {
        return DerivedOutcome::declined(
            resolved.finish(),
            context.unavailable(method_id, &[ONSET_CONSTRUCT, TAKEOFF_CONSTRUCT]),
        );
    };

    let mut spec = plateforce_core::takeoff_velocity_integration_spec(&landmarks);
    spec.quadrature = quadrature;
    centre_of_mass::record_operators(&mut resolved, &spec);
    context.rests_on(net_impulse::KEY, &spec.method_ids());
    context.rests_on(net_impulse::VELOCITY_KEY, &spec.method_ids());

    let epoch = context.epoch();
    let last_contact_index = landmarks.takeoff_index.saturating_sub(1);
    // The scale cancels from impulse, and holding it constant keeps that gravity-independent
    // quantity bit-stable when the analysis gravity changes.
    let impulse_velocity_series = centre_of_mass_velocity_meters_per_second(
        context.trial,
        epoch,
        &spec,
        STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    );
    let impulse = impulse_velocity_series
        .at(last_contact_index)
        .expect("a placed takeoff is inside the trial")
        * epoch.system_mass_kilograms(STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED);

    let gravity = context.gravity_behind(Some(net_impulse::VELOCITY_KEY));
    let velocity_series =
        centre_of_mass_velocity_meters_per_second(context.trial, epoch, &spec, gravity);
    let velocity = velocity_series
        .at(last_contact_index)
        .expect("a placed takeoff is inside the trial");

    DerivedOutcome {
        values: vec![
            (net_impulse::KEY, Some(impulse)),
            (net_impulse::VELOCITY_KEY, Some(velocity)),
        ],
        placed: Vec::new(),
        bound: resolved.finish(),
        refusal: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::slots::{integration_rule::trapezoid, net_impulse};
    use crate::{run, AnalysisRequest, MethodChoice, WeighingChoice, BINDINGS};

    fn subject01_trial1() -> plateforce_core::Trial {
        plateforce_core::read_trial_from_path(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plateforce-conformance/fixtures/subject01_trial1.force.txt"
            ),
            '\t',
            0,
            1200.0,
        )
        .expect("the committed trial reads")
        .0
    }

    fn request(method_id: Option<&str>) -> AnalysisRequest {
        let mut request = AnalysisRequest {
            weighing: WeighingChoice {
                method_id: "bwepoch.fixed_window".into(),
                parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
                ..Default::default()
            },
            onset: MethodChoice {
                method_id: "onset.threshold.noise_relative".into(),
                ..Default::default()
            },
            takeoff: MethodChoice {
                method_id: "takeoff.threshold.absolute_force".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        if let Some(id) = method_id {
            request.derived.insert(
                super::CONSTRUCT.to_string(),
                MethodChoice {
                    method_id: id.to_string(),
                    ..Default::default()
                },
            );
        }
        request
    }

    #[test]
    fn every_registry_quadrature_runs_and_names_the_rule_that_integrated() {
        let registry = plateforce_registry::Registry::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../registry"
        ))
        .expect("the shipped registry loads");
        let mut ids: Vec<&str> = registry
            .methods
            .values()
            .filter(|entry| {
                entry.construct == super::CONSTRUCT && entry.id.starts_with("integration.rule.")
            })
            .map(|entry| entry.id.as_str())
            .collect();
        ids.sort_unstable();
        assert!(
            registry
                .methods
                .contains_key("integration.direction.forward"),
            "the prefix control found no neighbouring integration entry"
        );
        assert!(
            ids.iter()
                .all(|id| BINDINGS.iter().any(|binding| binding.id == *id)),
            "an integration rule in the registry has no binding: {ids:?}"
        );

        let trial = subject01_trial1();
        let mut results = Vec::new();
        for id in &ids {
            let response = run(&trial, &request(Some(id))).expect("the quadrature rule runs");
            let impulse = response
                .metrics
                .iter()
                .find(|metric| metric.key == net_impulse::KEY)
                .expect("the rule reports net impulse");
            let velocity = response
                .metrics
                .iter()
                .find(|metric| metric.key == net_impulse::VELOCITY_KEY)
                .expect("the rule reports takeoff velocity");
            assert_eq!(impulse.computed_by.as_deref(), Some(*id));
            assert_eq!(velocity.computed_by.as_deref(), Some(*id));
            let bound = response
                .bound_methods
                .iter()
                .find(|bound| bound.method_id == *id)
                .expect("the selected quadrature is on the record");
            let choices: BTreeMap<String, String> =
                bound.enumerated_choices().into_iter().collect();
            let expected_choices = BTreeMap::from([
                (
                    "integration_direction".to_string(),
                    "integration.direction.forward".to_string(),
                ),
                (
                    "integration_start".to_string(),
                    "integration.start.detected_onset".to_string(),
                ),
                (
                    "integration_anchor".to_string(),
                    "integration.anchor.single_point".to_string(),
                ),
            ]);
            assert_eq!(choices, expected_choices);
            assert!(expected_choices.keys().all(|name| {
                bound.parameter_sources.get(name)
                    == Some(&plateforce_core::provenance::ParameterSource::Assumed)
            }));
            for expected in [
                *id,
                "integration.direction.forward",
                "integration.start.detected_onset",
                "integration.anchor.single_point",
            ] {
                assert!(
                    impulse
                        .contributing_method_ids
                        .iter()
                        .any(|named| named == expected),
                    "{id} omitted {expected} from the impulse chain"
                );
                assert!(
                    velocity
                        .contributing_method_ids
                        .iter()
                        .any(|named| named == expected),
                    "{id} omitted {expected} from the velocity chain"
                );
            }
            results.push((
                *id,
                impulse.value.expect("a finite impulse"),
                velocity.value.expect("a finite velocity"),
            ));
        }

        let distinct: BTreeSet<u64> = results
            .iter()
            .map(|(_, value, _)| value.to_bits())
            .collect();
        assert_eq!(
            distinct.len(),
            results.len(),
            "the quadrature choices returned one impulse: {results:?}"
        );

        let default = run(&trial, &request(None)).expect("the default analysis runs");
        let default_impulse = default
            .metrics
            .iter()
            .find(|metric| metric.key == net_impulse::KEY)
            .and_then(|metric| metric.value)
            .expect("the default reports net impulse");
        let trapezoid = results
            .iter()
            .find(|(id, _, _)| *id == trapezoid::ID)
            .map(|(_, value, _)| *value)
            .expect("the registry includes trapezoid");
        assert!(
            (default_impulse - trapezoid).abs() < 1e-9,
            "the bound trapezoid returned {trapezoid} N s and the existing path returned {default_impulse} N s"
        );
        println!(
            "{} of {} registry quadrature rules ran on subject 01 trial 1: {results:?}",
            results.len(),
            ids.len()
        );
    }
}
