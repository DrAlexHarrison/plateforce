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
    ///
    /// `binds_it` is the one way to bind this construct rather than every way, and nothing at
    /// all where a request reaches the construct by neither route. A sentence offering both
    /// ways offered a flag that does not exist to one caller and an assignment that is refused
    /// to the other, and which of the two worked inverted between them.
    NothingBound {
        construct: String,
        binds_it: Option<String>,
    },
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
            SweepRefusal::NothingBound {
                construct,
                binds_it,
            } => {
                write!(
                    formatter,
                    "a comparison varies a step this run bound, and nothing is bound for {construct}"
                )?;
                match binds_it {
                    Some(flag) => write!(
                        formatter,
                        ", so {flag} names the rule the others are compared against"
                    ),
                    None => Ok(()),
                }
            }
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

/// The construct a word names when the word is a step rather than a rule.
///
/// Both spellings, because the two commands that sweep take both: `--slot` accepts the short
/// word and the construct the record prints, so a caller moving between them writes the same
/// vocabulary at each. A step written where a rule goes is the ordinary slip between them.
fn step_named(word: &str) -> Option<&'static str> {
    plateforce_analysis::binding::construct_for_slot(word).or_else(|| {
        plateforce_analysis::BINDINGS
            .iter()
            .find(|binding| binding.construct == word)
            .map(|binding| binding.construct)
    })
}

/// The one way to bind a construct, named as a caller would write it, or nothing where a
/// request reaches this construct by neither route.
///
/// Read off the dispatch the binding table declares rather than from a list here. A landmark
/// arrives on its own named field and everything the request reaches through `derived` arrives
/// as an assignment, and those are the two a caller writes.
///
/// `None` rather than a guess for anything else. A remedy clause naming a word no surface
/// takes reads as an instruction and sends the caller straight to a second refusal, which is
/// what this sentence did for every construct it was written for.
pub fn binds(construct: &str) -> Option<String> {
    plateforce_analysis::BINDINGS
        .iter()
        .find(|binding| binding.construct == construct)
        .and_then(|binding| match binding.dispatch {
            plateforce_analysis::binding::Dispatch::Spine => Some(format!("--{}", binding.slot)),
            plateforce_analysis::binding::Dispatch::Derived(_) => {
                Some(format!("--derive {construct}=<method>"))
            }
            _ => None,
        })
}

/// A name written in a comparison that answers to no rule.
///
/// A step written where a rule goes names its own rules, because that caller is one word away
/// from the line they meant. A name that is neither gets every rule this build runs, under a
/// sentence that does not attribute them to a step nobody named.
fn no_rule_answers_to(written: &str) -> Refusal {
    match step_named(written) {
        Some(construct) => Refusal::method_not_implemented(
            written,
            construct,
            plateforce_analysis::binding::bindings_for_construct(construct)
                .map(|binding| binding.id.to_string())
                .collect(),
        ),
        None => Refusal::name_answers_to_no_rule(written, every_method_id()),
    }
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
            return Err(SweepRefusal::UnknownMethod(Box::new(no_rule_answers_to(
                id,
            ))));
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
            binds_it: binds(construct),
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
    ///
    /// The one way in, not both. `--peak_force` is a flag no surface has, and a sentence
    /// offering it sends the caller to a second refusal.
    #[test]
    fn a_step_this_run_never_bound_is_refused_naming_how_to_bind_it() {
        let refusal = axis_over(&request(), &["force.peak.gross".to_string()])
            .expect_err("nothing is bound for peak force");
        let said = refusal.to_string();
        assert!(said.contains("--derive peak_force=<method>"), "{said}");
        assert!(!said.contains("--peak_force"), "{said}");
    }

    /// The same sentence for the other kind of construct, and the offer inverts: a landmark is
    /// bound by its own flag, and `--derive` refuses its construct by name.
    #[test]
    fn a_landmark_nothing_was_bound_for_is_refused_naming_its_own_flag() {
        let mut request = request();
        request.onset.method_id.clear();
        let refusal = axis_over(&request, &["onset.threshold.absolute_force".to_string()])
            .expect_err("nothing is bound for movement onset");
        let said = refusal.to_string();
        assert!(said.contains("--onset"), "{said}");
        assert!(!said.contains("--derive"), "{said}");
        // What the offered alternative would have met. A sentence may not send a caller to a
        // flag that refuses the name the sentence handed them.
        assert!(
            crate::derive::accepts("movement_onset", "onset.threshold.absolute_force").is_err(),
            "--derive takes movement_onset, so the offer above was sound after all"
        );
    }

    /// A step written where a rule goes is the ordinary slip between the two sweeping
    /// commands, `--slot takeoff` and `--against <a takeoff rule>`. It gets that step's rules,
    /// not every rule in the build under a step nobody named.
    #[test]
    fn a_step_written_where_a_rule_goes_is_refused_with_that_step_s_rules() {
        for word in ["takeoff", "movement_onset"] {
            let refusal =
                axis_over(&request(), &[word.to_string()]).expect_err("a step is not a rule");
            let SweepRefusal::UnknownMethod(recorded) = refusal else {
                panic!("a step word is not a mixed line: {word}")
            };
            let said = recorded.message();
            assert!(!said.contains("this comparison"), "{said}");
            let prefix = if word == "takeoff" {
                "takeoff."
            } else {
                "onset."
            };
            assert!(
                !recorded.available.is_empty()
                    && recorded.available.iter().all(|id| id.starts_with(prefix)),
                "{word}: {:?}",
                recorded.available
            );
        }
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

    /// A name that is neither a rule nor a step gets every rule this build runs, under a
    /// sentence that does not attribute them to a step.
    ///
    /// The count is the binding table's own rather than a number written here, so a rule added
    /// to the table is offered without an edit and the denominator in the sentence stays true.
    #[test]
    fn a_name_no_rule_answers_to_is_refused_with_the_ones_that_do() {
        let refusal = axis_over(&request(), &["not.a.rule".to_string()]).expect_err("no such rule");
        let SweepRefusal::UnknownMethod(recorded) = refusal else {
            panic!("an unknown name is not a mixed line")
        };
        assert_eq!(
            recorded.available.len(),
            plateforce_analysis::BINDINGS.len()
        );
        let said = recorded.message();
        // The sentence this replaced read "was passed as the this comparison method, and the
        // rules for that step are", naming a step nobody wrote and filing every rule under it.
        assert!(said.contains("answers to no rule"), "{said}");
        assert!(!said.contains("that step"), "{said}");
        assert!(
            said.contains(&format!("{} rules", plateforce_analysis::BINDINGS.len())),
            "{said}"
        );
    }
}
