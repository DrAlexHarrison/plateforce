//! The saved plates on this machine: writing one, reading one back, and removing one.

use std::path::Path;

use plateforce_core::plate_store::replacements;
use plateforce_core::SavedPlate;
use serde_json::json;

use crate::exit::Outcome;
use crate::out::Format;
use crate::plate_source;
use crate::registry_cmd::canonical;

#[derive(clap::Subcommand)]
pub enum Command {
    /// Record a plate's settings once, so a later run is told about it by name
    #[command(after_help = crate::examples::PLATE_SAVE_SHORT)]
    Save {
        /// What to call this plate
        name: String,
        #[arg(
            long = "acquisition",
            value_name = "ASSIGNMENT",
            help = crate::acquisition_arg::ACQUISITION_HELP
        )]
        acquisition: Vec<String>,
    },
    /// Name every plate saved on this machine
    #[command(after_help = crate::examples::PLATE_LIST_SHORT)]
    List,
    /// Show one saved plate's settings
    #[command(after_help = crate::examples::PLATE_SHOW_SHORT)]
    Show {
        /// The plate to read
        name: String,
    },
    /// Remove a saved plate from this machine
    #[command(after_help = crate::examples::PLATE_FORGET_SHORT)]
    Forget {
        /// The plate to remove
        name: String,
    },
}

pub fn run(command: &Command, plates_directory: Option<&Path>, format: Format) -> Outcome {
    match command {
        Command::Save { name, acquisition } => save(name, acquisition, plates_directory, format),
        Command::List => list(plates_directory, format),
        Command::Show { name } => show(name, plates_directory, format),
        Command::Forget { name } => forget(name, plates_directory, format),
    }
    .unwrap_or_else(Outcome::declined)
}

fn save(
    name: &str,
    acquisition: &[String],
    plates_directory: Option<&Path>,
    format: Format,
) -> Result<Outcome, crate::exit::Declined> {
    let members = crate::acquisition_arg::stated_acquisition(acquisition)?;
    let (saved, replaced) = plate_source::write(name, &members, plates_directory)?;

    // What a later run will and will not be able to declare, said now rather than on the run
    // that finds out. A plate short of a member is savable and the runs it fills report an
    // incomplete block, so the answer belongs where somebody can still go and find the rest.
    let missing = saved.members.missing();
    // The answers this save replaced, which is what leaves a result recorded earlier resting
    // on settings this machine no longer holds.
    let displaced: Vec<(String, String, String)> = replaced
        .as_ref()
        .map(|before| replacements(&before.members, &saved.members))
        .unwrap_or_default();

    Ok(match format {
        Format::Markdown => return Ok(crate::out::markdown_wants_a_result("plate")),
        Format::Json => Outcome::complete(canonical(&json!({
            "plate": saved.name,
            "path": filed_at(&saved),
            "revision": saved.revision,
            "acquisition": saved.members,
            "acquisition_complete": saved.members.is_complete(),
            "missing": missing,
            "replaced_revision": replaced.as_ref().map(|before| before.revision.clone()),
            "replaced_members": displaced
                .iter()
                .map(|(member, was, now)| json!({ "member": member, "was": was, "now": now }))
                .collect::<Vec<_>>(),
        }))),
        Format::Text => {
            let mut lines = vec![
                format!("{} saved at {}", saved.name, filed_at(&saved)),
                format!("revision {}", saved.revision),
            ];
            for (member, value) in saved.members.stated_members() {
                lines.push(format!("  {member} {value}"));
            }
            if !missing.is_empty() {
                lines.push(format!(
                    "still to answer: {}. A run filled from this plate reports a block short of them",
                    missing.join(", ")
                ));
            }
            if let Some(before) = &replaced {
                lines.push(format!(
                    "replaces revision {}, so a result recorded against that one rests on settings this plate no longer holds",
                    before.revision
                ));
                for (member, was, now) in &displaced {
                    lines.push(format!("  {member} was {was}, now {now}"));
                }
            }
            Outcome::complete(lines.join("\n"))
        }
    })
}

fn list(plates_directory: Option<&Path>, format: Format) -> Result<Outcome, crate::exit::Declined> {
    let names = plate_source::saved_names(plates_directory)?;
    let mut saved = Vec::new();
    for name in &names {
        saved.push(plate_source::read(name, plates_directory)?);
    }

    Ok(match format {
        Format::Markdown => return Ok(crate::out::markdown_wants_a_result("plate")),
        Format::Json => Outcome::complete(canonical(&json!({
            "plates": saved
                .iter()
                .map(|plate| json!({
                    "plate": plate.name,
                    "revision": plate.revision,
                    "acquisition_complete": plate.members.is_complete(),
                    "missing": plate.members.missing(),
                }))
                .collect::<Vec<_>>(),
            "plates_folder": plate_source::directory(plates_directory)?.display().to_string(),
        }))),
        Format::Text => {
            let folder = plate_source::directory(plates_directory)?;
            let mut lines = vec![format!("{} in {}", counted(saved.len()), folder.display())];
            for plate in &saved {
                let missing = plate.members.missing();
                lines.push(format!(
                    "  {}  {}{}",
                    plate.name,
                    plate.revision,
                    if missing.is_empty() {
                        String::new()
                    } else {
                        format!("  still to answer: {}", missing.join(", "))
                    }
                ));
            }
            Outcome::complete(lines.join("\n"))
        }
    })
}

fn show(
    name: &str,
    plates_directory: Option<&Path>,
    format: Format,
) -> Result<Outcome, crate::exit::Declined> {
    let saved = plate_source::read(name, plates_directory)?;
    Ok(match format {
        Format::Markdown => return Ok(crate::out::markdown_wants_a_result("plate")),
        Format::Json => Outcome::complete(canonical(&json!({
            "plate": saved.name,
            "path": filed_at(&saved),
            "revision": saved.revision,
            "acquisition": saved.members,
            "acquisition_complete": saved.members.is_complete(),
            "missing": saved.members.missing(),
        }))),
        Format::Text => {
            let mut lines = vec![
                format!("{} at {}", saved.name, filed_at(&saved)),
                format!("revision {}", saved.revision),
            ];
            for (member, value) in saved.members.stated_members() {
                lines.push(format!("  {member} {value}"));
            }
            let missing = saved.members.missing();
            if !missing.is_empty() {
                lines.push(format!("still to answer: {}", missing.join(", ")));
            }
            Outcome::complete(lines.join("\n"))
        }
    })
}

fn forget(
    name: &str,
    plates_directory: Option<&Path>,
    format: Format,
) -> Result<Outcome, crate::exit::Declined> {
    let path = plate_source::forget(name, plates_directory)?;
    Ok(match format {
        Format::Markdown => return Ok(crate::out::markdown_wants_a_result("plate")),
        Format::Json => Outcome::complete(canonical(&json!({
            "plate": name,
            "path": path.display().to_string(),
            "saved": false,
        }))),
        Format::Text => Outcome::complete(format!(
            "{name} removed from {}. Results already recorded against it carry its members and are unchanged",
            path.display()
        )),
    })
}

/// Where a plate this command wrote or read is filed. Every plate these four subcommands
/// reach came off a folder, so the empty case is a plate stated in a request on some other
/// surface and never reaches this line.
fn filed_at(plate: &SavedPlate) -> String {
    plate
        .path
        .as_deref()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}

/// The count with the thing it counts, since a bare number reads as a total of something else.
fn counted(plates: usize) -> String {
    match plates {
        1 => "1 saved plate".to_string(),
        other => format!("{other} saved plates"),
    }
}
