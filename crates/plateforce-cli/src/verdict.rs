//! How hard a choice is, what it is worth, and why the registry rules it that way.
//!
//! An entry that carries no sensitivity or no rationale prints no line for it, and says
//! nothing about the absence.

use plateforce_registry::{Method, Surfacing};

use crate::render::Renderer;

/// The registry spells these in snake_case, and printing one any other way sends a reader
/// looking for a string the files do not contain.
pub fn as_registry_spells_it(surfacing: Surfacing) -> &'static str {
    match surfacing {
        Surfacing::DefaultAndHide => "default_and_hide",
        Surfacing::DefaultAndShow => "default_and_show",
        Surfacing::SurfaceOnDemand => "surface_on_demand",
        Surfacing::ForceADecision => "force_a_decision",
        Surfacing::NeverAUserChoice => "never_a_user_choice",
        Surfacing::Refuse => "refuse",
    }
}

/// The three lines an entry's `gui` block earns, in the order a reader needs them: what the
/// interface owes the choice, what the choice is worth, and why.
pub fn lines(method: &Method, renderer: &Renderer) -> Vec<String> {
    let Some(gui) = &method.gui else {
        return Vec::new();
    };
    let mut lines = vec![renderer.field("surfacing", as_registry_spells_it(gui.surfacing))];
    if let Some(sensitivity) = &gui.sensitivity {
        lines.extend(renderer.field_wrapped("sensitivity", sensitivity.trim()));
    }
    if let Some(rationale) = &gui.rationale {
        lines.extend(renderer.field_wrapped("rationale", rationale.trim()));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spelling is stated once here and proved against the schema's own serialisation,
    /// so a variant renamed in the registry fails this rather than printing its old name.
    #[test]
    fn every_verdict_prints_the_word_the_schema_serialises() {
        let every = [
            Surfacing::DefaultAndHide,
            Surfacing::DefaultAndShow,
            Surfacing::SurfaceOnDemand,
            Surfacing::ForceADecision,
            Surfacing::NeverAUserChoice,
            Surfacing::Refuse,
        ];
        for surfacing in every {
            let serialised = serde_json::to_string(&surfacing).expect("a unit variant serialises");
            assert_eq!(
                format!("\"{}\"", as_registry_spells_it(surfacing)),
                serialised
            );
        }
        println!("verdicts agreeing with the schema: {} of 6", every.len());
    }
}
