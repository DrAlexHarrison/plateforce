//! What every module here reads: the registry on disk, and a trial the rules can run on.

use plateforce_registry::{assemble, read_sources, Registry, Source};

const REGISTRY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry");

/// The registry as a directory holds it, which is what the terminal, Python and R load.
/// The browser embeds the same files through the same assembly, so a guard here reads what
/// every surface reads.
pub fn registry() -> Registry {
    let sources: Vec<Source> = read_sources(REGISTRY_ROOT).expect("the registry root is readable");
    assemble(sources.iter().map(Source::pair))
        .expect("the registry assembles")
        .registry
}
