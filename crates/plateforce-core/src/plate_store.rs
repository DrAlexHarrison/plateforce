//! Saved plates: what one may be called, where they live on a machine, and how a run's
//! capture is filled from one.
//!
//! A lab has one or two plates whose firmware changes rarely, so asking five questions at
//! every analysis asks the same answers hundreds of times, and a bar that tedious is what
//! leaves an acquisition block short of a member. A saved plate is a way of not retyping the
//! answers and never a place a result points at: every run writes the members themselves into
//! its record, so a reader holding one result holds the plate it came off whether or not the
//! machine that produced it still has the file.
//!
//! Here rather than in the terminal, because a plate saved in a terminal is the same plate a
//! notebook and an R session on that machine analyse under, and a second implementation of
//! where the file lives or what a name may be would let two surfaces disagree about which
//! plate a name reaches. A tab has no folder to read and states the members instead, which is
//! why `attributed_to` takes them rather than a path: the attribution is the same on all four
//! surfaces, and only the store behind it is the terminal's, the wheel's and R's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::acquisition::{Acquisition, Capture, MemberFault, PlateProfileAttribution};
use crate::refusal::Refusal;

/// What a call on the store answers with when it declines.
///
/// Boxed for the reason `RuleRefusal::Refused` boxes one: a `Refusal` carries every field a
/// caller branches on and is several times the size of anything here returns on the ok side,
/// so an unboxed error would make every call in this module pay for the refusal it did not
/// make.
pub type Declined = Result<(), Box<Refusal>>;

/// The folder under the user's configuration directory, and the ending every saved plate is
/// filed under.
const FOLDER: &str = "plateforce/plates";
const ENDING: &str = ".toml";

/// The name a refusal from this module reports itself under, so a caller reading a record
/// sees which part of the software declined rather than a rule id that does not exist.
const STORE: &str = "plate";

/// A saved plate as it was read, with the revision the file's members hash to.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedPlate {
    pub name: String,
    pub members: Acquisition,
    pub revision: String,
    /// Where the file was read from, and `None` for a plate a caller stated the members of
    /// rather than one this machine holds. A tab and a notebook both reach the second form.
    pub path: Option<PathBuf>,
}

impl SavedPlate {
    /// A plate from the members somebody stated, under the name they called it.
    ///
    /// The revision is taken here rather than accepted from the caller, because a surface
    /// computing it would be a second implementation of the one thing that tells two
    /// revisions of a plate apart.
    pub fn named(name: &str, members: Acquisition) -> Result<Self, Box<Refusal>> {
        Ok(Self {
            name: checked_name(name)?.to_string(),
            revision: PlateProfileAttribution::revision_of(&members),
            members,
            path: None,
        })
    }

    fn from_file(name: &str, members: Acquisition, path: PathBuf) -> Self {
        Self {
            name: name.to_string(),
            revision: PlateProfileAttribution::revision_of(&members),
            members,
            path: Some(path),
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

    /// The block a run carries when this plate fills it, with anything the caller stated
    /// beside the plate laid over the top and recorded as having displaced it.
    pub fn capture_under(&self, stated: &Acquisition) -> Capture {
        let (acquisition, superseded) = stated.over(&self.members);
        Capture {
            acquisition,
            plate_profile: Some(self.attributed(superseded)),
        }
    }
}

/// The capture a run carries, from a saved plate, from what the caller stated, or from both.
///
/// The one place the two are combined, so a terminal, a notebook and an R session cannot come
/// to different answers about which of the two wins.
pub fn capture_from(plate: Option<&SavedPlate>, stated: &Acquisition) -> Capture {
    match plate {
        Some(plate) => plate.capture_under(stated),
        None => Capture::stated(stated.clone()),
    }
}

/// Where saved plates live when the caller names no folder.
///
/// The place each system keeps a program's settings, so a plate saved here sits beside every
/// other tool's and is backed up by whatever already backs those up. Per user rather than per
/// folder of data, because a plate is a fact about a room and the same person analyses several
/// projects off it. A named folder is how a plate travels with a dataset rather than with the
/// person.
pub fn directory(named: Option<&Path>) -> Result<PathBuf, Box<Refusal>> {
    if let Some(named) = named {
        return Ok(named.to_path_buf());
    }
    configuration_root().map(|root| root.join(FOLDER)).ok_or_else(|| {
        Box::new(Refusal::file_not_read(
            "the folder saved plates live in",
            "this machine reports no configuration folder, so the folder saved plates live in has to be named",
        ))
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

/// A tab holds no folder of its own, so a plate reaches that surface as the members it holds
/// and every path below refuses by the same route a machine with no configuration folder does.
#[cfg(not(any(windows, unix)))]
fn configuration_root() -> Option<PathBuf> {
    None
}

/// What a saved plate may be called.
///
/// The name is the file name, so it is held to what every filesystem carries and what a
/// terminal renders: a name with a separator in it would name a folder somewhere else, and one
/// with a full stop in it would be read as an ending.
pub fn checked_name(name: &str) -> Result<&str, Box<Refusal>> {
    let usable = !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        });
    if usable {
        Ok(name)
    } else {
        Err(Box::new(Refusal::name_not_accepted(
            STORE,
            "name",
            name,
            vec!["letters, digits, - and _".to_string()],
        )))
    }
}

/// One saved plate, read back through the same member parser every surface uses, so a file
/// somebody edited by hand is held to what the block holds rather than read and dropped.
pub fn read(name: &str, plates_directory: Option<&Path>) -> Result<SavedPlate, Box<Refusal>> {
    let name = checked_name(name)?;
    let path = directory(plates_directory)?.join(format!("{name}{ENDING}"));
    let text = std::fs::read_to_string(&path).map_err(|error| {
        Box::new(Refusal::file_not_read(
            path.display().to_string(),
            format!("no plate is saved as {name}: {error}"),
        ))
    })?;
    Ok(SavedPlate::from_file(name, members_in(&text, &path)?, path))
}

/// Every saved plate a folder holds, by name. A folder that is not there holds none, which is
/// the state a machine that has saved no plate is in.
pub fn saved_names(plates_directory: Option<&Path>) -> Result<Vec<String>, Box<Refusal>> {
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

/// Every saved plate a folder holds, read.
pub fn saved(plates_directory: Option<&Path>) -> Result<Vec<SavedPlate>, Box<Refusal>> {
    saved_names(plates_directory)?
        .iter()
        .map(|name| read(name, plates_directory))
        .collect()
}

/// One saved plate written to disk, and the plate that was there before it, if any.
///
/// The previous one is returned rather than passed over, because saving over a name is the
/// edit that leaves an already-recorded result resting on answers the machine no longer holds,
/// and the caller says so where a reader is looking.
pub fn write(
    name: &str,
    members: &Acquisition,
    plates_directory: Option<&Path>,
) -> Result<(SavedPlate, Option<SavedPlate>), Box<Refusal>> {
    let name = checked_name(name)?;
    let directory = directory(plates_directory)?;
    let path = directory.join(format!("{name}{ENDING}"));
    let replaced = read(name, plates_directory).ok();

    std::fs::create_dir_all(&directory).map_err(|error| {
        Box::new(Refusal::file_not_read(
            directory.display().to_string(),
            format!("the folder saved plates live in cannot be made: {error}"),
        ))
    })?;
    std::fs::write(&path, file_text(members)).map_err(|error| {
        Box::new(Refusal::file_not_read(
            path.display().to_string(),
            format!("{name} cannot be written: {error}"),
        ))
    })?;

    Ok((SavedPlate::from_file(name, members.clone(), path), replaced))
}

/// A saved plate removed from a machine. Results already recorded against it carry its
/// members and are unchanged, which is why removing one is not an edit to any of them.
pub fn forget(name: &str, plates_directory: Option<&Path>) -> Result<PathBuf, Box<Refusal>> {
    let saved = read(name, plates_directory)?;
    let path = saved
        .path
        .clone()
        .expect("a plate read from a folder carries the path it was read from");
    std::fs::remove_file(&path).map_err(|error| {
        Box::new(Refusal::file_not_read(
            path.display().to_string(),
            format!("{name} cannot be removed: {error}"),
        ))
    })?;
    Ok(path)
}

/// Every member whose answer moved between two revisions of one saved plate.
pub fn replacements(before: &Acquisition, after: &Acquisition) -> Vec<(String, String, String)> {
    let was: BTreeMap<&str, String> = before.stated_members().into_iter().collect();
    after
        .stated_members()
        .into_iter()
        .filter_map(|(member, now)| {
            was.get(member)
                .filter(|earlier| **earlier != now)
                .map(|earlier| (member.to_string(), earlier.clone(), now))
        })
        .collect()
}

/// The members a saved plate's file holds.
///
/// Values are read as text whichever way they were written, so a person who typed
/// `plate_natural_frequency_hz = 400` and one who typed `"400"` have saved one plate, and the
/// revision they hash to is the same.
pub fn members_in(text: &str, path: &Path) -> Result<Acquisition, Box<Refusal>> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        acquisition: BTreeMap<String, toml::Value>,
    }

    let file: File = toml::from_str(text).map_err(|error| {
        Box::new(Refusal::file_not_read(
            path.display().to_string(),
            format!("this does not read as a saved plate: {error}"),
        ))
    })?;

    let mut members = Acquisition::default();
    for (member, value) in &file.acquisition {
        let written = match value {
            toml::Value::String(text) => text.clone(),
            toml::Value::Integer(number) => number.to_string(),
            toml::Value::Float(number) => number.to_string(),
            other => other.to_string(),
        };
        members
            .set_member(member, &written)
            .map_err(|fault| match fault {
                MemberFault::Unknown => Box::new(Refusal::unknown_parameter(
                    STORE,
                    member,
                    Acquisition::MEMBERS
                        .iter()
                        .map(|held| (*held).to_string())
                        .collect(),
                )),
                MemberFault::NotANumber => Box::new(Refusal::name_not_accepted(
                    STORE,
                    member,
                    &written,
                    vec!["a number".to_string()],
                )),
            })?;
    }
    Ok(members)
}

/// What a saved plate's file says, written from the members alone so the file and the revision
/// cannot come apart.
pub fn file_text(members: &Acquisition) -> String {
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

    /// The round trip the whole feature rests on: five answers saved once are five answers a
    /// later run does not retype, and the block it fills is complete.
    #[test]
    fn a_saved_plate_fills_a_complete_block_on_a_later_run() {
        let scratch = Scratch::new("round-trip");
        write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");

        let plate = read("lab-kistler-1", scratch.path()).expect("the plate reads");
        let capture = capture_from(Some(&plate), &Acquisition::default());

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

    /// A member stated beside a plate is the answer that runs, and the record says what it
    /// replaced. Both halves matter: an overlay that recorded nothing would let two runs off
    /// one plate differ with nothing in either record saying why.
    #[test]
    fn a_member_stated_beside_a_plate_wins_and_the_record_says_what_it_replaced() {
        let scratch = Scratch::new("stated-beside");
        write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");
        let plate = read("lab-kistler-1", scratch.path()).expect("the plate reads");

        let mut stated = Acquisition::default();
        stated
            .set_member("firmware_version", "2.2")
            .expect("a member");
        let capture = capture_from(Some(&plate), &stated);

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

    /// A plate somebody stated the members of and one read off a folder are one plate, which
    /// is what lets a tab and a terminal produce one attribution for one set of answers.
    #[test]
    fn a_stated_plate_and_a_saved_one_hash_to_one_revision() {
        let scratch = Scratch::new("stated-and-saved");
        let (saved, _) =
            write("lab-kistler-1", &filled(), scratch.path()).expect("the plate saves");
        let stated = SavedPlate::named("lab-kistler-1", filled()).expect("the name is usable");

        assert_eq!(stated.revision, saved.revision);
        assert_eq!(
            stated.attributed(BTreeMap::new()),
            saved.attributed(BTreeMap::new())
        );
        assert_eq!(stated.path, None);
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
        assert_eq!(
            replacements(&replaced.members, &second.members),
            vec![(
                "firmware_version".to_string(),
                "2.1".to_string(),
                "2.2".to_string()
            )]
        );
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
    /// whole list, the way every surface refuses it, rather than read and dropped.
    #[test]
    fn a_saved_file_naming_a_member_the_block_does_not_hold_is_refused_by_name() {
        let scratch = Scratch::new("unknown-member");
        std::fs::write(
            scratch.0.join("lab-kistler-1.toml"),
            "[acquisition]\ndebounce_ms = \"50\"\n",
        )
        .expect("the file writes");

        let refusal =
            read("lab-kistler-1", scratch.path()).expect_err("the block holds no debounce");
        let message = refusal.message().to_string();
        assert!(message.contains("debounce_ms"), "{message}");
        for member in Acquisition::MEMBERS {
            assert!(
                message.contains(member),
                "the refusal does not name {member}: {message}"
            );
        }
    }

    /// A plate nobody saved is answered by name, under a code a caller branches on rather
    /// than a sentence each surface would have to match on.
    #[test]
    fn a_plate_nobody_saved_is_refused_by_name() {
        let scratch = Scratch::new("absent");
        let refusal = read("lab-kistler-9", scratch.path()).expect_err("no such plate is saved");
        assert_eq!(refusal.code, crate::RefusalCode::FileNotRead);
        assert!(refusal.message().contains("lab-kistler-9"), "{refusal:?}");
    }

    /// A name that would name a file somewhere else is refused before anything touches the
    /// filesystem, and by the same rule wherever the name arrives from.
    #[test]
    fn a_name_that_would_reach_another_folder_is_refused() {
        for name in ["../secrets", "lab/1", "", "lab.1"] {
            assert!(checked_name(name).is_err(), "{name} was accepted");
            assert!(
                SavedPlate::named(name, filled()).is_err(),
                "{name} was accepted"
            );
        }
        assert!(checked_name("lab-kistler_1").is_ok());
    }

    /// Naming no plates folder reads the place this system keeps a program's settings, which
    /// is what makes a plate saved in one terminal readable in the next, and in a notebook.
    #[test]
    fn the_default_folder_is_the_one_this_system_keeps_settings_in() {
        let Ok(found) = directory(None) else {
            return;
        };
        assert!(found.ends_with(FOLDER), "{}", found.display());
    }
}
