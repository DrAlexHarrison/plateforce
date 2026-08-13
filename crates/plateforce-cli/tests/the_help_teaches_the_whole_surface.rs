//! What a stranger with the binary and nothing else can learn from it.
//!
//! The reader this is written for has never seen force-plate software and has nobody to ask,
//! and the program is the whole of the documentation they have. So the properties here are
//! about coverage rather than wording: every command carries an example, the two widths of
//! help say different things, and every flag whose values come out of the registry names the
//! command that prints them.
//!
//! Whether those examples run is a different question, and a heavier one, because it needs a
//! recording and a folder to run them against. `scripts/check-help-examples-run.sh` answers it.

use std::process::Command;

fn help(page: &[&str], width: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(page)
        .arg(width)
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    assert!(
        output.status.success(),
        "{page:?} {width} exited {:?}",
        output.status.code()
    );
    String::from_utf8(output.stdout).expect("the help is UTF-8")
}

/// Every page a reader can reach, read off the program rather than listed here, so a command
/// added without an example fails this rather than going unmentioned.
///
/// `serve` is left out because the server owns its own options and answers for them.
fn every_page() -> Vec<Vec<String>> {
    fn named(document: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut inside = false;
        for line in document.lines() {
            if line.starts_with("Commands:") {
                inside = true;
                continue;
            }
            if inside && line.trim().is_empty() {
                break;
            }
            if !inside {
                continue;
            }
            if let Some(word) = line.split_whitespace().next() {
                if word != "help" && word != "serve" && line.starts_with("  ") {
                    names.push(word.to_string());
                }
            }
        }
        names
    }

    let mut pages: Vec<Vec<String>> = vec![Vec::new()];
    for command in named(&help(&[], "--help")) {
        let nested = named(&help(&[command.as_str()], "--help"));
        pages.push(vec![command.clone()]);
        for under in nested {
            pages.push(vec![command.clone(), under]);
        }
    }
    pages
}

/// The shape `src/examples.rs` states and `scripts/check-help-examples-run.sh` extracts. Held
/// here too, because the three agreeing is what makes the check's count mean anything.
fn example_lines(document: &str) -> Vec<&str> {
    document
        .lines()
        .filter(|line| line.starts_with("  plateforce ") && !line.starts_with("   "))
        .collect()
}

#[test]
fn every_command_shows_an_example() {
    let pages = every_page();
    let bare: Vec<&Vec<String>> = pages
        .iter()
        .filter(|page| {
            let borrowed: Vec<&str> = page.iter().map(String::as_str).collect();
            example_lines(&help(&borrowed, "-h")).is_empty()
        })
        .collect();
    println!(
        "pages showing an example: {} of {}",
        pages.len() - bare.len(),
        pages.len()
    );
    assert!(pages.len() >= 15, "the walk found {} pages", pages.len());
    assert!(bare.is_empty(), "no example on {bare:?}");
}

/// The split clap gives for free and most programs leave unused. Before this, the two were
/// byte-identical at the top level, so a reader who typed the longer one was told the same
/// thing twice and learned that asking for more here returns nothing.
#[test]
fn the_long_help_says_more_than_the_short_one() {
    let short = help(&[], "-h");
    let long = help(&[], "--help");
    println!(
        "top level: -h is {} bytes, --help is {}",
        short.len(),
        long.len()
    );
    assert_ne!(short, long);
    assert!(long.len() > short.len());

    let widened: Vec<Vec<String>> = every_page()
        .into_iter()
        .filter(|page| {
            let borrowed: Vec<&str> = page.iter().map(String::as_str).collect();
            help(&borrowed, "-h") != help(&borrowed, "--help")
        })
        .collect();
    println!("pages whose two widths differ: {}", widened.len());
    assert!(widened.len() >= 8, "{widened:?}");
}

/// A flag whose values are registry ids cannot list them: the registry is data and the list
/// would be wrong the first time somebody added a rule. What does not move is the command that
/// prints them, so that is what each of those flags carries.
#[test]
fn every_flag_taking_a_rule_names_the_command_that_lists_them() {
    for page in [vec!["analyse"], vec!["batch"], vec!["spread"]] {
        let document = help(&page, "--help");
        for flag in ["--weighing", "--onset", "--takeoff", "--preset", "--derive"] {
            let entry = entry_for(&document, flag)
                .unwrap_or_else(|| panic!("{page:?} offers {flag}\n{document}"));
            assert!(
                entry.contains("plateforce methods"),
                "{page:?} {flag} sends a reader nowhere for its values: {entry}"
            );
        }
    }
}

/// One flag's help, from the line naming it to the next flag. Read off the rendered page
/// rather than off the source string, because what a reader gets is the rendering.
fn entry_for(document: &str, flag: &str) -> Option<String> {
    let lines: Vec<&str> = document.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with(&format!("{flag} ")))?;
    let entry: Vec<&str> = lines[start..]
        .iter()
        .take_while(|line| {
            std::ptr::eq(*line, &lines[start])
                || !line.trim_start().starts_with("--")
                || line.trim_start().starts_with(flag)
        })
        .copied()
        .collect();
    Some(entry.join(" "))
}

/// The command the flags above send a reader to answers for every one of them, so a reader
/// following the pointer meets the words they were told to look for.
#[test]
fn the_listing_covers_every_flag_that_points_at_it() {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(["--registry", "../../registry", "methods"])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    let listing = String::from_utf8(output.stdout).expect("the listing is UTF-8");
    for written in [
        "--weighing <METHOD>",
        "--onset <METHOD>",
        "--takeoff <METHOD>",
        "--preset <NAME>",
        "--derive ",
        "--condition ",
    ] {
        assert!(listing.contains(written), "{written} is not in the listing");
    }
    println!("the listing is {} bytes", listing.len());
}
