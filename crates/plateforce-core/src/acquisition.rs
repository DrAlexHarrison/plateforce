//! What the plate and its settings were, which no reanalysis recovers.
//!
//! `sample_rate_hz` is the sixth member of the fingerprint's acquisition block and is carried
//! by the `Trial`, so it is not repeated here. A block that cannot be filled fingerprints as
//! incomplete rather than as matching: the most consequential setting in one open tool is a
//! contact debounce living in firmware, and knowing the trace does not recover it.

use serde::{Deserialize, Serialize};

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
}
