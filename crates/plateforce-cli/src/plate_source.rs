//! What the terminal adds to the saved-plate store: the words on `--plate`, and reading the
//! members a caller wrote beside one.
//!
//! The store itself is `plateforce_core::plate_store`, where the wheel and the R package reach
//! it too. A plate saved in this terminal is the same plate a notebook on this machine
//! analyses under, which it could not be if where the file lives and what a name may be were
//! decided here.

use std::path::Path;

use plateforce_core::plate_store;
use plateforce_core::{Capture, SavedPlate};

use crate::exit::Declined;

/// The help `--plate` shows on the commands that take one.
pub(crate) const PLATE_HELP: &str =
    "Fill the acquisition block from a plate saved by `plateforce plate save`. A member written with --acquisition beside it is the answer that runs, and the record carries what it replaced";

/// The block a run was told about, from a saved plate, from the line, or from both.
pub(crate) fn capture_for(
    plate: Option<&str>,
    assignments: &[String],
    plates_directory: Option<&Path>,
) -> Result<Capture, Declined> {
    let stated = crate::acquisition_arg::stated_acquisition(assignments)?;
    let saved = match plate {
        Some(name) => Some(read(name, plates_directory)?),
        None => None,
    };
    Ok(plate_store::capture_from(saved.as_ref(), &stated))
}

/// One saved plate, with the store's refusal shown as the terminal's own sentence: a plate
/// nobody saved is the one fault a reader can act on from here, and the action is the command
/// that names the plates this machine holds.
pub(crate) fn read(name: &str, plates_directory: Option<&Path>) -> Result<SavedPlate, Declined> {
    plate_store::read(name, plates_directory).map_err(|refusal| {
        let shown = format!(
            "{}. `plateforce plate list` names the ones this machine holds",
            refusal.message()
        );
        Declined::shown_as(*refusal, shown)
    })
}

pub(crate) fn saved_names(plates_directory: Option<&Path>) -> Result<Vec<String>, Declined> {
    plate_store::saved_names(plates_directory).map_err(|refusal| Declined::recorded(*refusal))
}

pub(crate) fn directory(named: Option<&Path>) -> Result<std::path::PathBuf, Declined> {
    plate_store::directory(named).map_err(|refusal| Declined::recorded(*refusal))
}

pub(crate) fn write(
    name: &str,
    members: &plateforce_core::Acquisition,
    plates_directory: Option<&Path>,
) -> Result<(SavedPlate, Option<SavedPlate>), Declined> {
    plate_store::write(name, members, plates_directory)
        .map_err(|refusal| Declined::recorded(*refusal))
}

pub(crate) fn forget(
    name: &str,
    plates_directory: Option<&Path>,
) -> Result<std::path::PathBuf, Declined> {
    plate_store::forget(name, plates_directory).map_err(|refusal| Declined::recorded(*refusal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plateforce_core::Acquisition;

    /// A folder this test owns, removed when it drops, so no test writes into the machine's
    /// own configuration folder.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "plateforce-terminal-plates-{label}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("a scratch folder");
            Self(path)
        }

        fn path(&self) -> Option<&Path> {
            Some(self.0.as_path())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn filled() -> Acquisition {
        let mut members = Acquisition::default();
        for (member, written) in [
            ("filter_at_capture", "none"),
            ("tare_state", "tared"),
            ("plate_natural_frequency_hz", "400"),
            ("floor_surface", "concrete"),
            ("firmware_version", "2.1"),
        ] {
            members.set_member(member, written).expect("a member");
        }
        members
    }

    /// What this file adds to the store: the line's own members laid over a saved plate's.
    /// The store's own round trip is tested where the store is.
    #[test]
    fn a_member_written_beside_a_plate_reaches_the_run_and_the_record() {
        let scratch = Scratch::new("stated-beside");
        write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");

        let capture = capture_for(
            Some("lab-kistler-1"),
            &["firmware_version=2.2".to_string()],
            scratch.path(),
        )
        .expect("the plate reads");

        assert_eq!(capture.acquisition.firmware_version.as_deref(), Some("2.2"));
        let profile = capture.plate_profile.expect("the record names the plate");
        assert_eq!(profile.name, "lab-kistler-1");
        assert_eq!(
            profile
                .superseded_members
                .get("firmware_version")
                .map(String::as_str),
            Some("2.1")
        );
    }

    /// A run that named no plate carries the block it was handed and nothing to attribute.
    #[test]
    fn a_run_that_named_no_plate_has_nothing_to_attribute() {
        let scratch = Scratch::new("no-plate");
        let capture = capture_for(None, &["firmware_version=2.2".to_string()], scratch.path())
            .expect("the line reads");
        assert!(capture.plate_profile.is_none());
        assert_eq!(capture.acquisition.firmware_version.as_deref(), Some("2.2"));
    }

    /// A plate nobody saved is answered by name, and the answer says where to look for the
    /// ones this machine does hold. The second half is this surface's, so it is asserted here.
    #[test]
    fn a_plate_nobody_saved_is_refused_by_name_and_points_at_the_list() {
        let scratch = Scratch::new("absent");
        let declined = capture_for(Some("lab-kistler-9"), &[], scratch.path())
            .expect_err("no such plate is saved");
        assert!(
            declined.terminal().contains("lab-kistler-9"),
            "{declined:?}"
        );
        assert!(
            declined.terminal().contains("plateforce plate list"),
            "{declined:?}"
        );
        // The record a script reads, rather than the sentence a person reads.
        assert_eq!(declined.record()["code"], "file_not_read");
    }
}
