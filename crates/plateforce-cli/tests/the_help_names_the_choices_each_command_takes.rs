use std::process::Command;

fn help(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args(arguments)
        .output()
        .expect("the command runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("help is UTF-8")
}

#[test]
fn both_serve_help_spellings_reach_the_page_owned_by_the_server() {
    let help_subcommand = help(&["help", "serve"]);
    let serve_flag = help(&["serve", "--help"]);
    assert_eq!(help_subcommand, serve_flag);
    for option in ["--port", "--open"] {
        assert!(help_subcommand.contains(option), "{option} is unnamed");
    }
    for option in ["--registry", "--plates", "--format", "--out", "--color"] {
        assert!(
            !help_subcommand.contains(option),
            "{option} belongs to the command around serve"
        );
    }
}

#[test]
fn each_shells_completion_instruction_loads_that_shells_script() {
    let page = help(&["completions", "--help"]);
    assert!(page.contains("For the current bash shell:\n  source <(plateforce completions bash)"));
    assert!(page.contains(
        "For the current zsh shell, initialise completions before loading its script:\n  \
         autoload -Uz compinit && compinit\n  source <(plateforce completions zsh)"
    ));
}

#[test]
fn batch_help_names_each_modes_own_files() {
    let page = help(&["batch", "--help"]);
    let modes = page
        .split_once("Analyse mode writes:")
        .expect("analyse outputs are named")
        .1;
    let (analyse, compare) = modes
        .split_once("Compare mode writes:")
        .expect("compare outputs are named");

    for name in [
        "results.csv",
        "descriptions.csv",
        "provenance.csv",
        "refusals.csv",
        "warnings.csv",
        "exclusions.csv",
        "signals.csv",
        "run.json",
        "aggregates.csv",
    ] {
        assert!(analyse.contains(name), "analyse does not name {name}");
    }
    assert!(analyse.contains("when a reduction was bound"));
    for name in [
        "paired.csv",
        "provenance.csv",
        "refusals.csv",
        "compare-run.json",
    ] {
        assert!(compare.contains(name), "compare does not name {name}");
    }
    for name in ["results.csv", "descriptions.csv", "aggregates.csv"] {
        assert!(!compare.contains(name), "compare is assigned {name}");
    }
    assert!(!compare.contains("\n  run.json"));
}
