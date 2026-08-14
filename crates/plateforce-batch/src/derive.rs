//! Which rule a run binds to a construct computed from the landmarks.
//!
//! The landmark rules arrive on named fields and every other rule arrives keyed by construct,
//! so the second kind has no flag of its own to be validated by. One predicate answers both
//! halves of the question, the construct and the id filed under it, for the command line
//! reading assignments and for the engine checking a request some other caller built.

use std::collections::BTreeMap;

use plateforce_core::Refusal;

/// What the shape of an assignment is written as, wherever a surface prints it.
pub const SHAPE: &str = "<construct>=<method>";

/// A binding a run cannot make, either because the line could not be read or because the
/// names in it are not ones this build runs.
#[derive(Debug, Clone, PartialEq)]
pub enum DeriveRefusal {
    /// The line carries no `=`, so it reaches no rule and carries no published code.
    Malformed { flag: String, assignment: String },
    /// A name this build knows how to answer, with the alternatives the refusal carries.
    Recorded(Box<Refusal>),
}

impl std::fmt::Display for DeriveRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeriveRefusal::Malformed { flag, assignment } => {
                write!(
                    formatter,
                    "{flag} takes {SHAPE}, and '{assignment}' carries no ="
                )
            }
            DeriveRefusal::Recorded(refusal) => write!(formatter, "{}", refusal.message()),
        }
    }
}

/// One `<construct>=<method>` pair, read and checked.
///
/// Both halves are checked, because either alone lets through a request the engine would have
/// to refuse per trial or, worse, skip. A construct this build runs no rule for is a different
/// fault from an id filed under another construct, and they list different alternatives.
pub fn choice(flag: &str, assignment: &str) -> Result<(String, String), DeriveRefusal> {
    let Some((construct, method_id)) = assignment.split_once('=') else {
        return Err(DeriveRefusal::Malformed {
            flag: flag.to_string(),
            assignment: assignment.to_string(),
        });
    };
    accepts(construct, method_id).map_err(DeriveRefusal::Recorded)?;
    Ok((construct.to_string(), method_id.to_string()))
}

/// Whether this build runs `method_id` for `construct`, as the record rather than as a bool.
///
/// The one home for the question. A surface that answered it from its own copy of the
/// construct list would report a rule added to the binding table as absent until it was
/// edited too.
pub fn accepts(construct: &str, method_id: &str) -> Result<(), Box<Refusal>> {
    let runs = plateforce_analysis::binding::derived_constructs();
    if !runs.contains(&construct) {
        return Err(Box::new(Refusal::construct_not_on_the_path(
            construct,
            runs.into_iter().map(str::to_string).collect(),
        )));
    }
    let available: Vec<String> = plateforce_analysis::binding::bindings_for_construct(construct)
        .map(|binding| binding.id.to_string())
        .collect();
    if !available.iter().any(|id| id == method_id) {
        return Err(Box::new(Refusal::method_not_implemented(
            method_id, construct, available,
        )));
    }
    Ok(())
}

/// Every quantity key the rules a request bound will report, with the unit each is in.
///
/// Read off the binding rows rather than off what the rules produced. A rule that declines on
/// every trial in a folder produces no metric anywhere, so a table built from the values alone
/// loses the column the caller asked for, and a reader meets a run that answered a question
/// they did not ask instead of a column of blanks with a refusal beside each one.
pub fn declared_quantities(
    derived: &BTreeMap<String, plateforce_analysis::MethodChoice>,
) -> Vec<(&'static str, &'static str)> {
    let mut declared = Vec::new();
    for (construct, choice) in derived {
        for binding in plateforce_analysis::binding::bindings_for_construct(construct) {
            if binding.id != choice.method_id {
                continue;
            }
            for quantity in binding.quantities {
                declared.push((quantity.key, quantity.unit));
            }
        }
    }
    declared
}

/// The result columns one rule reports, for a refusal that names it.
///
/// Read off the binding row rather than off what the trial produced, because a rule that
/// declined produced no metric: the columns it would have filled are knowable only from the
/// table that declares them. Empty where no row answers for the name, which is every refusal
/// that is not a rule declining, a file the identity could not name among them.
///
/// Matched on the recorded name as well as the selected one. A composed id records under the
/// entry the registry already spells, and the refusal carries the recorded name, so a lookup
/// on `id` alone would report a composed rule's columns as none.
pub fn quantities_of_rule(method_id: &str) -> Vec<&'static str> {
    if method_id.is_empty() {
        return Vec::new();
    }
    let mut keys: Vec<&'static str> = plateforce_analysis::binding::BINDINGS
        .iter()
        .filter(|binding| binding.id == method_id || binding.records_under == Some(method_id))
        .flat_map(|binding| binding.quantities.iter().map(|quantity| quantity.key))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mapping a blank cell is joined to its reason by. Held against a rule whose decline
    /// is the one this crate meets on five of six shipped fixtures.
    #[test]
    fn a_declining_rule_names_the_columns_its_refusal_accounts_for() {
        assert_eq!(
            quantities_of_rule("flight_time.takeoff_to_touchdown"),
            vec!["flight_time_seconds"]
        );
        assert_eq!(
            quantities_of_rule("jumpheight.takeoff.flight_time"),
            vec!["jump_height_from_flight_time_meters"]
        );
    }

    /// A refusal no rule row answers for keeps an empty cell rather than borrowing the columns
    /// of whichever row a loose match found.
    ///
    /// The prefix is the case that separates an exact match from a loose one, and neither name
    /// above reaches it: `jumpheight.takeoff` is the front of two rules that report different
    /// heights, so a lookup matching on it would point a reader at a blank cell the refusal has
    /// nothing to do with. Without this the two names above pass under a prefix match as
    /// readily as under an exact one.
    #[test]
    fn a_refusal_naming_no_rule_claims_no_column() {
        assert!(quantities_of_rule("").is_empty());
        assert!(quantities_of_rule("not.a.rule").is_empty());
        assert!(
            quantities_of_rule("jumpheight.takeoff").is_empty(),
            "{:?}",
            quantities_of_rule("jumpheight.takeoff")
        );
        assert!(quantities_of_rule("flight_time").is_empty());
    }

    #[test]
    fn a_construct_this_build_runs_no_rule_for_is_refused_with_the_ones_it_does() {
        let refusal = choice("--derive", "not_a_construct=x").expect_err("no such construct");
        let DeriveRefusal::Recorded(recorded) = refusal else {
            panic!("a name is not a malformed line: {refusal}")
        };
        assert!(
            recorded.available.contains(&"phase_model".to_string()),
            "{:?}",
            recorded.available
        );
    }

    /// An id that is a rule, filed under a construct other than the one it was named for.
    /// Checking only that the id exists somewhere would bind an onset rule to peak force.
    #[test]
    fn an_id_filed_under_another_construct_is_refused_with_the_ones_filed_under_this_one() {
        let refusal = choice("--derive", "peak_force=onset.threshold.absolute_force")
            .expect_err("wrong home");
        let DeriveRefusal::Recorded(recorded) = refusal else {
            panic!("a name is not a malformed line: {refusal}")
        };
        assert!(
            recorded
                .available
                .iter()
                .all(|id| id.starts_with("force.peak.")),
            "{:?}",
            recorded.available
        );
    }

    #[test]
    fn a_line_carrying_no_equals_is_a_fault_in_the_line_rather_than_a_recorded_refusal() {
        let refusal = choice("--derive", "peak_force").expect_err("no assignment");
        assert_eq!(
            refusal,
            DeriveRefusal::Malformed {
                flag: "--derive".to_string(),
                assignment: "peak_force".to_string(),
            }
        );
    }

    #[test]
    fn the_quantities_a_bound_rule_declares_are_read_off_its_binding_row() {
        let derived: BTreeMap<String, plateforce_analysis::MethodChoice> = [(
            "net_peak_force".to_string(),
            plateforce_analysis::MethodChoice {
                method_id: "force.peak.net".to_string(),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect();
        let declared = declared_quantities(&derived);
        assert!(
            !declared.is_empty(),
            "a bound rule declares what it reports"
        );
        for (key, unit) in &declared {
            assert!(!key.is_empty() && !unit.is_empty(), "{key} is in {unit}");
        }
    }
}
