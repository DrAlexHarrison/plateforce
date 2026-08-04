//! The desktop shell: the same interface the browser gets, in a window, with the file
//! never leaving the machine and no browser, no server and no terminal in the way.
//!
//! It links no plateforce crate for arithmetic. The WebAssembly module inside the bundle
//! is the same module the browser instantiates, so the numbers here are the numbers there.

// Launching on Windows otherwise draws a console window behind the interface.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sha2::{Digest, Sha256};
use tauri::plugin::Builder as PluginBuilder;

/// The module the page instantiates, addressed as the bundle carries it.
const THE_MODULE_INSIDE_THE_BUNDLE: &str = "pkg/plateforce_wasm_bg.wasm";

/// The page itself, which every bundle carries and which separates a bundle missing the
/// browser build from a binary that embedded nothing.
const THE_PAGE_INSIDE_THE_BUNDLE: &str = "index.html";

fn main() {
    let context = tauri::generate_context!();

    // Before anything draws. A bundle carries its own copy of the browser build, so one
    // built against a stale module ships older numbers while claiming to be the release,
    // and no comparison between the other surfaces would see it.
    if std::env::args().any(|argument| argument == "--capability") {
        std::process::exit(report_what_this_bundle_carries(&context));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            PluginBuilder::<tauri::Wry>::new("external-links")
                .js_init_script(include_str!("external_links.js").to_string())
                .build(),
        )
        .run(context)
        .expect("the desktop shell could not start");
}

fn report_what_this_bundle_carries(context: &tauri::Context) -> i32 {
    if let Some(module) = context.assets().get(&THE_MODULE_INSIDE_THE_BUNDLE.into()) {
        println!("sha256:{:x}", Sha256::digest(&module));
        return 0;
    }

    // Tauri embeds the interface only under `custom-protocol`, which `cargo tauri build`
    // sets and a plain `cargo build` does not, so an empty table is its own situation.
    if context
        .assets()
        .get(&THE_PAGE_INSIDE_THE_BUNDLE.into())
        .is_none()
    {
        eprintln!("plateforce: this binary carries no interface at all.");
        eprintln!("cargo tauri build produces one that does.");
        return 1;
    }

    eprintln!("plateforce: this bundle carries the interface and no browser module.");
    eprintln!("scripts/build-web.sh did not run before it was packed.");
    1
}
