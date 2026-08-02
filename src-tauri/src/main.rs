//! The desktop shell: the same interface the browser gets, in a window, with the file
//! never leaving the machine and no browser, no server and no terminal in the way.
//!
//! It links no plateforce crate for arithmetic. The WebAssembly module inside the bundle
//! is the same module the browser instantiates, so the numbers here are the numbers there
//! by construction rather than by a second implementation agreeing with the first.

// A second console window behind the app on Windows would be the only surface where
// launching plateforce shows the reader something that is not the interface.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sha2::{Digest, Sha256};
use tauri::plugin::Builder as PluginBuilder;

/// The module the page instantiates, addressed as the bundle carries it.
const THE_MODULE_INSIDE_THE_BUNDLE: &str = "pkg/plateforce_wasm_bg.wasm";

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
    match context.assets().get(&THE_MODULE_INSIDE_THE_BUNDLE.into()) {
        Some(module) => {
            println!("sha256:{:x}", Sha256::digest(&module));
            0
        }
        None => {
            eprintln!("plateforce: this bundle carries no browser module");
            eprintln!("the build skipped scripts/build-web.sh, so there is no interface in it");
            1
        }
    }
}
