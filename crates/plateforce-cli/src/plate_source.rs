//! Saved plates: where they live on this machine, and how a run is filled from one.
//!
//! A lab has one or two plates whose firmware changes rarely, so asking five questions at
//! every analysis asks the same answers hundreds of times, and a bar that tedious is what
//! leaves an acquisition block short of a member. A saved plate is a way of not retyping the
//! answers and never a place a result points at: every run writes the members themselves into
//! its record, so a reader holding one result holds the plate it came off whether or not this
//! machine still has the file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use plateforce_core::{Acquisition, Capture, MemberFault, PlateProfileAttribution};

use crate::exit::{Declined, Fault};

/// The help `--plate` shows on the commands that take one.
pub(crate) const PLATE_HELP: &str =
    "Fill the acquisition block from a plate saved by `plateforce plate save`. A member written with --acquisition beside it is the answer that runs, and the record carries what it replaced";

/// The folder name under the user's configuration directory, and the ending every saved plate
/// is filed under.
const FOLDER: &str = "plateforce/plates";
const ENDING: &str = ".toml";

/// A saved plate as it was read, with the revision the file's members hash to.
#[derive(Debug)]
pub(crate) struct SavedPlate {
    pub name: String,
    pub members: Acquisition,
    pub revision: String,
    pub path: PathBuf,
}

impl SavedPlate {
    fn of(name: &str, members: Acquisition, path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            revision: PlateProfileAttribution::revision_of(&members),
            members,
            path,
        }
    }

    /// What this plate contributes to a result: the name a reader recognises and the revision
    /// that tells them whether the file on their machine is still the one that ran.
    pub fn attributed(
        &self,
        superseded_members: BTreeMap<String, String>,
    ) -> PlateProfileAttribution {
        PlateProfileAttribution {
            name: self.name.clone(),
            revision: self.revision.clone(),
            superseded_members,
        }
    }
}

/// The block a run was told about, from a saved plate, from the line, or from both.
pub(crate) fn capture_for(
    plate: Option<&str>,
    assignments: &[String],
    plates_directory: Option<&Path>,
) -> Result<Capture, Declined> {
    let stated = crate::acquisition_arg::stated_acquisition(assignments)?;
    let Some(name) = plate else {
        return Ok(Capture::stated(stated));
    };

    let saved = read(name, plates_directory)?;
    let (acquisition, superseded) = stated.over(&saved.members);
    Ok(Capture {
        acquisition,
        plate_profile: Some(saved.attributed(superseded)),
    })
}

/// Where saved plates live when the caller names no folder.
///
/// The place each system keeps a program's settings, so a plate saved here sits beside every
/// other tool's and is backed up by whatever already backs those up. Per user rather than per
/// folder of data, because a plate is a fact about a room and the same person analyses several
/// projects off it. `--plates` names a folder instead, which is how a plate travels with a
/// dataset rather than with the person.
pub(crate) fn directory(named: Option<&Path>) -> Result<PathBuf, Declined> {
    if let Some(named) = named {
        return Ok(named.to_path_buf());
    }
    configuration_root()
        .map(|root| root.join(FOLDER))
        .ok_or_else(|| {
            Declined::line(
                Fault::Input,
                "this machine reports no configuration folder, so --plates has to name where saved plates live".to_string(),
            )
        })
}

#[cfg(windows)]
fn configuration_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn configuration_root() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn configuration_root() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|set| !set.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

/// What a saved plate may be called.
///
/// The name is the file name, so it is held to what every filesystem carries and what a
/// terminal renders: a name with a separator in it would name a folder somewhere else, and one
/// with a full stop in it would be read as an ending.
pub(crate) fn checked_name(name: &str) -> Result<&str, Declined> {
    let usable = !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    if usable {
        Ok(name)
    } else {
        Err(Declined::line(
            Fault::Request,
            format!("'{name}' is not a name a plate can be saved under, which takes letters, digits, - and _"),
        ))
    }
}

/// One saved plate, read back through the same member parser the command line uses, so a file
/// somebody edited by hand is held to what the block holds rather than read and dropped.
pub(crate) fn read(name: &str, plates_directory: Option<&Path>) -> Result<SavedPlate, Declined> {
    let name = checked_name(name)?;
    let path = directory(plates_directory)?.join(format!("{name}{ENDING}"));
    let text = std::fs::read_to_string(&path).map_err(|error| {
        Declined::line(
            Fault::Input,
            format!(
                "no plate is saved as {name}: {} cannot be read, {error}. `plateforce plate list` names the ones this machine holds",
                path.display()
            ),
        )
    })?;
    Ok(SavedPlate::of(name, members_in(&text, &path)?, path))
}

/// Every saved plate this machine holds, by name.
pub(crate) fn saved_names(plates_directory: Option<&Path>) -> Result<Vec<String>, Declined> {
    let directory = directory(plates_directory)?;
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|file| file.strip_suffix(ENDING))
                .map(str::to_string)
        })
        .collect();
    names.sort();
    Ok(names)
}

/// The members a saved plate's file holds.
///
/// Values are read as text whichever way they were written, so a person who typed
/// `plate_natural_frequency_hz = 400` and one who typed `"400"` have saved one plate, and the
/// revision they hash to is the same.
fn members_in(text: &str, path: &Path) -> Result<Acquisition, Declined> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        acquisition: BTreeMap<String, toml::Value>,
    }

    let file: File = toml::from_str(text).map_err(|error| {
        Declined::line(
            Fault::Input,
            format!("{} does not read as a saved plate: {error}", path.display()),
        )
    })?;

    let mut members = Acquisition::default();
    for (member, value) in &file.acquisition {
        let written = match value {
            toml::Value::String(text) => text.clone(),
            toml::Value::Integer(number) => number.to_string(),
            toml::Value::Float(number) => number.to_string(),
            other => other.to_string(),
        };
        members.set_member(member, &written).map_err(|fault| {
            Declined::line(
                Fault::Input,
                match fault {
                    MemberFault::Unknown => format!(
                        "{} states {member}, which names nothing the block holds, and it holds {}",
                        path.display(),
                        Acquisition::MEMBERS.join(", ")
                    ),
                    MemberFault::NotANumber => format!(
                        "{} gives {member} the value '{written}', which is not a number",
                        path.display()
                    ),
                },
            )
        })?;
    }
    Ok(members)
}

/// One saved plate written to disk, and the plate that was there before it, if any.
///
/// The previous one is returned rather than passed over, because saving over a name is the
/// edit that leaves an already-recorded result resting on answers this machine no longer
/// holds, and the caller says so where a reader is looking.
pub(crate) fn write(
    name: &str,
    members: &Acquisition,
    plates_directory: Option<&Path>,
) -> Result<(SavedPlate, Option<SavedPlate>), Declined> {
    let name = checked_name(name)?;
    let directory = directory(plates_directory)?;
    let path = directory.join(format!("{name}{ENDING}"));
    let replaced = read(name, plates_directory).ok();

    std::fs::create_dir_all(&directory).map_err(|error| {
        Declined::line(
            Fault::Input,
            format!("{} cannot be made: {error}", directory.display()),
        )
    })?;
    std::fs::write(&path, file_text(members)).map_err(|error| {
        Declined::line(
            Fault::Input,
            format!("{} cannot be written: {error}", path.display()),
        )
    })?;

    Ok((SavedPlate::of(name, members.clone(), path), replaced))
}

/// A saved plate removed from this machine.
pub(crate) fn forget(name: &str, plates_directory: Option<&Path>) -> Result<PathBuf, Declined> {
    let saved = read(name, plates_directory)?;
    std::fs::remove_file(&saved.path).map_err(|error| {
        Declined::line(
            Fault::Input,
            format!("{} cannot be removed: {error}", saved.path.display()),
        )
    })?;
    Ok(saved.path)
}

/// What a saved plate's file says, written from the members alone so the file and the revision
/// cannot come apart.
fn file_text(members: &Acquisition) -> String {
    let mut lines = String::from("[acquisition]\n");
    for (member, value) in members.stated_members() {
        lines.push_str(&format!("{member} = \"{}\"\n", value.replace('"', "\\\"")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder this test owns, removed when it drops, so no test writes into the machine's
    /// own configuration folder.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("plateforce-plates-{label}-{}", std::process::id()));
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

    fn assignments(written: &[&str]) -> Vec<String> {
        written.iter().map(|line| (*line).to_string()).collect()
    }

    /// The round trip the whole feature rests on: five answers saved once are five answers a
    /// later run does not retype, and the block it fills is complete.
    #[test]
    fn a_saved_plate_fills_a_complete_block_on_a_later_run() {
        let scratch = Scratch::new("round-trip");
        write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");

        let capture =
            capture_for(Some("lab-kistler-1"), &[], scratch.path()).expect("the plate reads");

        assert!(
            capture.acquisition.is_complete(),
            "{:?}",
            capture.acquisition.missing()
        );
        assert_eq!(capture.acquisition, filled());
        let profile = capture.plate_profile.expect("the record names the plate");
        assert_eq!(profile.name, "lab-kistler-1");
        assert!(
            profile.superseded_members.is_empty(),
            "{:?}",
            profile.superseded_members
        );
    }

    /// A member written beside the plate is the answer that runs, and the record says what it
    /// replaced. Both halves matter: an overlay that recorded nothing would let two runs off
    /// one plate differ with nothing in either record saying why.
    #[test]
    fn a_member_stated_beside_a_plate_wins_and_the_record_says_what_it_replaced() {
        let scratch = Scratch::new("stated-beside");
        write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");

        let capture = capture_for(
            Some("lab-kistler-1"),
            &assignments(&["firmware_version=2.2"]),
            scratch.path(),
        )
        .expect("the plate reads");

        assert_eq!(capture.acquisition.firmware_version.as_deref(), Some("2.2"));
        let profile = capture.plate_profile.expect("the record names the plate");
        assert_eq!(
            profile
                .superseded_members
                .get("firmware_version")
                .map(String::as_str),
            Some("2.1")
        );
    }

    /// Saving over a name is the stale-fact risk this shape accepts, so the revision moves
    /// with the members and the caller is handed what was there before.
    #[test]
    fn saving_over_a_name_changes_the_revision_and_hands_back_what_was_replaced() {
        let scratch = Scratch::new("saving-over");
        let (first, nothing) =
            write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");
        assert!(nothing.is_none());

        let mut edited = filled();
        edited.firmware_version = Some("2.2".to_string());
        let (second, replaced) =
            write("lab-kistler-1", &edited, scratch.path()).expect("the plate saves");

        assert_ne!(first.revision, second.revision);
        let replaced = replaced.expect("a plate was already saved under that name");
        assert_eq!(replaced.revision, first.revision);
        assert_eq!(replaced.members.firmware_version.as_deref(), Some("2.1"));
    }

    /// A file written here is a file this reads, and the number survives being written and
    /// read back as the number it was rather than as a different spelling of it.
    #[test]
    fn a_saved_number_reads_back_as_the_number_it_was() {
        let scratch = Scratch::new("number");
        write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");
        let saved = read("lab-kistler-1", scratch.path()).expect("the plate reads");
        assert_eq!(saved.members.plate_natural_frequency_hz, Some(400.0));
        assert_eq!(
            saved.revision,
            PlateProfileAttribution::revision_of(&filled())
        );
    }

    /// Somebody hand-editing the file writes a bare number, and it is the same saved plate.
    #[test]
    fn a_number_written_without_quotes_is_the_same_saved_plate() {
        let scratch = Scratch::new("bare-number");
        std::fs::write(
            scratch.0.join("lab-kistler-1.toml"),
            "[acquisition]\nfilter_at_capture = \"none\"\ntare_state = \"tared\"\nplate_natural_frequency_hz = 400\nfloor_surface = \"concrete\"\nfirmware_version = \"2.1\"\n",
        )
        .expect("the file writes");

        let saved = read("lab-kistler-1", scratch.path()).expect("the plate reads");
        assert_eq!(saved.members, filled());
        assert_eq!(
            saved.revision,
            PlateProfileAttribution::revision_of(&filled())
        );
    }

    /// A hand-edited file naming something the block does not hold is refused against the
    /// whole list, the way the command line refuses it, rather than read and dropped.
    #[test]
    fn a_saved_file_naming_a_member_the_block_does_not_hold_is_refused_by_name() {
        let scratch = Scratch::new("unknown-member");
        std::fs::write(
            scratch.0.join("lab-kistler-1.toml"),
            "[acquisition]\ndebounce_ms = \"50\"\n",
        )
        .expect("the file writes");

        let declined =
            read("lab-kistler-1", scratch.path()).expect_err("the block holds no debounce");
        let message = format!("{declined:?}");
        assert!(message.contains("debounce_ms"), "{message}");
        for member in Acquisition::MEMBERS {
            assert!(
                message.contains(member),
                "the refusal does not name {member}: {message}"
            );
        }
    }

    /// A plate nobody saved is answered by name, and the answer says where to look for the
    /// ones this machine does hold.
    #[test]
    fn a_plate_nobody_saved_is_refused_by_name() {
        let scratch = Scratch::new("absent");
        let declined = capture_for(Some("lab-kistler-9"), &[], scratch.path())
            .expect_err("no such plate is saved");
        assert!(
            format!("{declined:?}").contains("lab-kistler-9"),
            "{declined:?}"
        );
    }

    /// A name that would name a file somewhere else is refused before anything touches the
    /// filesystem.
    #[test]
    fn a_name_that_would_reach_another_folder_is_refused() {
        for name in ["../secrets", "lab/1", "", "lab.1"] {
            assert!(checked_name(name).is_err(), "{name} was accepted");
        }
        assert!(checked_name("lab-kistler_1").is_ok());
    }

    /// Naming no plates folder reads the place this system keeps a program's settings, which
    /// is what makes a plate saved in one terminal readable in the next.
    #[test]
    fn the_default_folder_is_the_one_this_system_keeps_settings_in() {
        let Ok(found) = directory(None) else {
            return;
        };
        assert!(found.ends_with(FOLDER), "{}", found.display());
    }
}
