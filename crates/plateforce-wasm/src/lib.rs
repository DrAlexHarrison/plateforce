//! WebAssembly surface. Scaffold only; the binding layer is not written yet.

/// Placeholder so the workspace builds before the binding layer lands.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
