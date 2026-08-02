//! What build this is.

use serde_json::json;

use crate::exit::Outcome;
use crate::out::Format;
use crate::registry_cmd::canonical;

pub fn run(format: Format) -> Outcome {
    let version = env!("CARGO_PKG_VERSION");
    match format {
        Format::Json => Outcome::complete(canonical(&json!({ "plateforce_version": version }))),
        Format::Text => Outcome::complete(format!("plateforce {version}")),
    }
}
