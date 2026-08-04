//! Force-plate kinetics.
//!
//! Every quantity is computed in exactly one place. Independent implementations of the same
//! named method disagree, so a second implementation of anything in here would move the
//! number.
//!
//! Nothing in this crate decides a method. A caller passes a bound method from the
//! registry and gets a result carrying what produced it.

use serde::{Deserialize, Serialize};

pub mod acquisition;
pub mod agreement;
pub mod baseline_offset;
pub mod bspline;
pub mod butterworth;
pub mod calibration;
pub mod cutoff;
pub mod exponential_rise;
pub mod gravity;
pub mod method_ids;
pub mod normalisation;
pub mod onset;
pub mod peak;
pub mod phases;
pub mod power;
pub mod provenance;
pub mod rate;
pub mod read;
pub mod refusal;
pub mod reporting;
pub mod resample;
pub mod series;
pub mod signal;
pub mod smoothing;
pub mod spectrum;
pub mod stabilisation;
pub mod statistics;
pub mod takeoff;
pub mod trial;
pub mod validity;
pub mod warp;
pub mod waveform;

pub use acquisition::{Acquisition, Capture, MemberFault, PlateProfileAttribution};
pub use provenance::ProvenanceChain;
pub use read::{read_delimited_column, read_trial_from_path, ColumnReadReport, ReadError};
pub use refusal::{exit_code, Refusal, RefusalCode};
pub use series::{
    centre_of_mass_displacement_meters, centre_of_mass_velocity_meters_per_second,
    CentreOfMassHeightTable, DisplacementSeries, IntegrationAnchor, IntegrationDirection,
    IntegrationSpec, IntegrationStart, QuadratureRule, VelocitySeries,
};
pub use signal::{Sentinel, Trial, TrialError};
pub use statistics::{DispersionEstimator, VarianceAccumulation};
pub use trial::{
    flight_time_seconds, jump_height_from_flight_time, jump_height_from_takeoff_velocity,
    reactive_strength_index_modified, takeoff_velocity_integration_spec,
    takeoff_velocity_meters_per_second, time_to_takeoff_seconds, CentralTendency, Landmarks,
    WeighingEpoch,
};

/// Standard gravity, re-exported from `gravity`, which also carries the location-dependent
/// value. Declared once because the registry records real instances of implementations
/// disagreeing on it.
pub use gravity::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;

/// Reported alongside every computed quantity so a number never travels without the
/// choices that produced it.
///
/// Field order is the wire order every surface is compared against. The first seven are the
/// shared schema; the rest follow it and default when absent, so a record written against the
/// shared schema alone still reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub method_id: String,
    /// Every numeric value the rule read, each carrying where it came from. A value the
    /// caller typed and one the interface pre-filled move the number identically, so the
    /// record has to keep them apart.
    pub parameters: Vec<crate::provenance::ParameterRecord>,
    /// Choices between named alternatives, which move the number as far as the numbers do.
    pub choices: Vec<crate::provenance::ChoiceRecord>,
    /// The revision a caller pinned, and None when they pinned none. Never filled from the
    /// registry's own claim: a reader takes a value here as the author having chosen it, and
    /// `registry_declared_version` below is where the registry's claim goes.
    pub registry_version: Option<String>,
    /// Digest of the registry files that were read, measured rather than declared, so it
    /// identifies them without resting on a caller's word. None when no registry was read.
    pub registry_digest: Option<String>,
    /// False when the acquisition block could not be filled, in which case this result
    /// must never be declared to match another lab's.
    pub acquisition_complete: bool,
    /// The provenance of each result this one was computed from. Jump height moves with the
    /// onset rule and the weighing epoch, so a record naming only the last step understates
    /// what produced it.
    pub depends_on: Vec<Provenance>,
    /// Where the method itself came from: named by the caller, accepted from the registry's
    /// recommendation, adopted with a published pipeline, or run because nobody named one. A
    /// bulk acceptance, a considered pick, a pipeline's binding and a rule the software chose
    /// are four records, not one.
    #[serde(default = "method_source_default")]
    pub method_source: crate::provenance::ParameterSource,
    /// Names the request carried that this rule does not read, reported rather than dropped.
    #[serde(default)]
    pub not_read: Vec<String>,
    /// Set when a marker was dragged. The strongest provenance fact a record can carry, and
    /// an export that lost it would be exportable and wrong.
    #[serde(default)]
    pub manual_override: bool,
    /// False when no registry row carries this id.
    #[serde(default = "registry_entry_default")]
    pub registry_entry: bool,
    /// The entry this rule composes, when it is a composition rather than an entry.
    #[serde(default)]
    pub composed_from: Option<String>,
    /// The named published pipeline this rule and its values were adopted from, and None on
    /// a step no pipeline spoke to. A pipeline binds the slots its source states, so a
    /// result can carry it on some steps and not others.
    #[serde(default)]
    pub preset: Option<crate::provenance::PresetAttribution>,
    /// The revision the registry names about itself, and None where it names none. A claim
    /// the data makes, never a caller's, which is why it is not `registry_version`.
    ///
    /// Not recoverable from the digest: the walk that measures the digest reads only `toml`
    /// files and the revision lives in a `VERSION` file beside them, so two registries with
    /// byte-identical rules and different revisions carry one digest and two claims.
    #[serde(default)]
    pub registry_declared_version: Option<String>,
}

/// A record silent about how its method was chosen has not said the caller chose it. Silence
/// is the software's own choice, which is what `Assumed` spells, and reading it as `Stated`
/// would put a reader's signature on a rule the record never claims they saw.
fn method_source_default() -> crate::provenance::ParameterSource {
    crate::provenance::ParameterSource::Assumed
}

/// A record silent either way is a registry entry, because the absence of a row is the
/// exceptional case and the one worth stating.
fn registry_entry_default() -> bool {
    true
}

impl Provenance {
    /// A step with nothing bound to it, for a caller filling the fields it has.
    ///
    /// Nothing bound includes the claim about the rule, so the step starts where the serde
    /// default starts: the software's own choice, which a caller who knows the reader picked
    /// the rule overwrites by saying so.
    pub fn of(method_id: impl Into<String>) -> Self {
        Self {
            method_id: method_id.into(),
            method_source: crate::provenance::ParameterSource::Assumed,
            parameters: Vec::new(),
            choices: Vec::new(),
            depends_on: Vec::new(),
            registry_version: None,
            registry_declared_version: None,
            registry_digest: None,
            acquisition_complete: false,
            not_read: Vec::new(),
            manual_override: false,
            registry_entry: true,
            composed_from: None,
            preset: None,
        }
    }

    /// The values this rule read, as name and value pairs. Reading them back out drops the
    /// source, which is why it is a named call and not the field itself.
    pub fn bound_parameters(&self) -> Vec<(String, f64)> {
        self.parameters
            .iter()
            .map(|record| (record.name.clone(), record.value))
            .collect()
    }

    /// This step and every one upstream of it, depth first.
    pub fn flattened(&self) -> Vec<&Provenance> {
        let mut collected = Vec::new();
        self.collect_into(&mut collected);
        collected
    }

    fn collect_into<'a>(&'a self, into: &mut Vec<&'a Provenance>) {
        into.push(self);
        for input in &self.depends_on {
            input.collect_into(into);
        }
    }
}

/// A value and the choices that produced it. Nothing user-facing returns a bare f64.
///
/// `unit` is a fixed string the registry spells, so this serialises and does not deserialise.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measured {
    pub value: f64,
    pub unit: &'static str,
    pub provenance: Provenance,
}

/// What a computation dropped and under which rule. Silent exclusion is the failure
/// mode the registry documents, and silent inclusion of a sentinel is the same defect
/// wearing the opposite costume.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Exclusions {
    pub dropped_samples: usize,
    pub reason: Option<String>,
    pub sentinel_convention: Option<String>,
}
