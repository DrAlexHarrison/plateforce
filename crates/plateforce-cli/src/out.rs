//! Where a result goes, and what it is made of when it gets there.
//!
//! Windows PowerShell 5.1 writes UTF-16LE with a byte-order mark through `>`, and
//! `json.load`, `jq`, `pandas.read_json` and `jsonlite` all reject it. So a result asked for
//! by path is written by this program: UTF-8, no mark, `\n` on every platform.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::ValueEnum;

use crate::exit::{Fault, Stream};

/// What a result is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum Format {
    Text,
    Json,
}

/// Puts the document where the operator asked for it.
pub fn deliver(
    document: &str,
    destination: Option<&Path>,
    stream: Stream,
) -> Result<(), (Fault, String)> {
    match destination {
        Some(path) => write_file(document, path),
        None => {
            let ending = if document.ends_with('\n') { "" } else { "\n" };
            match stream {
                Stream::Stdout => {
                    let mut out = anstream::stdout();
                    write!(out, "{document}{ending}").and_then(|()| out.flush())
                }
                Stream::Stderr => {
                    let mut out = anstream::stderr();
                    write!(out, "{document}{ending}").and_then(|()| out.flush())
                }
            }
            .map_err(|error| (Fault::Internal, format!("{error}")))
        }
    }
}

/// Writes beside the destination and renames, so an interrupted run leaves the previous file
/// or the new one and never a truncated one.
fn write_file(document: &str, path: &Path) -> Result<(), (Fault, String)> {
    let staging = staging_path(path);
    let ending = if document.ends_with('\n') { "" } else { "\n" };
    let write = std::fs::File::create(&staging).and_then(|mut file| {
        file.write_all(document.as_bytes())?;
        file.write_all(ending.as_bytes())?;
        file.sync_all()
    });
    if let Err(error) = write {
        let _ = std::fs::remove_file(&staging);
        return Err((
            Fault::Request,
            format!("{} cannot be written: {error}", path.display()),
        ));
    }
    std::fs::rename(&staging, path).map_err(|error| {
        let _ = std::fs::remove_file(&staging);
        (
            Fault::Request,
            format!("{} cannot be replaced: {error}", path.display()),
        )
    })
}

fn staging_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.partial", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_file_starts_with_its_own_first_character_and_ends_with_one_newline() {
        let directory = std::env::temp_dir().join(format!("plateforce-out-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("result.json");
        write_file("{\"ok\":1}", &path).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[..1],
            b"{",
            "no byte-order mark precedes the document"
        );
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\r').count(), 0);
        assert!(!directory.join("result.json.partial").exists());
        std::fs::remove_dir_all(&directory).unwrap();
    }

    /// A second write over the same path replaces it rather than appending, and the staging
    /// file it went through does not survive.
    #[test]
    fn writing_twice_leaves_one_document() {
        let directory =
            std::env::temp_dir().join(format!("plateforce-out-twice-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("result.json");
        write_file("{\"first\":1}", &path).unwrap();
        write_file("{\"second\":2}", &path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"second\":2}\n");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
