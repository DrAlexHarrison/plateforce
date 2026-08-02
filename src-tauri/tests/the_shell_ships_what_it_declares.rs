//! What the desktop shell promises its user, asserted rather than reviewed.
//!
//! Every value here changes something a reader meets: whether dropping a file works, whether
//! installing needs an administrator, whether an upgrade recognises the copy already on the
//! machine. None of them fails loudly when it changes, so none of them is left to a reading.

use std::path::{Path, PathBuf};

fn src_tauri() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn configuration() -> serde_json::Value {
    let text = std::fs::read_to_string(src_tauri().join("tauri.conf.json"))
        .expect("the shell has no configuration");
    serde_json::from_str(&text).expect("the configuration is not valid JSON")
}

/// macOS keys preferences, the keychain and notarisation on this string, and Windows keys the
/// Store package family name on it. Changing it after a public release strands everyone who
/// already installed, with no upgrade path, so it changes only deliberately.
#[test]
fn the_identifier_is_the_one_installed_copies_were_keyed_on() {
    assert_eq!(configuration()["identifier"], "dev.aphd.plateforce");
    assert_eq!(configuration()["productName"], "plateforce");
}

/// Tauri attaches a native drag-drop handler whose closure stops GTK signal emission, which
/// leaves the page's own drop listener dead. This one key is the whole fix, and with it back
/// at its default a reader drops a trace onto the window and nothing happens.
#[test]
fn dropping_a_trace_onto_the_window_reaches_the_page() {
    let window = &configuration()["app"]["windows"][0];
    assert_eq!(window["dragDropEnabled"], false);
    assert_eq!(window["label"], "main");
    // The mobile gate the shipped stylesheet already handles. A window that cannot reach it
    // would be the one surface where the layout is untested at that width.
    assert_eq!(window["minWidth"], 390);
}

#[test]
fn every_declared_artefact_is_one_this_project_ships() {
    let targets = configuration()["bundle"]["targets"]
        .as_array()
        .expect("the bundle declares no targets")
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<String>>();
    let mut sorted = targets.clone();
    sorted.sort();
    assert_eq!(sorted, ["appimage", "deb", "dmg", "nsis", "rpm"]);
    // MSI has no per-user install mode, so it needs an administrator, which fails the bar for
    // a researcher on a managed laptop.
    assert!(!targets.contains(&"msi".to_string()));
}

#[test]
fn installing_on_windows_needs_no_administrator() {
    let windows = &configuration()["bundle"]["windows"];
    assert_eq!(windows["nsis"]["installMode"], "currentUser");
    assert_eq!(
        windows["webviewInstallMode"]["type"],
        "downloadBootstrapper"
    );
}

/// Notarisation requires the hardened runtime, which blocks unsigned JIT, and WebKit's
/// JavaScript engine needs it. The two entitlements that widen the attack surface of a program
/// reading other people's research data turn up in copied configurations, so their absence is
/// asserted rather than assumed.
#[test]
fn the_hardened_runtime_grants_exactly_what_webkit_needs() {
    let entitlements = std::fs::read_to_string(src_tauri().join("entitlements.plist"))
        .expect("no entitlements file");
    // The granted keys, not the file's text. The comment above them names what is deliberately
    // absent, and a text search cannot tell a grant from an explanation of one.
    let granted: Vec<&str> = entitlements
        .match_indices("<key>")
        .filter_map(|(at, _)| {
            let after = &entitlements[at + "<key>".len()..];
            after.find("</key>").map(|end| after[..end].trim())
        })
        .collect();
    assert_eq!(granted, ["com.apple.security.cs.allow-jit"]);
    assert_eq!(
        configuration()["bundle"]["macOS"]["minimumSystemVersion"],
        "10.13"
    );
}

/// The updater makes an outbound request on launch. The header the reader sees says their file
/// never leaves the machine, and a clinical or air-gapped install is a population this shell
/// exists to serve.
#[test]
fn nothing_in_the_shell_reaches_the_network_on_its_own() {
    for name in ["tauri.conf.json", "tauri.store.conf.json"] {
        let text = std::fs::read_to_string(src_tauri().join(name)).expect("a configuration");
        assert!(!text.contains("updater"), "{name} carries an updater");
    }

    // The opener is scoped to the two kinds of address the interface links, rather than granted
    // the ability to open whatever URL a page names.
    let capability = std::fs::read_to_string(src_tauri().join("capabilities/default.json"))
        .expect("no capability file");
    assert!(capability.contains("https://doi.org/*"));
    assert!(capability.contains("https://github.com/DrAlexHarrison/*"));
    assert!(!capability.contains("opener:default"));

    let shell = configuration();
    let policy = shell["app"]["security"]["csp"]
        .as_str()
        .expect("the shell sends no content security policy");
    assert!(policy.contains("connect-src 'self'"));
}

/// The page is compiled in from `web/`, and the build hook that produces the half of it version
/// control excludes runs from the repository root rather than from here.
#[test]
fn the_bundle_builds_the_browser_half_before_it_packs() {
    let build = &configuration()["build"];
    assert_eq!(build["frontendDist"], "../web");
    let before = build["beforeBuildCommand"].as_str().expect("no build hook");
    assert_eq!(before, "bash scripts/build-web.sh release");
}
