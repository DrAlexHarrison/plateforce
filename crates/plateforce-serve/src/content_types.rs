// What a browser must be told each kind of interface file is. `build.rs` includes this
// file as well as the crate compiling it, so the type the build script validates against
// and the type the server sends are one table. Regular comments rather than module doc,
// because an included file lands mid-file where inner doc comments are illegal.

/// Two of these are load-bearing rather than cosmetic. A module script served as anything
/// but a JavaScript type is refused outright by the browser and the page stays blank; a
/// WebAssembly module served as anything but `application/wasm` instantiates through the
/// non-streaming path, slower, recorded by nothing but a console warning.
pub const CONTENT_TYPES: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("wasm", "application/wasm"),
];

/// `None` for a file kind the interface has never carried. `build.rs` refuses to embed one
/// rather than sending it as `application/octet-stream`: a wrong content type fails
/// silently in a browser, and a refused build fails in front of whoever caused it.
pub fn content_type_for(path: &str) -> Option<&'static str> {
    let (_, extension) = path.rsplit_once('.')?;
    CONTENT_TYPES
        .iter()
        .find(|(suffix, _)| *suffix == extension)
        .map(|(_, content_type)| *content_type)
}
