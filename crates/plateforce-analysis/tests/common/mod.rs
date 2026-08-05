//! What an integration test needs to run a hand-built request the way a surface would.

// Each test binary links this and uses part of it.
#![allow(dead_code)]

use plateforce_analysis::AnalysisRequest;
use plateforce_registry::Registry;

/// The committed registry, the one every surface embeds.
pub fn registry() -> Registry {
    Registry::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../registry"))
        .expect("the committed registry loads")
}

/// What every surface does before running: the rules read the registry's declared defaults
/// rather than copies of them. A hand-built request that skips this reads nothing and
/// refuses, which is the mechanism working, not a fixture to paper over: a test asserting
/// that refusal must NOT call this.
pub fn prepared(mut request: AnalysisRequest) -> AnalysisRequest {
    request.reading(&registry());
    request
}
