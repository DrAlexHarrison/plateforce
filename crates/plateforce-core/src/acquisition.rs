//! What the plate and its settings were, which no reanalysis recovers.
//!
//! `sample_rate_hz` is the sixth member of the fingerprint's acquisition block and is carried
//! by the `Trial`, so it is not repeated here. A block that cannot be filled fingerprints as
//! incomplete rather than as matching: the most consequential setting in one open tool is a
//! contact debounce living in firmware, and knowing the trace does not recover it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Why text a caller wrote did not reach the block. The words a reader meets are the
/// surface's, and which of the two happened is decided here, so a terminal and a browser tab
/// cannot come to different answers about whether a name is a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberFault {
    /// The block declares no member of this name.
    Unknown,
    /// The member holds a number and the text is not one.
    NotANumber,
}

/// How one member reads the text a caller wrote for it.
///
/// Implemented per kind rather than matched per member, so a member added to the block below
/// is stored by every surface without any of them being told about it.
pub trait MemberValue: Sized {
    fn read(written: &str) -> Result<Self, MemberFault>;
}

impl MemberValue for String {
    fn read(written: &str) -> Result<Self, MemberFault> {
        Ok(written.to_string())
    }
}

impl MemberValue for f64 {
    fn read(written: &str) -> Result<Self, MemberFault> {
        written.parse().map_err(|_| MemberFault::NotANumber)
    }
}

/// Declares the block's members once, so the list a reader is told to go and find cannot
/// fall behind the members the block holds.
macro_rules! acquisition_block {
    ($( $(#[$note:meta])* $member:ident : $kind:ty ),+ $(,)?) => {
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        pub struct Acquisition {
            $( $(#[$note])* pub $member: Option<$kind>, )+
        }

        impl Acquisition {
            /// Every member this block declares.
            pub const MEMBERS: &'static [&'static str] = &[ $( stringify!($member), )+ ];

            /// One member set from the text a caller wrote.
            ///
            /// The arms are generated from the declaration, so there is no arm to forget and
            /// no build that accepts a value it cannot store.
            pub fn set_member(&mut self, member: &str, written: &str) -> Result<(), MemberFault> {
                match member {
                    $( stringify!($member) => {
                        self.$member = Some(<$kind as MemberValue>::read(written)?);
                    } )+
                    _ => return Err(MemberFault::Unknown),
                }
                Ok(())
            }

            /// This block laid over a saved plate's, and what each stated member displaced.
            ///
            /// A member stated beside a saved plate wins, because stating one is a caller
            /// producing evidence and reading a saved plate is not. What it displaced is
            /// returned rather than dropped, so a record can carry both numbers.
            pub fn over(&self, saved: &Acquisition) -> (Acquisition, BTreeMap<String, String>) {
                let mut laid = saved.clone();
                let mut displaced = BTreeMap::new();
                $(
                    if let Some(stated) = self.$member.as_ref() {
                        if let Some(was) = saved.$member.as_ref() {
                            if was.to_string() != stated.to_string() {
                                displaced.insert(
                                    stringify!($member).to_string(),
                                    was.to_string(),
                                );
                            }
                        }
                        laid.$member = Some(stated.clone());
                    }
                )+
                (laid, displaced)
            }

            /// The members somebody stated, as the text they stated, in declaration order.
            /// An absent member is not here at all, which is what tells it apart from one
            /// stated as an empty string.
            pub fn stated_members(&self) -> Vec<(&'static str, String)> {
                let mut stated = Vec::new();
                $( if let Some(value) = self.$member.as_ref() {
                    stated.push((stringify!($member), value.to_string()));
                } )+
                stated
            }

            /// True only when every member is present. Anything less and results from this
            /// trial carry `acquisition_complete = false`.
            pub fn is_complete(&self) -> bool {
                $( self.$member.is_some() && )+ true
            }

            /// The members still missing, so a reader can see what to go and find.
            pub fn missing(&self) -> Vec<&'static str> {
                let mut missing = Vec::new();
                $( if self.$member.is_none() { missing.push(stringify!($member)); } )+
                missing
            }

            /// Every member as the text a fingerprint is taken over, in declaration order.
            /// An absent member contributes an empty string.
            pub fn members_as_text(&self) -> Vec<(&'static str, String)> {
                vec![ $( (
                    stringify!($member),
                    self.$member.as_ref().map(|value| value.to_string()).unwrap_or_default(),
                ), )+ ]
            }
        }
    };
}

acquisition_block! {
    /// The filter the plate applied before the trace was written, which no later filtering
    /// can undo.
    filter_at_capture: String,
    tare_state: String,
    plate_natural_frequency_hz: f64,
    floor_surface: String,
    firmware_version: String,
}

/// The saved plate a run's block was filled from, recorded beside the members rather than in
/// place of them.
///
/// A record carries the block itself, so nothing here is what the run was computed under: a
/// reader who ignores this record entirely still holds every member. What it adds is which
/// saved plate the members were typed into and which revision of it was read, so two results
/// taken off one name after somebody edited it differ visibly rather than silently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlateProfileAttribution {
    /// The name the caller wrote, as the profile is filed under it.
    pub name: String,
    /// Digest over the members the profile held when it was read. A local nickname is not a
    /// fact about the capture, so neither this nor the name above is fingerprint material:
    /// two labs whose plates are configured alike match whatever they call them.
    pub revision: String,
    /// Members the profile states that the caller replaced on the same line, as the profile
    /// states them. What ran is in the block, and this is what it displaced, so a reader sees
    /// both numbers rather than having to go and read the profile.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub superseded_members: BTreeMap<String, String>,
}

impl PlateProfileAttribution {
    /// The revision of a saved plate, taken over the members it holds and nothing else.
    ///
    /// Deterministic across machines on purpose: two labs that recorded the same five answers
    /// hold one revision, and an edit to any member is a different one.
    pub fn revision_of(members: &Acquisition) -> String {
        plateforce_registry::content_digest(
            members
                .stated_members()
                .iter()
                .map(|(member, value)| (*member, value.as_str()))
                .collect::<Vec<_>>(),
        )
    }
}

/// What the plate was, and where the answers came from, as one surface hands them to a record.
///
/// One record rather than two arguments, for the reason `RegistryStamp` is one: the two travel
/// together to every record that carries either, and a surface that passed the block and
/// dropped the attribution would publish a result whose members nobody can trace to the saved
/// plate they were read from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Capture {
    pub acquisition: Acquisition,
    pub plate_profile: Option<PlateProfileAttribution>,
}

impl Capture {
    /// A block the caller stated on the line, with no saved plate behind it.
    pub fn stated(acquisition: Acquisition) -> Self {
        Self {
            acquisition,
            plate_profile: None,
        }
    }
}

impl From<Acquisition> for Capture {
    fn from(acquisition: Acquisition) -> Self {
        Self::stated(acquisition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> Acquisition {
        Acquisition {
            filter_at_capture: Some("none".to_string()),
            tare_state: Some("tared_before_trial".to_string()),
            plate_natural_frequency_hz: Some(400.0),
            floor_surface: Some("concrete".to_string()),
            firmware_version: Some("2.4.1".to_string()),
        }
    }

    #[test]
    fn a_block_with_every_member_is_complete() {
        assert!(filled().is_complete());
        assert!(filled().missing().is_empty());
    }

    #[test]
    fn a_block_missing_one_member_names_that_member() {
        let mut block = filled();
        block.firmware_version = None;
        assert!(!block.is_complete());
        assert_eq!(block.missing(), ["firmware_version"]);
    }

    #[test]
    fn an_unfilled_block_names_every_member_it_declares() {
        let block = Acquisition::default();
        assert!(!block.is_complete());
        assert_eq!(block.missing(), Acquisition::MEMBERS);
    }

    /// The arms are generated, so this asks the property rather than a list: every member the
    /// block declares is one that text can be written into. A hand-written match had an arm to
    /// forget, and a build that accepts a value it cannot store looks exactly like one that
    /// stored it.
    #[test]
    fn every_member_the_block_declares_can_be_set_from_text() {
        for member in Acquisition::MEMBERS {
            let mut block = Acquisition::default();
            let written = if *member == "plate_natural_frequency_hz" {
                "400"
            } else {
                "stated"
            };
            block
                .set_member(member, written)
                .unwrap_or_else(|_| panic!("{member} is a member the block declares"));
            assert_eq!(
                block.missing().len(),
                Acquisition::MEMBERS.len() - 1,
                "{member} was accepted and stored nothing"
            );
        }
    }

    #[test]
    fn a_name_the_block_does_not_hold_and_a_value_of_the_wrong_kind_are_told_apart() {
        let mut block = Acquisition::default();
        assert_eq!(block.set_member("debounce_ms", "50"), Err(MemberFault::Unknown));
        assert_eq!(
            block.set_member("plate_natural_frequency_hz", "stiff"),
            Err(MemberFault::NotANumber)
        );
    }

    /// A member nobody stated is absent from this list rather than present as an empty
    /// string, which is what makes the revision below a digest of what was answered.
    #[test]
    fn the_stated_members_are_the_ones_somebody_answered() {
        let mut block = Acquisition::default();
        block.set_member("floor_surface", "concrete").expect("a member");
        assert_eq!(block.stated_members(), vec![("floor_surface", "concrete".to_string())]);
        assert_eq!(Acquisition::default().stated_members(), Vec::new());
    }

    /// A member stated beside a saved plate wins, and the saved answer is returned rather
    /// than dropped. The second half is the load-bearing one: an overlay that returned only
    /// the winning value would leave a record unable to say a saved answer had been replaced.
    #[test]
    fn a_stated_member_displaces_the_saved_one_and_says_what_it_displaced() {
        let mut stated = Acquisition::default();
        stated.set_member("firmware_version", "2.4.2").expect("a member");

        let (laid, displaced) = stated.over(&filled());

        assert_eq!(laid.firmware_version.as_deref(), Some("2.4.2"));
        assert_eq!(laid.floor_surface.as_deref(), Some("concrete"));
        assert_eq!(
            displaced,
            BTreeMap::from([("firmware_version".to_string(), "2.4.1".to_string())])
        );
    }

    /// Stating the answer the saved plate already holds displaces nothing, so a record does
    /// not report a replacement that changed no number.
    #[test]
    fn restating_the_saved_answer_displaces_nothing() {
        let mut stated = Acquisition::default();
        stated.set_member("plate_natural_frequency_hz", "400").expect("a member");

        let (laid, displaced) = stated.over(&filled());

        assert_eq!(laid.plate_natural_frequency_hz, Some(400.0));
        assert!(displaced.is_empty(), "{displaced:?}");
    }

    /// A member the saved plate is silent about is filled by the caller and displaces
    /// nothing, which is how a profile short of a member is completed on the line.
    #[test]
    fn a_member_the_saved_plate_is_silent_about_is_filled_rather_than_displaced() {
        let mut saved = filled();
        saved.firmware_version = None;
        let mut stated = Acquisition::default();
        stated.set_member("firmware_version", "2.4.1").expect("a member");

        let (laid, displaced) = stated.over(&saved);

        assert!(laid.is_complete(), "{:?}", laid.missing());
        assert!(displaced.is_empty(), "{displaced:?}");
    }

    /// The pair that makes an edited profile visible: the same answers are one revision, and
    /// one changed answer is another. The second half is the one that does the work, and a
    /// digest over the name rather than the members would pass the first and fail this.
    #[test]
    fn a_revision_follows_the_members_and_a_changed_member_is_a_different_one() {
        let first = filled();
        let mut edited = filled();
        edited.firmware_version = Some("2.4.2".to_string());

        assert_eq!(
            PlateProfileAttribution::revision_of(&first),
            PlateProfileAttribution::revision_of(&filled())
        );
        assert_ne!(
            PlateProfileAttribution::revision_of(&first),
            PlateProfileAttribution::revision_of(&edited)
        );
    }
}
