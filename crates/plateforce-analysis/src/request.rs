//! What a caller asks for: a rule per slot, the values bound to it, and the constants the
//! request itself carries.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use serde::Deserialize;

/// Unknown fields are refused rather than ignored. A caller whose field name has drifted
/// from this one would otherwise send every value it holds into nothing, and each rule
/// would run its own value under the id the caller asked for.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodChoice {
    pub method_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    /// Enumerated settings that are not numbers, such as which divisor a standard
    /// deviation uses. They are choices in exactly the same sense and are fingerprinted
    /// the same way.
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Set when the user dragged the marker. An override is a provenance fact, not a
    /// bypass, so it is reported next to the number it changed.
    #[serde(default)]
    pub manual_index: Option<usize>,
    /// Names in `parameters` and `options` whose value came from the caller accepting the
    /// registry's recommendation rather than choosing it. An acceptance and a hand-typed
    /// value produce byte-identical requests otherwise, so the request has to say which.
    #[serde(default)]
    pub recommended: BTreeSet<String>,
    /// Set when the rule itself came from the recommendation rather than being picked.
    #[serde(default)]
    pub method_from_recommendation: bool,
    /// Names the caller filled from the registry's default with nobody asked. Distinct from
    /// `recommended`, which is an act somebody performed.
    #[serde(default)]
    pub from_registry_default: BTreeSet<String>,
}

/// `Default` so a caller can build one with `..Default::default()`. The next field this
/// struct gains would otherwise break every exhaustive literal in the workspace at once.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeighingChoice {
    pub method_id: String,
    #[serde(default)]
    pub start_index: Option<usize>,
    /// Named as the registry names them. The three weighing rules each carry their own name
    /// for the window's length, so there is no one field that would fit all three.
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    /// Names in `parameters` and `options` whose value came from the caller accepting the
    /// registry's recommendation rather than choosing it. An acceptance and a hand-typed
    /// value produce byte-identical requests otherwise, so the request has to say which.
    #[serde(default)]
    pub recommended: BTreeSet<String>,
    /// Set when the rule itself came from the recommendation rather than being picked.
    #[serde(default)]
    pub method_from_recommendation: bool,
    /// Names the caller filled from the registry's default with nobody asked. Distinct from
    /// `recommended`, which is an act somebody performed.
    #[serde(default)]
    pub from_registry_default: BTreeSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisRequest {
    pub weighing: WeighingChoice,
    pub onset: MethodChoice,
    pub takeoff: MethodChoice,
    #[serde(default)]
    pub touchdown_index: Option<usize>,
    /// Gravity is a bound parameter because the tools disagree on it, and the core takes
    /// it as an argument for the same reason.
    #[serde(default = "standard_gravity")]
    pub gravity_meters_per_second_squared: f64,
    #[serde(default)]
    pub registry_backed_ids: Vec<String>,
}

fn standard_gravity() -> f64 {
    STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED
}

impl AnalysisRequest {
    pub(crate) fn is_backed(&self, method_id: &str) -> bool {
        self.registry_backed_ids.iter().any(|id| id == method_id)
    }
}
