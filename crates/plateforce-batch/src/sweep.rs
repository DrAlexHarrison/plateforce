//! Which step of the analysis a comparison varies.
//!
//! The axis is read off the rules the caller named: every method id in this build is filed
//! under exactly one construct, so naming a rule to compare against already names the step
//! being compared.

use plateforce_analysis::AnalysisRequest;
use plateforce_core::Refusal;

/// A comparison that cannot be set up, before any trial is read.
#[derive(Debug, Clone, PartialEq)]
pub enum SweepRefusal {
    /// A name no rule in this build answers to.
    UnknownMethod(Box<Refusal>),
    /// Rules from two different steps, which is two comparisons rather than one.
    MixedSteps {
        first_construct: String,
        first_id: String,
        second_construct: String,
        second_id: String,
    },
    /// The step is one this run never bound, so there is no rule to compare against.
    NothingBound { construct: String, slot: String },
}

impl std::fmt::Display for SweepRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SweepRefusal::UnknownMethod(refusal) => write!(formatter, "{}", refusal.message()),
            SweepRefusal::MixedSteps {
                first_construct,
                first_id,
                second_construct,
                second_id,
            } => write!(
                formatter,
                "a comparison varies one step, and {first_id} is a rule for {first_construct} while {second_id} is a rule for {second_construct}, so this line asks for two"
            ),
            SweepRefusal::NothingBound { construct, slot } => write!(
                formatter,
                "a comparison varies a step this run bound, and nothing is bound for {construct}, so --{slot} or --derive {construct}=<method> names the rule the others are compared against"
            ),
        }
    }
}

/// The step a comparison varies, and the rules it varies across.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    /// The word `plateforce_analysis::spread` reaches this step by: `weighing`, `onset` and
    /// `takeoff` for the three the request names on their own fields, and the construct id for
    /// every rule reached through `derived`. Read off the binding row rather than decided here.
    pub slot: String,
    pub construct: String,
    /// The rule the run bound first, then the ones it is compared against, in the order the
    /// caller wrote them. A caller reading the output finds their own line back.
    pub method_ids: Vec<String>,
}

/// The construct a rule is filed under, and the word a sweep reaches its step by.
///
/// Every id in this build is filed under exactly one construct, measured across the binding
/// table rather than assumed, so a rule names its step without a second flag to say which.
fn step_of(method_id: &str) -> Option<(&'static str, &'static str)> {
    plateforce_analysis::BINDINGS
        .iter()
        .find(|binding| binding.id == method_id)
        .map(|binding| (binding.slot, binding.construct))
}

/// Every rule this build runs, for a refusal that has to list the alternatives.
fn every_method_id() -> Vec<String> {
    plateforce_analysis::BINDINGS
        .iter()
        .map(|binding| binding.id.to_string())
        .collect()
}

/// The rule this request bound for a construct, or `None` where it bound none.
///
/// The three landmarks arrive on their own fields and everything else arrives keyed by
/// construct, so one reader covers both rather than each caller branching on which kind it has.
pub fn bound_for(request: &AnalysisRequest, construct: &str) -> Option<String> {
    let named = match construct {
        plateforce_analysis::WEIGHING_CONSTRUCT => &request.weighing.method_id,
        plateforce_analysis::ONSET_CONSTRUCT => &request.onset.method_id,
        plateforce_analysis::TAKEOFF_CONSTRUCT => &request.takeoff.method_id,
        other => {
            return request
                .derived
                .get(other)
                .map(|choice| choice.method_id.clone())
                .filter(|id| !id.is_empty())
        }
    };
    Some(named.clone()).filter(|id| !id.is_empty())
}

/// The axis a comparison varies, read off the rules it was asked to compare.
///
/// `against` carries the rules to compare, and the bound rule for their construct opens the
/// list, because a comparison is between what the run does and what it could have done.
pub fn axis_over(request: &AnalysisRequest, against: &[String]) -> Result<Axis, SweepRefusal> {
    let mut resolved: Option<(&str, &str, &str)> = None;
    for id in against {
        let Some((slot, construct)) = step_of(id) else {
            return Err(SweepRefusal::UnknownMethod(Box::new(
                Refusal::method_not_implemented(id, "this comparison", every_method_id()),
            )));
        };
        match resolved {
            None => resolved = Some((slot, construct, id)),
            // Two steps in one line is two comparisons, and running either would be picking
            // one of the caller's two questions without saying which.
            Some((_, first_construct, first_id)) if first_construct != construct => {
                return Err(SweepRefusal::MixedSteps {
                    first_construct: first_construct.to_string(),
                    first_id: first_id.to_string(),
                    second_construct: construct.to_string(),
                    second_id: id.clone(),
                })
            }
            Some(_) => {}
        }
    }
    let (slot, construct, _) = resolved.expect("the caller checked that --against is not empty");

    let Some(bound) = bound_for(request, construct) else {
        return Err(SweepRefusal::NothingBound {
            construct: construct.to_string(),
            slot: slot.to_string(),
        });
    };

    // The bound rule first, then the ones written against it, without repeating a rule a
    // caller named twice on one line: a variant run twice is a pair with itself, and it would
    // report a spread of zero between a rule and a copy of it.
    let mut method_ids = vec![bound];
    for id in against {
        if !method_ids.contains(id) {
            method_ids.push(id.clone());
        }
    }

    Ok(Axis {
        slot: slot.to_string(),
        construct: construct.to_string(),
        method_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plateforce_analysis::MethodChoice;

    fn request() -> AnalysisRequest {
        let mut request = AnalysisRequest {
            weighing: plateforce_analysis::WeighingChoice {
                method_id: "bwepoch.fixed_window".to_string(),
                ..Default::default()
            },
            onset: MethodChoice {
                method_id: "onset.threshold.noise_relative".to_string(),
                ..Default::default()
            },
            takeoff: MethodChoice {
                method_id: "takeoff.threshold.absolute_force".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        request.derived.insert(
            "jump_height.takeoff_frame".to_string(),
            MethodChoice {
                method_id: "jumpheight.takeoff.impulse_momentum".to_string(),
                ..Default::default()
            },
        );
        request
    }

    /// The behaviour the hardcoded slot gave, reached now by naming an onset rule rather than
    /// by being the only thing the surface could do.
    #[test]
    fn an_onset_rule_names_the_onset_step_and_the_bound_rule_opens_the_list() {
        let axis = axis_over(&request(), &["onset.threshold.absolute_force".to_string()])
            .expect("an onset rule names its step");
        assert_eq!(axis.slot, "onset");
        assert_eq!(axis.construct, "movement_onset");
        assert_eq!(
            axis.method_ids,
            vec![
                "onset.threshold.noise_relative".to_string(),
                "onset.threshold.absolute_force".to_string()
            ]
        );
    }

    /// The whole point: a construct computed from the landmarks is an axis, and the word the
    /// sweep reaches it by is the construct id rather than a slot name.
    #[test]
    fn a_derived_rule_names_its_construct_as_the_axis() {
        let axis = axis_over(&request(), &["jumpheight.takeoff.work_energy".to_string()])
            .expect("a derived rule names its step");
        assert_eq!(axis.slot, "jump_height.takeoff_frame");
        assert_eq!(axis.construct, "jump_height.takeoff_frame");
        assert_eq!(
            axis.method_ids.first().map(String::as_str),
            Some("jumpheight.takeoff.impulse_momentum")
        );
    }

    /// Two steps on one line is two comparisons, and the refusal names both rather than
    /// silently running the first.
    #[test]
    fn rules_from_two_steps_are_refused_naming_both() {
        let refusal = axis_over(
            &request(),
            &[
                "onset.threshold.absolute_force".to_string(),
                "takeoff.threshold.longest_run".to_string(),
            ],
        )
        .expect_err("two steps is two comparisons");
        let said = refusal.to_string();
        assert!(said.contains("movement_onset"), "{said}");
        assert!(said.contains("takeoff"), "{said}");
    }

    /// A step nothing was bound for has no rule for the others to be compared against, and the
    /// refusal names the flag that binds it rather than the flag that is missing.
    #[test]
    fn a_step_this_run_never_bound_is_refused_naming_how_to_bind_it() {
        let refusal = axis_over(&request(), &["force.peak.gross".to_string()])
            .expect_err("nothing is bound for peak force");
        let said = refusal.to_string();
        assert!(said.contains("peak_force"), "{said}");
        assert!(said.contains("--derive"), "{said}");
    }

    /// A rule written twice is one variant. Two identical variants pair a rule with a copy of
    /// itself and report a spread of zero that no reader asked for.
    #[test]
    fn a_rule_named_twice_is_one_variant() {
        let axis = axis_over(
            &request(),
            &[
                "onset.threshold.absolute_force".to_string(),
                "onset.threshold.absolute_force".to_string(),
            ],
        )
        .expect("a repeat is not a second rule");
        assert_eq!(axis.method_ids.len(), 2, "{:?}", axis.method_ids);
    }

    /// The bound rule named again is still one variant, which is the case a naive dedup of the
    /// against list alone would miss.
    #[test]
    fn the_bound_rule_written_against_itself_is_one_variant() {
        let axis = axis_over(&request(), &["onset.threshold.noise_relative".to_string()])
            .expect("naming the bound rule is not an error");
        assert_eq!(
            axis.method_ids,
            vec!["onset.threshold.noise_relative".to_string()]
        );
    }

    #[test]
    fn a_name_no_rule_answers_to_is_refused_with_the_ones_that_do() {
        let refusal = axis_over(&request(), &["not.a.rule".to_string()]).expect_err("no such rule");
        let SweepRefusal::UnknownMethod(recorded) = refusal else {
            panic!("an unknown name is not a mixed line")
        };
        assert!(
            recorded.available.len() > 40,
            "{}",
            recorded.available.len()
        );
    }
}
