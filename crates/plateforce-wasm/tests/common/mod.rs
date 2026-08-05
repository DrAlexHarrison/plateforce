//! What an integration test needs to run a hand-built request the way the tab runs one.

// Each test binary links this and uses part of it.
#![allow(dead_code)]

use std::sync::{Arc, LazyLock};

use plateforce_analysis::{AnalysisRequest, DeclaredDefaults};
use plateforce_registry::Registry;
use plateforce_wasm::registry_embed;

/// The registry this module carries, which is the one `analyse` and `spread` read. A test
/// that loaded the directory instead would hold the rules to a second registry, and a browser
/// tab has no directory to read.
pub fn registry() -> Registry {
    registry_embed::load()
        .expect("the embedded registry assembles")
        .registry
}

/// What that registry declares, assembled once. A sweep builds thousands of requests and
/// `AnalysisRequest::declared_from` is the path for a surface that read the registry once and
/// kept the answer.
static DECLARED: LazyLock<Arc<DeclaredDefaults>> =
    LazyLock::new(|| Arc::new(DeclaredDefaults::of(&registry())));

/// What both entry points do before running: the rules read the registry's declared defaults
/// rather than copies of them. A hand-built request that skips this reads nothing and refuses,
/// which is the mechanism working, not a fixture to paper over: a test asserting that refusal
/// must NOT call this.
///
/// Called after every slot a test names is in place, because a choice written into a slot
/// afterwards carries no declarations and refuses by name.
pub fn prepared(mut request: AnalysisRequest) -> AnalysisRequest {
    request.declared_from(Arc::clone(&DECLARED));
    request
}
