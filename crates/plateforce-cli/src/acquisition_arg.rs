//! What the plate and its settings were, asked of the caller on the command line.
//!
//! One repeatable flag rather than five, in the grammar `--set`, `--choose` and `--place`
//! already use. The member names come from `Acquisition::MEMBERS`, so a member added to the
//! block is a name this flag accepts without being told about it, and one that is not a member
//! is refused against the list rather than read and dropped.

use plateforce_core::Acquisition;

use crate::analyse::stated_twice;
use crate::exit::{Declined, Fault};

/// What `--acquisition` takes, in one string for the help and the refusals, so the two cannot
/// describe different grammars.
pub(crate) const ACQUISITION_SHAPE: &str = "<member>=<value>";

/// The help this flag shows. It names no command as the place to look the members up, because
/// no other command carries them: the refusal reads the block itself and lists them.
pub(crate) const ACQUISITION_HELP: &str =
    "A fact about the capture, written <member>=<value>. Repeatable, and a block short of any member fingerprints as incomplete rather than as matching";

/// The block a run states, or the empty block when it states nothing.
///
/// An empty block rather than `None`, because a run that stated nothing and a run that stated
/// two of five are the same thing downstream: incomplete, with the missing members nameable.
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
            return Err(Declined::line(
                Fault::Request,
                format!(
                    "--acquisition {member} names nothing the block holds, which has {}",
                    Acquisition::MEMBERS.join(", ")
                ),
            ));
        }
        if written.is_empty() {
            return Err(Declined::line(
                Fault::Request,
                format!("--acquisition {member} was given no value"),
            ));
        }
        // Refused for the reason `--set` refuses a repeated name: the run of a caller who wrote
        // both would otherwise be byte-identical to the run of a caller who wrote one.
        if let Some((_, first)) = as_written.iter().find(|(name, _)| name == member) {
            return Err(stated_twice("--acquisition", member, first, written));
        }
        as_written.push((member.to_string(), written.to_string()));

        assign(&mut block, member, written)?;
    }

    Ok(block)
}

/// One stated member onto the block.
///
/// Matched against the same `MEMBERS` list the check above reads, so a member the block gains
/// and this arm does not is a compile-time hole rather than a silent drop: the final arm is
/// unreachable for every declared member and states what it means when it is reached.
fn assign(block: &mut Acquisition, member: &str, written: &str) -> Result<(), Declined> {
    match member {
        "filter_at_capture" => block.filter_at_capture = Some(written.to_string()),
        "tare_state" => block.tare_state = Some(written.to_string()),
        "plate_natural_frequency_hz" => {
            block.plate_natural_frequency_hz = Some(written.parse().map_err(|_| {
                Declined::line(
                    Fault::Request,
                    format!(
                        "--acquisition plate_natural_frequency_hz was given '{written}', which is not a number"
                    ),
                )
            })?)
        }
        "floor_surface" => block.floor_surface = Some(written.to_string()),
        "firmware_version" => block.firmware_version = Some(written.to_string()),
        // Reachable only when the block declares a member this function was not taught, which
        // is a build that can accept a value it cannot store.
        other => {
            return Err(Declined::line(
                Fault::Internal,
                format!("the acquisition block declares {other} and this surface cannot store it"),
            ))
        }
    }
    Ok(())
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

    /// A caller who states every member gets a block that can be declared to match another
    /// lab's.
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

    /// Stating nothing is incomplete rather than absent, so the reason is nameable.
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

    /// The help sends the reader to no command that cannot answer. `plateforce capability`
    /// carries `methods`, `operations`, `output_formats`, `plateforce_version`,
    /// `refusal_codes` and `schema`, and none of the five members appears in it.
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
