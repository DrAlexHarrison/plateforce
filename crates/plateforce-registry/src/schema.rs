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
    /// The field's spoken words for this quantity, for surfaces that show a name beside the
    /// identifier. Measured across six course documents, `takeoff` appears in 6 of 6 and
    /// `onset`, `threshold` and `epoch` in 0 of 6, so the identifier alone reaches a reader
    /// who has met the concept under other words.
    #[serde(default)]
    pub label: Option<String>,
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

/// Whether a row is one side of a live argument, interoperability bookkeeping, or the
/// only published rule. The three want different treatment in the interface.
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

/// One number a source states for a named option, in the unit that source states it in.
///
/// The unit is not optional. A regression coefficient without it is the unit confusion
/// this registry records real instances of, and a coefficient set mixes units within one
/// option: watts per centimetre beside watts per kilogram beside watts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedNumber {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

/// One option of a parameter whose options are names rather than numbers.
///
/// `published_values` carries a parameter the literature varies by number. This carries one
/// it varies by name, from a two-way choice of search signal up to ten regression
/// coefficient sets, where a single option is several numbers in different units.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedValue {
    pub key: String,
    /// The field's spoken words for this option, for surfaces that show a name beside the
    /// key a result records.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, rename = "number")]
    pub numbers: Vec<NamedNumber>,
    /// The citation key this option comes from, where one source states it alone.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// The settings the per-kind-of-rule grain deliberately keeps off the entry list.
///
/// A parameter varies by number or by name, never both, so it carries `published_values`
/// with `default`, or `named_values` with `default_key`. Either default names the source
/// that chose it and the validator refuses a parameter that declares both.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(default)]
    pub unit: Option<String>,
    /// Every value the literature contains, which is how the software can report that a
    /// tool exposes one of six.
    #[serde(default)]
    pub published_values: Vec<f64>,
    #[serde(default, rename = "value")]
    pub named_values: Vec<NamedValue>,
    #[serde(default)]
    pub default: Option<f64>,
    /// The `named_values` key the software binds when the user states none.
    #[serde(default)]
    pub default_key: Option<String>,
    #[serde(default)]
    pub default_source: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub notes: Option<String>,
}

impl Parameter {
    /// Whether a default was declared at all, whichever of the two shapes carries it.
    pub fn has_default(&self) -> bool {
        self.default.is_some() || self.default_key.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionKind {
    HumanVisual,
    Instrument,
    SimultaneousCapture,
    Model,
}

/// What a bias was measured against, where the thing is not a registry entry. Closed for the
/// same reason the vocabularies beside it are closed: an open field takes a mistyped name as a
/// fourth instrument nobody owns, and the entry still loads.
pub const EXTERNAL_CRITERIA: &[&str] = &[
    "motion_capture_marker",
    "rubber_band_goniometer",
    "static_dead_weight_calibration",
];

/// A bias is meaningless without the thing it was measured against, so `criterion`
/// is mandatory. Two device-validation papers derive their reference plate's jump
/// height from flight time, which makes their biases additive to flight-time method
/// bias rather than inclusive of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bias {
    /// The size of the bias. Where `equals_parameter` names a parameter, this is the size
    /// at that parameter's declared default, and the validator holds the two together.
    pub magnitude: f64,
    pub unit: String,
    #[serde(default)]
    pub direction: Option<String>,
    /// The parameter of this entry whose value the bias equals, where it tracks one rather
    /// than being fixed. A rule that waits a dwell before declaring stabilisation overstates
    /// by exactly that dwell, so a magnitude recorded alone goes wrong the moment a reader
    /// changes the parameter the entry told them to change.
    #[serde(default)]
    pub equals_parameter: Option<String>,
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

/// What stands between an entry and a recording it could be computed on.
///
/// Closed, because the classification these are written from fixes exactly these five and
/// a sixth spelling would reach a surface that has no arm for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Boundary {
    /// A different movement on the plate the operator already owns.
    Protocol,
    /// An instrument the lab does not have. The movement is not the barrier.
    Equipment,
    Both,
    /// No acquisition unblocks it: the rule text, constant or equation is not obtainable.
    Source,
    Undetermined,
}

/// Why an entry is out of reach, beside the entry rather than in a table kept in step by
/// hand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reach {
    pub boundary: Boundary,
    /// What would settle an undetermined boundary. Beside a settled one it would read as
    /// doubt about a classification that was made, so the validator refuses it there.
    #[serde(default)]
    pub query: Option<String>,
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
    #[serde(default)]
    pub reach: Option<Reach>,
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

display_as_registry_spells_it!(Boundary {
    Protocol => "protocol",
    Equipment => "equipment",
    Both => "both",
    Source => "source",
    Undetermined => "undetermined",
});
