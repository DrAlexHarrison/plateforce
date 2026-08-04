//! The browser interface, compiled in, with the type each file is served as.

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

pub struct Asset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub bytes: &'static [u8],
}

pub fn assets() -> &'static [Asset] {
    EMBEDDED_ASSETS
}

/// Whether this binary carries the WebAssembly module the page instantiates. False in a
/// working tree where `scripts/build-web.sh` has not run.
pub fn carries_the_browser_bundle() -> bool {
    THE_BROWSER_BUNDLE_WAS_BUILT
}

/// The interface files this binary left out on purpose, so a caller can subtract a
/// deliberate exclusion from a directory listing instead of counting to a number.
pub fn not_part_of_the_interface() -> &'static [&'static str] {
    NOT_PART_OF_THE_INTERFACE
}

/// `/` is the document. Everything else is an exact lookup against the compiled-in table,
/// and no filesystem path is ever built from a request.
pub fn asset_for(request_target: &str) -> Option<&'static Asset> {
    let path = request_target
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .strip_prefix('/')?;
    let path = if path.is_empty() { "index.html" } else { path };
    EMBEDDED_ASSETS.iter().find(|asset| asset.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_answers_the_root_and_its_own_name() {
        let root = asset_for("/").expect("no document is embedded");
        assert_eq!(root.path, "index.html");
        assert_eq!(asset_for("/index.html").unwrap().path, "index.html");
        assert_eq!(asset_for("/?cache=0").unwrap().path, "index.html");
    }

    /// A request that walks out of the table finds nothing, because the table is the only
    /// thing the server can answer from.
    #[test]
    fn nothing_outside_the_table_resolves() {
        assert!(asset_for("/../Cargo.toml").is_none());
        assert!(asset_for("/pkg/../../etc/passwd").is_none());
        assert!(asset_for("/nowhere.js").is_none());
        assert!(asset_for("index.html").is_none());
    }
}
