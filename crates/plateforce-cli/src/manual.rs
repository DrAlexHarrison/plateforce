//! The manual pages and the shell completions, written by the program they describe.
//!
//! This binary is copied onto a machine that has no package manager holding it and no clone of
//! the repository beside it, so a page generated at build time and left in an archive reaches
//! nobody. Both artefacts are rendered from `command_tree()` on demand, which is the same tree
//! `--help` reads, so neither can describe a command that is not there.

use std::path::{Path, PathBuf};

use crate::exit::{Fault, Outcome};
use crate::out::Format;

/// Which shell the completions are written for.
///
/// A repeat of `clap_complete::Shell` rather than that enum directly, so the words this
/// surface accepts are decided here and a reader meets `powershell` rather than `power-shell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
pub enum ShellName {
    Bash,
    Elvish,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Zsh,
}

impl From<ShellName> for clap_complete::Shell {
    fn from(named: ShellName) -> Self {
        match named {
            ShellName::Bash => clap_complete::Shell::Bash,
            ShellName::Elvish => clap_complete::Shell::Elvish,
            ShellName::Fish => clap_complete::Shell::Fish,
            ShellName::PowerShell => clap_complete::Shell::PowerShell,
            ShellName::Zsh => clap_complete::Shell::Zsh,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct ManArgs {
    /// Write the pages here rather than beside this machine's other manual pages
    #[arg(long = "out-dir", value_name = "DIR")]
    pub out_dir: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct CompletionsArgs {
    /// The shell the script is written for
    #[arg(value_enum)]
    pub shell: ShellName,
    /// Write the script to a file in this directory, under the name that shell looks for.
    /// Absent writes it to the terminal
    #[arg(long = "out-dir", value_name = "DIR")]
    pub out_dir: Option<PathBuf>,
}

/// Where manual pages belong on a machine that has no package manager placing them.
///
/// `XDG_DATA_HOME` first because a reader who set it meant it, then the path the base
/// directory specification names when nobody set one. Windows has no such convention and no
/// `man` to read the result, so a run there names the flag rather than inventing a location.
fn default_manual_root() -> Result<PathBuf, String> {
    if let Some(stated) = std::env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(stated);
        if path.is_absolute() {
            return Ok(path.join("man"));
        }
    }
    match std::env::var_os("HOME") {
        Some(home) => Ok(PathBuf::from(home).join(".local").join("share").join("man")),
        None => Err(
            "this machine names no home directory, so --out-dir is where the pages go".to_string(),
        ),
    }
}

pub fn write_manual(args: &ManArgs, format: Format) -> Outcome {
    if format != Format::Text {
        return refuse_a_format("man", format);
    }
    // The reader names a directory of pages, and the pages themselves go in the section
    // directory inside it, because that is where `man` looks and a page one level up is a file
    // rather than a page.
    let root = match &args.out_dir {
        Some(named) => named.clone(),
        None => match default_manual_root() {
            Ok(root) => root,
            Err(message) => return Outcome::declined_line(Fault::Request, message),
        },
    };
    let section = root.join("man1");
    if let Err(error) = std::fs::create_dir_all(&section) {
        return Outcome::declined_line(
            Fault::Request,
            format!("{} cannot be created: {error}", section.display()),
        );
    }

    if let Err(error) = write_pages(&section) {
        return Outcome::declined_line(
            Fault::Request,
            format!("{} cannot be written: {error}", section.display()),
        );
    }

    let written = pages_in(&section);
    Outcome::complete(format!(
        "{} pages written to {}\n\n  man -M {} plateforce\n  man plateforce{}",
        written,
        section.display(),
        root.display(),
        "\n\nThe second reads them once that directory is on the manual path."
    ))
}

/// One page per command in the tree, the top level and every command under it.
///
/// The walk is this crate's rather than `clap_mangen::generate_to`'s, because the examples are
/// the part of a page a reader copies and roff refills a paragraph: run through the generator's
/// own extra section, `--sentinel` comes back out of `man` hyphenated across a line break as
/// `--sen-tinel` with a typographic hyphen in it, which does not run. Here they are written in
/// no-fill under the heading the manual convention gives them.
pub(crate) fn write_pages(section: &Path) -> std::io::Result<usize> {
    let mut tree = crate::command_tree().disable_help_subcommand(true);
    tree.build();
    let mut written = 0;
    write_page_tree(&tree, section, &mut written)?;
    Ok(written)
}

fn write_page_tree(
    command: &clap::Command,
    section: &Path,
    written: &mut usize,
) -> std::io::Result<()> {
    for nested in command.get_subcommands().filter(|one| !one.is_hide_set()) {
        write_page_tree(nested, section, written)?;
    }
    let page = clap_mangen::Man::new(command.clone());
    std::fs::write(section.join(page.get_filename()), render_page(command)?)?;
    *written += 1;
    Ok(())
}

fn render_page(command: &clap::Command) -> std::io::Result<Vec<u8>> {
    let page = clap_mangen::Man::new(command.clone());
    let mut roff: Vec<u8> = Vec::new();
    page.render_title(&mut roff)?;
    page.render_name_section(&mut roff)?;
    page.render_synopsis_section(&mut roff)?;
    page.render_description_section(&mut roff)?;
    if command.get_arguments().any(|one| !one.is_hide_set()) {
        page.render_options_section(&mut roff)?;
    }
    if command.get_subcommands().any(|one| !one.is_hide_set()) {
        page.render_subcommands_section(&mut roff)?;
    }
    if let Some(shown) = command
        .get_after_long_help()
        .or_else(|| command.get_after_help())
    {
        roff.extend_from_slice(examples_section(&shown.to_string()).as_bytes());
    }
    if command.get_version().is_some() || command.get_long_version().is_some() {
        page.render_version_section(&mut roff)?;
    }
    if command.get_author().is_some() {
        page.render_authors_section(&mut roff)?;
    }
    Ok(roff)
}

/// The help's own trailing block as a manual section, with the command lines left as they were
/// written and the prose around them filled the way a page's prose is.
///
/// A line opening with two spaces is a command, which is the shape `examples.rs` states and the
/// shape `scripts/check-help-examples-run.sh` extracts, so the three agree on one rule.
fn examples_section(shown: &str) -> String {
    let mut document = String::from(".SH EXAMPLES\n");
    let mut in_no_fill = false;
    for line in shown.lines() {
        let is_command = line.starts_with("  ") && !line.trim().is_empty();
        if is_command && !in_no_fill {
            document.push_str(".PP\n.nf\n");
            in_no_fill = true;
        } else if !is_command && in_no_fill {
            document.push_str(".fi\n");
            in_no_fill = false;
        }
        if line.trim().is_empty() {
            if !in_no_fill {
                document.push_str(".PP\n");
            }
            continue;
        }
        document.push_str(&as_roff_text(line));
        document.push('\n');
    }
    if in_no_fill {
        document.push_str(".fi\n");
    }
    document
}

/// One line of ours as roff sees it.
///
/// The hyphen is the one that matters: written bare it is a typographic hyphen that `man` may
/// break a word across, so every flag in every example would arrive at a reader unrunnable.
/// A line opening with a control character is escaped so it reads as text rather than as a
/// request.
fn as_roff_text(line: &str) -> String {
    let escaped = line.replace('\\', "\\e").replace('-', "\\-");
    match escaped.starts_with('.') || escaped.starts_with('\'') {
        true => format!("\\&{escaped}"),
        false => escaped,
    }
}

/// What the run put there, counted from the directory rather than from the tree, so a page the
/// generator declined to write is not reported as written.
fn pages_in(section: &Path) -> usize {
    std::fs::read_dir(section)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("plateforce")
                })
                .filter(|entry| entry.path().extension().is_some_and(|suffix| suffix == "1"))
                .count()
        })
        .unwrap_or(0)
}

pub fn write_completions(args: &CompletionsArgs, format: Format) -> Outcome {
    if format != Format::Text {
        return refuse_a_format("completions", format);
    }
    let shell: clap_complete::Shell = args.shell.into();
    let mut tree = crate::command_tree();

    match &args.out_dir {
        Some(directory) => {
            if let Err(error) = std::fs::create_dir_all(directory) {
                return Outcome::declined_line(
                    Fault::Request,
                    format!("{} cannot be created: {error}", directory.display()),
                );
            }
            match clap_complete::generate_to(shell, &mut tree, "plateforce", directory) {
                Ok(path) => Outcome::complete(format!("{}", path.display())),
                Err(error) => Outcome::declined_line(
                    Fault::Request,
                    format!("{} cannot be written: {error}", directory.display()),
                ),
            }
        }
        None => {
            let mut script: Vec<u8> = Vec::new();
            clap_complete::generate(shell, &mut tree, "plateforce", &mut script);
            match String::from_utf8(script) {
                Ok(text) => Outcome::complete(text.trim_end().to_string()),
                Err(error) => Outcome::declined_line(Fault::Internal, format!("{error}")),
            }
        }
    }
}

/// Both commands write one shape of document and neither of them is a result, so a caller who
/// asked for a result's containers is told what this one writes instead of receiving roff or a
/// completion script wrapped in an envelope nothing would read it back out of.
fn refuse_a_format(command: &str, format: Format) -> Outcome {
    let asked = match format {
        Format::Json => "json",
        Format::Markdown => "markdown",
        Format::Text => "text",
    };
    // Named per command, because what each writes is the fact that answers the caller and one
    // sentence covering both could only describe them as documents.
    let writes = match command {
        "man" => "manual pages, which `man` reads",
        _ => "a completion script, which a shell reads",
    };
    Outcome::declined_line(
        Fault::Request,
        format!("--format {asked} reports an analysed trial, and `{command}` writes {writes}. `plateforce {command}` takes no format"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("plateforce-manual-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("a scratch folder");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// One page per command the tree offers, so a command added without a page is a page
    /// missing rather than a page nobody notices. Counted against the tree rather than against
    /// a number written here, which would go on agreeing with itself after the tree moved.
    #[test]
    fn the_pages_cover_every_command_the_tree_offers() {
        let scratch = Scratch::new("covers");
        let section = scratch.0.join("man1");
        std::fs::create_dir_all(&section).unwrap();
        clap_mangen::generate_to(crate::command_tree(), &section).expect("the pages render");

        let mut expected: Vec<String> = vec!["plateforce.1".to_string()];
        for command in crate::command_tree().get_subcommands() {
            if command.get_name() == "help" {
                continue;
            }
            expected.push(format!("plateforce-{}.1", command.get_name()));
            for nested in command.get_subcommands() {
                if nested.get_name() == "help" {
                    continue;
                }
                expected.push(format!(
                    "plateforce-{}-{}.1",
                    command.get_name(),
                    nested.get_name()
                ));
            }
        }

        let missing: Vec<&String> = expected
            .iter()
            .filter(|name| !section.join(name).exists())
            .collect();
        println!(
            "pages written {} of {} commands in the tree",
            expected.len() - missing.len(),
            expected.len()
        );
        assert!(expected.len() > 10, "the walk found {expected:?}");
        assert!(missing.is_empty(), "no page for {missing:?}");
    }

    /// A page a reader can open. `man` reads a roff document, so a file that carries no roff
    /// request at all is a file rather than a page, and an empty one passes an existence check.
    #[test]
    fn a_written_page_carries_the_command_it_describes() {
        let scratch = Scratch::new("content");
        let section = scratch.0.join("man1");
        std::fs::create_dir_all(&section).unwrap();
        clap_mangen::generate_to(crate::command_tree(), &section).expect("the pages render");

        let page = std::fs::read_to_string(section.join("plateforce-analyse.1")).unwrap();
        println!("plateforce-analyse.1 is {} bytes", page.len());
        assert!(
            page.starts_with(".ie"),
            "a roff document opens with a request"
        );
        assert!(page.contains("plateforce\\-analyse"), "{page:.400}");
        assert!(
            page.contains("sample\\-rate\\-hz"),
            "the flags reach the page"
        );
        assert!(page.len() > 1000);
    }

    /// Every shell this surface offers produces a script rather than an empty buffer, which is
    /// what a generator that declined would leave behind.
    #[test]
    fn every_shell_offered_produces_a_script_naming_the_program() {
        for shell in [
            ShellName::Bash,
            ShellName::Elvish,
            ShellName::Fish,
            ShellName::PowerShell,
            ShellName::Zsh,
        ] {
            let outcome = write_completions(
                &CompletionsArgs {
                    shell,
                    out_dir: None,
                },
                Format::Text,
            );
            let script = outcome.document.expect("a script is written");
            println!("{shell:?} script is {} bytes", script.len());
            assert!(script.contains("plateforce"), "{shell:?}");
            assert!(script.contains("analyse"), "{shell:?} lists the commands");
            assert!(script.len() > 200, "{shell:?} wrote {} bytes", script.len());
        }
    }
}
