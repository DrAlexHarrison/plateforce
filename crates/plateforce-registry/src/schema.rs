//! Serde types for the registry files. The shapes here are the contract in `docs/schema.md`.

use serde::{Deserialize, Serialize};

/// What is being measured, sitting above the methods that measure it.
///
/// Standing-frame and takeoff-frame jump height differ by about 144 mm for the same
/// method, so they are separate constructs rather than variants of one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Construct {
    pub id: String,
    pub title: String,
    pub unit: String,
    #[serde(default)]
    pub frame: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Recommended,
    Accepted,
    Contested,
    Legacy,
    Deprecated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// Whether a row is one side of a live argument, interoperability bookkeeping, or
/// simply the only published rule. The three want different treatment in the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Debate {
    Genuine,
    VendorOrLegacy,
    SinglePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationRole {
    Proposes,
    Uses,
    Evaluates,
    Disputes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub key: String,
    pub role: CitationRole,
    pub reference: String,
    #[serde(default)]
    pub doi: Option<String>,
    /// False means the claim rests on an abstract or a secondary source, which bars
    /// the entry from `recommended`.
    #[serde(default)]
    pub obtained: bool,
}

/// The settings the per-kind-of-rule grain deliberately keeps off the entry list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(default)]
    pub unit: Option<String>,
    /// Every value the literature actually contains, which is how the software can
    /// report that a tool exposes one of six.
    #[serde(default)]
    pub published_values: Vec<f64>,
    #[serde(default)]
    pub default: Option<f64>,
    #[serde(default)]
    pub default_source: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionKind {
    HumanVisual,
    Instrument,
    SimultaneousCapture,
    Model,
}

/// A bias is meaningless without the thing it was measured against, so `criterion`
/// is mandatory. Two device-validation papers derive their reference plate's jump
/// height from flight time, which makes their biases additive to flight-time method
/// bias rather than inclusive of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bias {
    pub magnitude: f64,
    pub unit: String,
    #[serde(default)]
    pub direction: Option<String>,
    pub criterion: String,
    pub criterion_kind: CriterionKind,
    #[serde(default)]
    pub source: Option<String>,
    /// True when the figure describes only the trials on which the rule worked.
    #[serde(default)]
    pub conditional_on_success: bool,
}

/// Whether a failing rule announces itself. A rule returning an absurd value fails
/// loudly if something is checking and invisibly if nothing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detectability {
    Silent,
    Loud,
    Guarded,
}

/// For some rules the disagreement is not a bias at all. Measured on the 244-trial
/// corpus, two published onset rules miss by more than two seconds on roughly one
/// trial in seven while their medians look ordinary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub rate: f64,
    pub numerator: u32,
    pub denominator: u32,
    pub corpus: String,
    pub definition: String,
    pub detectability: Detectability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisagreementKind {
    Genuine,
    VendorConvention,
    Units,
    Naming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disagreement {
    pub id: String,
    pub kind: DisagreementKind,
    #[serde(default)]
    pub note: Option<String>,
}

/// How hard the interface pushes this choice at the user. `Refuse` exists because
/// some combinations must not be offered at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surfacing {
    DefaultAndHide,
    DefaultAndShow,
    SurfaceOnDemand,
    ForceADecision,
    NeverAUserChoice,
    Refuse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gui {
    pub surfacing: Surfacing,
    #[serde(default)]
    pub sensitivity: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub id: String,
    pub construct: String,
    #[serde(default)]
    pub group: Option<String>,
    pub title: String,
    pub rule: String,
    pub status: Status,
    pub confidence: Confidence,
    #[serde(default)]
    pub debate: Option<Debate>,
    #[serde(default, rename = "parameter")]
    pub parameters: Vec<Parameter>,
    #[serde(default, rename = "citation")]
    pub citations: Vec<Citation>,
    #[serde(default, rename = "bias")]
    pub biases: Vec<Bias>,
    #[serde(default)]
    pub failure: Option<Failure>,
    #[serde(default)]
    pub disagrees_with: Vec<Disagreement>,
    #[serde(default)]
    pub gui: Option<Gui>,
}

/// Where a protocol requirement came from. `ObservedFromCode` covers requirements no
/// paper states: two tools assume the recording was trimmed to a single jump, and on
/// an untrimmed recording they place takeoff 843 ms late on 155 of 244 trials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Published,
    ObservedFromCode,
    VendorDocumented,
}

/// Protocol entries have no rule to evaluate and are counted on their own denominator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub id: String,
    pub area: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub affects: Vec<String>,
    pub provenance: Provenance,
    #[serde(default)]
    pub citations: Vec<Citation>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructFile {
    #[serde(default, rename = "construct")]
    pub constructs: Vec<Construct>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodFile {
    #[serde(default, rename = "method")]
    pub methods: Vec<Method>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolFile {
    #[serde(default, rename = "protocol")]
    pub protocols: Vec<Protocol>,
}

/// The registry spells these in snake_case, and printing them any other way sends a user
/// looking for a string the files do not contain.
macro_rules! display_as_registry_spells_it {
    ($type:ty { $($variant:ident => $spelling:literal),+ $(,)? }) => {
        impl $type {
            pub fn as_registry_str(self) -> &'static str {
                match self { $(<$type>::$variant => $spelling),+ }
            }
        }
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_registry_str())
            }
        }
    };
}

display_as_registry_spells_it!(Status {
    Recommended => "recommended",
    Accepted => "accepted",
    Contested => "contested",
    Legacy => "legacy",
    Deprecated => "deprecated",
});

display_as_registry_spells_it!(Confidence {
    High => "high",
    Medium => "medium",
    Low => "low",
});

display_as_registry_spells_it!(Detectability {
    Silent => "silent",
    Loud => "loud",
    Guarded => "guarded",
});
