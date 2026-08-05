//! WebAssembly surface.
//!
//! The browser build runs the same compiled logic as the desktop, so nothing in this
//! crate computes a quantity and nothing here decides a method. It reads files, holds a
//! trial, forwards to `plateforce_analysis`, and hands back JSON.
//!
//! The boundary is JSON strings rather than a structural bridge. That holds the
//! dependency tree at wasm-bindgen and serde, and no payload in this project is large
//! enough for the difference to be measurable: a trial is 6,000 samples.
//!
//! No threads. Threads would require `SharedArrayBuffer`, which would require
//! cross-origin isolation headers, which static hosting does not serve.

pub mod batch;
pub mod demo;
pub mod registry_embed;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use plateforce_analysis::binding::{conditioning_constructs, derived_constructs, SPINE_CONSTRUCTS};
use plateforce_analysis::capability::{capability, AcquisitionIntake, Operation, OutputFormat};
use plateforce_analysis::{document, spread, AnalysisRequest, Binding, BINDINGS};
use plateforce_core::read;
use plateforce_core::signal::{reported_samples, trial_from_column, ReportedSamples, Sentinel};
use plateforce_core::Trial;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Serialize)]
struct BuildInfo {
    version: &'static str,
    /// The revision the registry names about itself, and None where it names none. Named
    /// `declared` rather than `version` because this build pinned nothing: it compiled a
    /// registry in, and the registry's claim about itself is not a citation anybody made.
    registry_declared_version: Option<String>,
    registry_digest: String,
    registry_file_count: usize,
    registry_valid: bool,
    registry_violations: Vec<String>,
    /// Every rule that runs, with the slot it fills.
    bindings: &'static [Binding],
    /// The constructs the request names by its own fields. `Dispatch` carries a function
    /// pointer and is not serialised, and a binding's slot word equals its construct id for
    /// takeoff as well as for every derived row, so an interface reading `bindings` alone
    /// cannot tell which of the two ways a construct is asked for.
    spine_constructs: &'static [&'static str],
    /// What a request may name in `derived`, and what it may name in `conditioning`. The
    /// same argument as the field above, carried the rest of the way: a construct id looks
    /// identical whichever of the three routes reaches it, and an interface that guesses
    /// wrong asks for a step under the wrong name and has the whole request refused.
    ///
    /// Read rather than inferred. Every conditioning rule this build carries happens to
    /// declare no `quantities`, so a surface can separate the two lists that way and be
    /// right today; the day a derived rule lands that reports nothing, that surface starts
    /// naming a step in the wrong map with no test failing.
    derived_constructs: Vec<&'static str>,
    conditioning_constructs: Vec<&'static str>,
    threads: bool,
}

/// Everything the interface needs to describe this build, including which registry it
/// was compiled against and whether that registry passes its own validator. A number is
/// only reproducible alongside this.
#[wasm_bindgen(js_name = buildInfoJson)]
pub fn build_info_json() -> Result<String, JsError> {
    let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
    to_json(&BuildInfo {
        version: version(),
        registry_declared_version: loaded.registry.declared_version.clone(),
        registry_digest: loaded.digest.clone(),
        registry_file_count: loaded.file_count,
        registry_valid: loaded.is_valid(),
        registry_violations: loaded.violation_messages(),
        bindings: BINDINGS,
        spine_constructs: SPINE_CONSTRUCTS,
        derived_constructs: derived_constructs(),
        conditioning_constructs: conditioning_constructs(),
        threads: false,
    })
}

#[derive(Serialize)]
struct Census {
    constructs: usize,
    computation_entries: usize,
    protocol_entries: usize,
    preset_entries: usize,
}

#[derive(Serialize)]
struct RegistryView<'a> {
    digest: String,
    valid: bool,
    violations: Vec<String>,
    constructs: Vec<&'a plateforce_registry::Construct>,
    methods: Vec<&'a plateforce_registry::Method>,
    protocols: Vec<&'a plateforce_registry::Protocol>,
    /// The published pipelines, in full. A tab that received only the count could report how
    /// many there are and offer none of them.
    presets: Vec<&'a plateforce_registry::Preset>,
    census: Census,
}

/// The registry as data, for a picker driven by the registry rather than by a hardcoded
/// list. Adding a method is a file edit and the interface follows.
#[wasm_bindgen(js_name = registryJson)]
pub fn registry_json() -> Result<String, JsError> {
    let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
    let census = loaded.registry.census();
    to_json(&RegistryView {
        digest: loaded.digest.clone(),
        valid: loaded.is_valid(),
        violations: loaded.violation_messages(),
        constructs: loaded.registry.constructs.values().collect(),
        methods: loaded.registry.methods.values().collect(),
        protocols: loaded.registry.protocols.values().collect(),
        presets: loaded.registry.presets.values().collect(),
        census: Census {
            constructs: census.constructs,
            computation_entries: census.computation_entries,
            protocol_entries: census.protocol_entries,
            preset_entries: census.preset_entries,
        },
    })
}

/// The request a published pipeline resolves to, or the record saying why it did not.
///
/// One of the two is present. A refusal travels as the record every other surface receives
/// rather than as a sentence, so a tab branches on the code the terminal exits on.
#[derive(Serialize)]
struct AdoptedPreset {
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<AnalysisRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<plateforce_core::Refusal>,
}

/// The request a named published pipeline resolves to, laid onto the one the tab has built.
///
/// Resolved here rather than in JavaScript. A second implementation of which values a
/// pipeline supplied would be a second answer to the question this product exists to answer,
/// and the interface reads the returned request back into its own controls, so a reader sees
/// exactly what the pipeline bound before anything is computed.
#[wasm_bindgen(js_name = adoptPreset)]
pub fn adopt_preset(preset_id: &str, request_json: &str) -> Result<String, JsError> {
    let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
    let mut request: AnalysisRequest =
        serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;

    let refused = match plateforce_analysis::request::preset_named(&loaded.registry, preset_id) {
        Err(refusal) => Some(*refusal),
        Ok(preset) => request.adopt(preset).err().map(|refusal| *refusal),
    };
    to_json(&AdoptedPreset {
        request: refused.is_none().then_some(request),
        refusal: refused,
    })
}

/// Every entry point this crate hands JavaScript, and what each can be asked to do.
///
/// Keyed by the name JavaScript calls, so an export that goes away takes its operations
/// with it. The tab reaches `plateforce_batch::analyse` and not `compare`, so `batch`
/// appears here and `compare` does not.
/// None for a name with no arm, which the test below refuses, so an export reaching the
/// list without reaching this table cannot pass as one that dispatches nothing.
fn operations_named(export: &str) -> Option<&'static [Operation]> {
    match export {
        "analyse" => Some(&[Operation::Analyse, Operation::Spread]),
        // Shaping the request a pipeline resolves to, which is part of analysing one trial
        // rather than a question of its own.
        "adoptPreset" => Some(&[Operation::Analyse]),
        "spread" => Some(&[Operation::Spread]),
        "parse" => Some(&[Operation::ParseForceFile]),
        "batchJson" | "batchCoverage" => Some(&[Operation::Batch]),
        "capabilityJson" => Some(&[Operation::Capability]),
        // One call answers all three: it returns the census, every entry in full, and
        // whether the registry it was compiled against passes its own validator.
        "registryJson" => Some(&[
            Operation::RegistryCensus,
            Operation::RegistryShow,
            Operation::RegistryValidate,
        ]),
        "buildInfoJson" => Some(&[Operation::Version, Operation::RegistryValidate]),
        // Loading a trial, drawing it, and describing what was loaded. None is
        // a computation this manifest asserts over.
        "fromForceFile" | "demonstration" | "infoJson" | "envelopeJson" | "windowEnvelopeJson"
        | "summaryJson" => Some(&[]),
        _ => None,
    }
}

/// Every name this crate exposes to JavaScript. The test below reads the same names out of
/// the source, so an export added or removed without reaching this list fails there.
const EXPORTS: &[&str] = &[
    "adoptPreset",
    "analyse",
    "batchCoverage",
    "batchJson",
    "buildInfoJson",
    "capabilityJson",
    "demonstration",
    "envelopeJson",
    "fromForceFile",
    "infoJson",
    "parse",
    "registryJson",
    "spread",
    "summaryJson",
    "windowEnvelopeJson",
];

/// What this surface can be asked to do, reported by this surface.
///
/// The browser answers the same question the terminal and the wheel answer, in the same
/// shape. Nothing here reports a build digest: the same commit produces three different
/// wasm digests on three runners, so a digest would make this a comparison that can only
/// fail. Every field is semantic, ids and slots and constructs and refusal codes.
/// Whether a caller of this surface can state the acquisition block.
///
/// `analyse` and the batch request both take a capture whose members fill the block, so the
/// page can state everything the fingerprint asks for. The test below holds this answer to
/// what the crate builds, in both directions.
const ACQUISITION_INTAKE: AcquisitionIntake = AcquisitionIntake::StatedByCaller;

#[wasm_bindgen(js_name = capabilityJson)]
pub fn capability_json() -> Result<String, JsError> {
    let operations: Vec<Operation> = EXPORTS
        .iter()
        .filter_map(|export| operations_named(export))
        .flatten()
        .copied()
        .collect();
    replied(&capability(
        &operations,
        &[OutputFormat::Json],
        ACQUISITION_INTAKE,
    ))
}

/// What the tab was told about the plate, in the words the person answered in.
///
/// Members arrive as text and reach the block through the block's own parser, so a name the
/// block does not hold is refused here rather than dropped, and the tab and the terminal read
/// one member the same way. A saved plate arrives as the members it holds rather than as a
/// digest, because a tab computing the revision itself would be a second implementation of
/// the one thing that tells two revisions of a plate apart.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatedCapture {
    /// What runs, member by member.
    #[serde(default)]
    acquisition: std::collections::BTreeMap<String, String>,
    /// The saved plate the answers were read from, when the person picked one.
    #[serde(default)]
    plate: Option<StatedPlate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatedPlate {
    name: String,
    #[serde(default)]
    members: std::collections::BTreeMap<String, String>,
}

impl StatedCapture {
    /// The block this run carries, and the saved plate behind it.
    pub(crate) fn resolved(self) -> Result<plateforce_core::Capture, JsError> {
        let stated = block_of(&self.acquisition)?;
        let Some(plate) = self.plate else {
            return Ok(plateforce_core::Capture::stated(stated));
        };
        let saved = block_of(&plate.members)?;
        let (acquisition, superseded_members) = stated.over(&saved);
        Ok(plateforce_core::Capture {
            acquisition,
            plate_profile: Some(plateforce_core::PlateProfileAttribution {
                name: plate.name,
                revision: plateforce_core::PlateProfileAttribution::revision_of(&saved),
                superseded_members,
            }),
        })
    }
}

fn block_of(
    members: &std::collections::BTreeMap<String, String>,
) -> Result<plateforce_core::Acquisition, JsError> {
    let mut block = plateforce_core::Acquisition::default();
    for (member, written) in members {
        block.set_member(member, written).map_err(|fault| {
            JsError::new(&match fault {
                plateforce_core::MemberFault::Unknown => format!(
                    "{member} names nothing the acquisition block holds, which has {}",
                    plateforce_core::Acquisition::MEMBERS.join(", ")
                ),
                plateforce_core::MemberFault::NotANumber => {
                    format!("{member} was given '{written}', which is not a number")
                }
            })
        })?;
    }
    Ok(block)
}

/// A parsed force file, before any column has been declared to be the force channel.
#[wasm_bindgen]
pub struct ForceFile {
    inner: read::ForceFile,
}

#[wasm_bindgen]
impl ForceFile {
    /// Parse text that came from a file input. The bytes never leave the tab.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse_text(text: &str) -> Result<ForceFile, JsError> {
        read::parse(text)
            .map(|inner| ForceFile { inner })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// What the reader found and what it decided, including the columns it did not pick.
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<String, JsError> {
        to_json(&self.inner.summary)
    }
}

#[derive(Serialize)]
struct LoadedTrialInfo {
    sample_count: usize,
    sample_rate_hz: f64,
    duration_seconds: f64,
    force_column: usize,
    sentinel_convention: String,
    /// The two reasons a sample is reported, apart. This block published the total under one
    /// name and the result document published the same total under a second, so one surface
    /// had two spellings of one policy and neither could say which reason it counted.
    samples_matching_the_convention: usize,
    samples_carrying_no_number: usize,
    minimum_newtons: f64,
    maximum_newtons: f64,
    synthetic: bool,
}

/// One loaded trial. Held in WebAssembly memory for the life of the tab and written
/// nowhere.
#[wasm_bindgen]
pub struct LoadedTrial {
    trial: Trial,
    info: LoadedTrialInfo,
}

#[wasm_bindgen]
impl LoadedTrial {
    /// Bind a column of a parsed file to the vertical ground reaction force channel.
    ///
    /// The sentinel convention is stated by the caller and never inherited, because a
    /// silent choice here cannot be told apart from a measurement.
    #[wasm_bindgen(js_name = fromForceFile)]
    pub fn from_force_file(
        file: &ForceFile,
        force_column: usize,
        sample_rate_hz: f64,
        sentinel_convention: &str,
    ) -> Result<LoadedTrial, JsError> {
        let column = file.inner.columns.get(force_column).ok_or_else(|| {
            JsError::new(&format!(
                "column {} was requested but the file has {}",
                force_column + 1,
                file.inner.columns.len()
            ))
        })?;

        let sentinel = match sentinel_convention {
            "zero" => Some(Sentinel::Zero),
            "negative_one" => Some(Sentinel::NegativeOne),
            "none" => None,
            other => {
                return Err(JsError::new(&format!(
                    "unknown sentinel convention '{other}'"
                )))
            }
        };

        // Through the one home, so this tab cannot spell the policy a second way. It held
        // every unreadable sample at the last real reading and reached past the declared
        // convention to do it, and answered an interrupted recording with the intact trial's
        // numbers to the last digit. `plateforce_core::signal::trial_from_column` states what
        // that cost and what the alternative cost.
        let (trial, reported) = trial_from_column(column.to_vec(), sample_rate_hz, sentinel)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let info = describe(&trial, force_column, sentinel_convention, reported, false);
        Ok(LoadedTrial { trial, info })
    }

    /// The trial the interface opens with, so the tool is explorable with no data at hand.
    ///
    /// Recorded rather than drawn. Sweeping the published rules for the start of the jump
    /// moves the height by 1.9 cm on this trace and by 0.04 mm on the drawn one, so the
    /// drawn one cannot show what choosing a method costs.
    #[wasm_bindgen(js_name = demonstration)]
    pub fn demonstration() -> LoadedTrial {
        let trial = demo::recorded_countermovement_jump();
        // Drawn rather than recorded, so it carries neither reason. Counted rather than
        // written as zeros: a drawn trace that grew a hole would say so.
        let reported = reported_samples(trial.force(), None);
        let info = describe(&trial, 0, "none", reported, false);
        LoadedTrial { trial, info }
    }

    #[wasm_bindgen(js_name = infoJson)]
    pub fn info_json(&self) -> Result<String, JsError> {
        to_json(&self.info)
    }

    /// Per-bucket minimum and maximum, so a spike survives being drawn at 900 px wide
    /// instead of being averaged away.
    #[wasm_bindgen(js_name = envelopeJson)]
    pub fn envelope_json(&self, buckets: usize) -> Result<String, JsError> {
        self.envelope_between(buckets, 0, self.trial.force().len())
    }

    /// The same lossless envelope over one visible interval. Long recordings can be
    /// inspected without allocating one browser-side value per sample or flattening the
    /// event of interest into a few pixels.
    #[wasm_bindgen(js_name = windowEnvelopeJson)]
    pub fn window_envelope_json(
        &self,
        buckets: usize,
        start_index: usize,
        end_index: usize,
    ) -> Result<String, JsError> {
        self.envelope_between(buckets, start_index, end_index)
    }

    fn envelope_between(
        &self,
        buckets: usize,
        start_index: usize,
        end_index: usize,
    ) -> Result<String, JsError> {
        let force = self.trial.force();
        let start = start_index.min(force.len().saturating_sub(1));
        let end = end_index.clamp(start + 1, force.len());
        let visible = &force[start..end];
        let buckets = buckets.clamp(1, visible.len());
        let width = visible.len() as f64 / buckets as f64;
        let mut lower = Vec::with_capacity(buckets);
        let mut upper = Vec::with_capacity(buckets);
        for bucket in 0..buckets {
            let start = (bucket as f64 * width) as usize;
            let end = (((bucket + 1) as f64 * width) as usize).min(visible.len());
            let slice = &visible[start..end.max(start + 1)];
            lower.push(slice.iter().copied().fold(f64::INFINITY, f64::min));
            upper.push(slice.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        }
        to_json(&Envelope {
            sample_count: force.len(),
            sample_rate_hz: self.trial.sample_rate_hz(),
            lower,
            upper,
        })
    }

    /// One analysis. Every number in the response names the methods that produced it, and a
    /// rule that declined arrives as the record it built rather than as a thrown sentence.
    ///
    /// Returned as the document the terminal and R return, so a result carried out of the tab
    /// says which method set produced it. A tab that answered with the response alone handed
    /// back numbers a reader could not attribute to a registry.
    ///
    /// The name of the trace is the caller's, because the module is handed text and never a
    /// file. A caller that names none has a trace this surface cannot name.
    ///
    /// The plate is the caller's too. This surface passed a literal `false` for the block's
    /// completeness under a comment saying none reaches it, so every result it had ever
    /// produced fingerprinted as incomplete and no reader could change that.
    #[wasm_bindgen(js_name = analyse)]
    pub fn analyse(
        &self,
        request_json: &str,
        trial_name: Option<String>,
        capture_json: Option<String>,
    ) -> Result<String, JsError> {
        let mut request: AnalysisRequest =
            serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;
        let capture = stated_capture(capture_json.as_deref())?;
        let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
        // Read here rather than sent by the page: what a rule falls back to is the registry's
        // claim, and a field the page could fill would let a page publish a default nobody
        // wrote down.
        request.reading(&loaded.registry);
        match plateforce_analysis::run(&self.trial, &request) {
            Ok(response) => replied(&document::ResultDocument::of(
                version(),
                document::TrialSource {
                    name: trial_name.unwrap_or_default(),
                    rows_read: self.info.sample_count,
                    samples_matching_the_convention: self.info.samples_matching_the_convention,
                },
                // Nothing pinned: this surface runs the registry compiled into the bundle,
                // and a tab asserting a revision about bytes it did not choose would be
                // signing the reader's name to the build's own claim.
                &plateforce_core::provenance::RegistryStamp::unpinned(
                    loaded.registry.declared_version.clone(),
                    Some(loaded.digest.clone()),
                ),
                &capture,
                &response,
                // The tab sweeps on its own schedule through `spread`, so an analysis that
                // computed one here would answer a question nobody asked and pay for it.
                None,
            )),
            Err(refusal) => refused(&refusal),
        }
    }

    /// Every defensible alternative for one quantity, and how far the number moves.
    ///
    /// The sweep leaves this tab on its own, so it carries the identity `analyse` above puts on
    /// a result rather than none at all. The stamp is unpinned because `SpreadRequest` denies
    /// unknown fields and this surface has no field a caller could write a revision into, which
    /// is a different thing from a surface that accepts a pin and drops it. What the registry
    /// declares about itself is carried beside the absent pin rather than in place of it.
    #[wasm_bindgen(js_name = spread)]
    pub fn spread(&self, request_json: &str) -> Result<String, JsError> {
        let mut request: spread::SpreadRequest =
            serde_json::from_str(request_json).map_err(|e| JsError::new(&e.to_string()))?;
        let loaded = registry_embed::load().map_err(|e| JsError::new(&e.to_string()))?;
        // Every combination is the base request with one rule or one value swapped, so the
        // declarations are read onto the base once and every candidate reads its own rule's.
        request.base.reading(&loaded.registry);
        match spread::run(&self.trial, &request) {
            Ok(response) => replied(&document::SpreadDocument::of(
                version(),
                &plateforce_core::provenance::RegistryStamp::unpinned(
                    loaded.registry.declared_version.clone(),
                    Some(loaded.digest.clone()),
                ),
                response,
            )),
            Err(refusal) => refused(&refusal),
        }
    }
}

/// What the caller said about the plate, or the empty block when they said nothing.
///
/// An empty block rather than a refusal, because a run told nothing about the plate is a run
/// whose result fingerprints as incomplete and names what would fill it, which is a different
/// thing from a run that cannot happen.
pub(crate) fn stated_capture(
    capture_json: Option<&str>,
) -> Result<plateforce_core::Capture, JsError> {
    let Some(text) = capture_json else {
        return Ok(plateforce_core::Capture::default());
    };
    let stated: StatedCapture = serde_json::from_str(text).map_err(|error| {
        JsError::new(&format!(
            "what the tab said about the plate did not parse: {error}"
        ))
    })?;
    stated.resolved()
}

fn describe(
    trial: &Trial,
    force_column: usize,
    sentinel_convention: &str,
    reported: ReportedSamples,
    synthetic: bool,
) -> LoadedTrialInfo {
    LoadedTrialInfo {
        sample_count: trial.len(),
        sample_rate_hz: trial.sample_rate_hz(),
        duration_seconds: trial.duration_seconds(),
        force_column,
        sentinel_convention: sentinel_convention.to_string(),
        samples_matching_the_convention: reported.matched_the_convention,
        samples_carrying_no_number: reported.carried_no_number,
        minimum_newtons: trial.force().iter().copied().fold(f64::INFINITY, f64::min),
        maximum_newtons: trial
            .force()
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max),
        synthetic,
    }
}

#[derive(Serialize)]
struct Envelope {
    sample_count: usize,
    sample_rate_hz: f64,
    lower: Vec<f64>,
    upper: Vec<f64>,
}

fn to_json<T: Serialize>(value: &T) -> Result<String, JsError> {
    serde_json::to_string(value).map_err(|e| JsError::new(&e.to_string()))
}

/// The envelope every surface returns, `{"ok": ...}` or `{"refusal": {...}}`.
///
/// A reply that can carry a refusal record is written this way, because a thrown string
/// loses every field a caller branches on: the code, the rule, the parameter and what could
/// have been asked for instead. `batchJson` already answered in this shape, and the terminal
/// and the R package answer in it too. The exports that stay on the throwing path are the
/// ones whose only failure is this bundle being broken, where there is nothing to branch to.
fn replied<T: Serialize>(value: &T) -> Result<String, JsError> {
    to_json(&serde_json::json!({ "ok": value }))
}

fn refused(refusal: &plateforce_core::Refusal) -> Result<String, JsError> {
    to_json(&serde_json::json!({ "refusal": refusal }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `js_name` in this crate's source, so the list the manifest is built from is
    /// checked against what the crate actually exposes rather than trusted.
    fn exports_in_the_source() -> Vec<String> {
        let marker = concat!("js_name", " = ");
        let mut found: Vec<String> = [include_str!("lib.rs"), include_str!("batch.rs")]
            .iter()
            .flat_map(|source| source.split(marker).skip(1))
            .filter_map(|tail| tail.split(')').next())
            .map(|name| name.trim().to_string())
            .filter(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
            })
            .collect();
        found.sort();
        found.dedup();
        found
    }

    #[test]
    fn the_manifest_is_built_from_the_entry_points_this_crate_exposes() {
        let mut declared: Vec<String> = EXPORTS.iter().map(|name| name.to_string()).collect();
        declared.sort();
        let found = exports_in_the_source();
        assert!(
            found.len() >= 12,
            "the source scan found {} names, so it is matching nothing",
            found.len()
        );
        assert_eq!(declared, found);
    }

    /// An export reaching the list with no arm in the table would contribute nothing and
    /// read exactly like one that dispatches nothing on purpose.
    #[test]
    fn every_entry_point_states_what_it_dispatches() {
        let unstated: Vec<&&str> = EXPORTS
            .iter()
            .filter(|export| operations_named(export).is_none())
            .collect();
        assert!(unstated.is_empty(), "{unstated:?}");
    }

    /// A surface claiming what it cannot do is the one failure this manifest exists to make
    /// visible, so the two the tab does not reach are named rather than left to the diff.
    #[test]
    fn the_browser_claims_neither_compare_nor_reach() {
        let manifest = capability_json().expect("the manifest serialises");
        assert!(!manifest.contains("\"compare\""), "{manifest}");
        assert!(!manifest.contains("\"reach\""), "{manifest}");
        assert!(manifest.contains("\"batch\""), "{manifest}");
    }

    /// What this surface says about the acquisition block is what this surface does with one.
    ///
    /// Two directions. A boundary that builds a block from what a tab handed it while the
    /// manifest says none reaches here publishes the tab as unable to state what it can state,
    /// and a reader comparing surfaces picks another one for the recording. A manifest claiming
    /// the block with nothing building one is the failure the comparison exists to make visible.
    ///
    /// Held against the construction rather than against the export names, because a block
    /// arrives on a field of a request an existing export already takes, and a scan of the
    /// names would report a tab that accepts one as a tab that does not.
    #[test]
    fn the_block_the_manifest_claims_is_the_block_this_crate_builds() {
        // What the boundary does with a stated block, which is the only thing separating a
        // surface that takes one from a surface that reads the same trace without one.
        // Spelled in two halves because this test lives in the file it reads, and written
        // whole it is what its own scan finds.
        let builds_a_block = ["Acquisition::", "default()"].concat();
        let source_directory = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut source = String::new();
        for entry in std::fs::read_dir(source_directory).expect("the crate has sources") {
            let path = entry.expect("a readable entry").path();
            if path.extension().is_some_and(|kind| kind == "rs") {
                source.push_str(&std::fs::read_to_string(&path).expect("a readable source"));
            }
        }

        // A control first: a scan that read nothing reports every boundary as building no
        // block, which reads exactly like a tab that cannot be told what the plate was.
        assert!(
            source.contains("capabilityJson"),
            "the scan read no source, so its verdict means nothing"
        );

        let builds_one = source.contains(&builds_a_block);
        let claimed = ACQUISITION_INTAKE == AcquisitionIntake::StatedByCaller;
        println!("acquisition block claimed: {claimed}; built from a caller's own: {builds_one}");
        assert_eq!(
            claimed,
            builds_one,
            "the manifest says the block is {}, and this crate {} one from what a caller stated",
            if claimed {
                "stated here"
            } else {
                "absent here"
            },
            if builds_one { "builds" } else { "builds no" }
        );
    }

    /// Every member the block holds reaches the tab's own answer, named rather than counted:
    /// a listing naming four teaches a reader to go and find four.
    #[test]
    fn the_tab_names_every_member_of_the_acquisition_block() {
        let manifest = capability_json().expect("the manifest serialises");
        let unnamed: Vec<&&str> = plateforce_core::Acquisition::MEMBERS
            .iter()
            .filter(|member| !manifest.contains(&format!("\"{member}\"")))
            .collect();
        assert!(
            unnamed.is_empty(),
            "{} of {} members are absent from the tab's manifest: {unnamed:?}",
            unnamed.len(),
            plateforce_core::Acquisition::MEMBERS.len()
        );
        // The value is asserted against the construction above rather than pinned here, so a
        // tab that gains an intake flips one declaration and not two.
        assert!(manifest.contains("\"stated_by_caller\":"), "{manifest}");
    }

    /// JSON strings are the whole boundary of this crate, so a container format arriving
    /// here without a writer behind it would be a claim about nobody's code.
    #[test]
    fn the_only_container_the_tab_writes_is_json() {
        let manifest = capability_json().expect("the manifest serialises");
        assert!(
            manifest.contains("\"output_formats\":[\"json\"]"),
            "{manifest}"
        );
    }
}
