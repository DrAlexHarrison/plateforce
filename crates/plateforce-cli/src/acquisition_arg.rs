//! What the plate and its settings were, asked of the caller on the command line.
//!
//! No terminal surface asked for this. Both of them passed a literal `false` for
//! `acquisition_complete` and carried a comment saying no block reaches here, so every result
//! either has ever written fingerprints as incomplete and no caller could change that. R and
//! Python both ask, which left the block fillable only from a surface that needs a programming
//! language.
//!
//! One repeatable flag rather than five, in the grammar `--set`, `--choose` and `--place`
//! already use, so a caller who has written one writes this one. The member names come from
//! `Acquisition::MEMBERS`, so a member added to the block is a name this flag accepts without
//! being told about it, and one that is not a member is refused against the list rather than
//! read and dropped.

use plateforce_core::{Acquisition, MemberFault};

use crate::analyse::stated_twice;
use crate::exit::{Declined, Fault};

/// What `--acquisition` takes, in one string for the help and the refusals, so a flag whose
/// help describes something the parser does not accept cannot happen here.
pub(crate) const ACQUISITION_SHAPE: &str = "<member>=<value>";

/// The help this flag shows.
///
/// It names no other command as the place to look the members up. An earlier draft said
/// `plateforce capability` lists them, and it does not: its document carries `methods`,
/// `operations`, `output_formats`, `plateforce_version`, `refusal_codes` and `schema`, and
/// none of the five members appears anywhere in it. A help line sending a reader to a command
/// that cannot answer is the same defect as a flag that accepts a value and drops it.
///
/// The refusal is where the list comes from, and it reads the block itself, so what a caller
/// is shown cannot fall behind what the block holds.
pub(crate) const ACQUISITION_HELP: &str =
    "A fact about the capture, written <member>=<value>. Repeatable, and naming a member the block does not hold answers with the ones it does. A block short of any member fingerprints as incomplete rather than as matching";

/// The block a run states, or the empty block when it states nothing.
///
/// An empty block is returned rather than `None`, because a run that stated nothing and a run
/// that stated two of five are the same kind of thing to every reader downstream: incomplete,
/// with the members it is missing nameable. The caller decides whether an empty block is worth
/// carrying.
pub(crate) fn stated_acquisition(assignments: &[String]) -> Result<Acquisition, Declined> {
    let mut block = Acquisition::default();
    let mut as_written: Vec<(String, String)> = Vec::new();

    for assignment in assignments {
        let Some((member, written)) = assignment.split_once('=') else {
            return Err(Declined::line(
                Fault::Request,
                format!("--acquisition takes {ACQUISITION_SHAPE}, and '{assignment}' carries no ="),
            ));
        };
        let member = member.trim();
        let written = written.trim();

        if !Acquisition::MEMBERS.contains(&member) {
            return Err(not_a_member(member));
        }
        if written.is_empty() {
            return Err(Declined::line(
                Fault::Request,
                format!("--acquisition {member} was given no value"),
            ));
        }
        // A member stated twice is refused for the reason `--set` refuses one: the run of a
        // caller who wrote both would otherwise be byte-identical to the run of a caller who
        // wrote only the second, with nothing recorded anywhere saying so.
        if let Some((_, first)) = as_written.iter().find(|(name, _)| name == member) {
            return Err(stated_twice("--acquisition", member, first, written));
        }
        as_written.push((member.to_string(), written.to_string()));

        assign(&mut block, member, written)?;
    }

    Ok(block)
}

/// One stated member onto the block, in the words this surface says it in.
///
/// The block itself decides whether a name is a member and whether the text is the kind that
/// member holds, so a member added to the block reaches this flag without being told about it
/// and there is no arm here to forget.
fn assign(block: &mut Acquisition, member: &str, written: &str) -> Result<(), Declined> {
    block.set_member(member, written).map_err(|fault| match fault {
        MemberFault::Unknown => not_a_member(member),
        MemberFault::NotANumber => Declined::line(
            Fault::Request,
            format!("--acquisition {member} was given '{written}', which is not a number"),
        ),
    })
}

/// The refusal a caller learns the members from, said once so the two places that raise it
/// cannot come to name different sets.
fn not_a_member(member: &str) -> Declined {
    Declined::line(
        Fault::Request,
        format!(
            "--acquisition {member} names nothing the block holds, which has {}",
            Acquisition::MEMBERS.join(", ")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stated(assignments: &[&str]) -> Result<Acquisition, Declined> {
        stated_acquisition(
            &assignments
                .iter()
                .map(|written| (*written).to_string())
                .collect::<Vec<_>>(),
        )
    }

    /// The whole point of the flag: a caller who states every member gets a block that can be
    /// declared to match another lab's.
    #[test]
    fn every_member_stated_is_a_complete_block() {
        let block = stated(&[
            "filter_at_capture=none",
            "tare_state=tared_before_trial",
            "plate_natural_frequency_hz=400",
            "floor_surface=concrete",
            "firmware_version=2.4.1",
        ])
        .expect("every member is one the block holds");

        assert!(block.is_complete(), "{:?}", block.missing());
    }

    /// Stating nothing is the state every terminal run was in before this flag, and it is
    /// incomplete rather than absent, so the reason is nameable.
    #[test]
    fn stating_nothing_is_an_incomplete_block_that_names_what_is_missing() {
        let block = stated(&[]).expect("stating nothing is not a fault");

        assert!(!block.is_complete());
        assert_eq!(block.missing(), Acquisition::MEMBERS);
    }

    /// A block short of one member is incomplete, which is what stops it being declared a
    /// match. The interesting case sits between the two above and would be swallowed by a
    /// test that only asked about none and all.
    #[test]
    fn four_of_five_members_is_incomplete_and_names_the_fifth() {
        let block = stated(&[
            "filter_at_capture=none",
            "tare_state=tared_before_trial",
            "plate_natural_frequency_hz=400",
            "floor_surface=concrete",
        ])
        .expect("four members are four names the block holds");

        assert!(!block.is_complete());
        assert_eq!(block.missing(), vec!["firmware_version"]);
    }

    /// The refusal is where a caller learns the members, because the help sends them nowhere
    /// else, so it has to name every one rather than a representative few.
    #[test]
    fn a_member_the_block_does_not_hold_is_refused_against_the_whole_list() {
        let declined = stated(&["debounce_ms=50"]).expect_err("the block holds no debounce");
        let message = format!("{declined:?}");

        assert!(message.contains("debounce_ms"), "{message}");
        for member in Acquisition::MEMBERS {
            assert!(
                message.contains(member),
                "the refusal does not name {member}, so a caller reading it learns four of five: {message}"
            );
        }
    }

    /// The help describes this flag and sends the reader nowhere that cannot answer.
    ///
    /// An earlier draft said `plateforce capability` lists the members. It does not: that
    /// document carries `methods`, `operations`, `output_formats`, `plateforce_version`,
    /// `refusal_codes` and `schema`, and none of the five members appears in it. A help line
    /// pointing at a command that cannot answer is the same defect as a flag that accepts a
    /// value and drops it, and it shipped in the first draft of the fix for that defect.
    #[test]
    fn the_help_names_no_command_to_look_the_members_up_in() {
        assert!(
            !ACQUISITION_HELP.contains("plateforce "),
            "the help sends the reader to another command: {ACQUISITION_HELP}"
        );
        assert!(
            ACQUISITION_HELP.contains(ACQUISITION_SHAPE),
            "the help does not state the grammar the parser accepts: {ACQUISITION_HELP}"
        );
    }

    /// Refused rather than settled by position, for the reason `--set` refuses it: two runs
    /// that stated different things would otherwise be one run.
    #[test]
    fn one_member_given_two_values_is_refused() {
        let declined = stated(&["floor_surface=concrete", "floor_surface=sprung"])
            .expect_err("a member takes one value");
        let message = format!("{declined:?}");

        assert!(message.contains("concrete"), "{message}");
        assert!(message.contains("sprung"), "{message}");
    }

    #[test]
    fn a_frequency_that_is_not_a_number_is_refused() {
        let declined =
            stated(&["plate_natural_frequency_hz=stiff"]).expect_err("that is not a frequency");

        assert!(format!("{declined:?}").contains("not a number"));
    }

    #[test]
    fn a_line_carrying_no_equals_is_refused_with_the_grammar() {
        let declined = stated(&["floor_surface concrete"]).expect_err("that carries no =");

        assert!(format!("{declined:?}").contains(ACQUISITION_SHAPE));
    }

    /// The list this flag accepts is the block's own, so a member added upstream is accepted
    /// here without this file being edited. A second list written out here is the one that
    /// goes stale.
    #[test]
    fn every_member_the_block_declares_is_a_member_this_flag_stores() {
        for member in Acquisition::MEMBERS {
            let written = if *member == "plate_natural_frequency_hz" {
                "400"
            } else {
                "stated"
            };
            let block = stated(&[&format!("{member}={written}")])
                .unwrap_or_else(|_| panic!("{member} is a member the block declares"));

            assert_eq!(
                block.missing().len(),
                Acquisition::MEMBERS.len() - 1,
                "{member} was accepted and stored nothing"
            );
        }
    }
}
