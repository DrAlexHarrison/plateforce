//! Why the software declined to produce a number.
//!
//! Every field a caller can branch on is a field, never a substring of `message`, and the
//! sentence is generated here so a refusal reads the same in a browser tab, a traceback, an
//! R condition and a terminal.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Declares the enum and the list of every variant from one source, so the list a surface
/// reports as this build's vocabulary cannot fall behind the codes the build can emit.
macro_rules! refusal_codes {
    ($( $(#[$note:meta])* $variant:ident ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum RefusalCode {
            $( $(#[$note])* $variant, )+
        }

        impl RefusalCode {
            /// Every code this build can emit, generated beside the enum so a manifest
            /// cannot assert a vocabulary the binary has outgrown.
            pub const ALL: &'static [RefusalCode] = &[ $( RefusalCode::$variant, )+ ];
        }
    };
}

refusal_codes! {
    NoCrossing,
    CollapsedBand,
    MethodNotImplemented,
    UnknownParameter,
    ParameterNotFinite,
    /// A value outside what its parameter takes, reported with the value and what is taken.
    /// Distinct from `ParameterNotFinite`, where the number is not a number, and from
    /// `UnknownParameter`, where the name rather than the value is the fault.
    ValueNotAccepted,
    TraceTooShort,
    ColumnNotFound,
    SentinelConventionUnknown,
    RegistryInvalid,
    /// A result was asked for while a choice the registry forces is still open.
    DecisionNotMade,
    /// A parameter the registry marks required, with no default to fall through to, that
    /// nobody stated. Distinct from `UnknownParameter`: the name is known and the value is
    /// missing, so saying the name is unknown sends a caller looking for a typo.
    RequiredParameterUnstated,
    /// A file whose name a declared identity pattern could not parse. Refused by name rather
    /// than skipped, because a file that vanishes from the denominator is a silent exclusion.
    TrialIdentityUnparsed,
    /// More than one column in a file looks like a force channel while the caller declared
    /// one. A quantity taken over the whole system, read from one plate of two, is wrong by
    /// roughly a factor of two and nothing in the record would say so.
    AmbiguousForceChannels,
    /// The plate reported a reading its own levelling makes uninterpretable.
    PlateNotLevel,
    /// A document declaring a schema this build does not implement. Distinct from every
    /// other code here: the remedy is a newer plateforce, not a different request.
    SchemaUnsupported,
    /// Values a comparison paired that did not come from the same repetition. The trace is
    /// sound and the pairing is not, so a caller that reads this repairs the pairing rather
    /// than the data.
    ObservationsNotPaired,
    /// Two figures computed under conventions whose difference the literature has never
    /// characterised. Not a fault in the request or the data: the comparison itself has no
    /// published meaning, and supplying a number would invent one.
    ConventionsNotComparable,
    /// Fewer observations than the rule requires, reported with the count it had and the
    /// count it needs. Distinct from `TraceTooShort`, which is one recording being short
    /// rather than a group being small.
    NotEnoughObservations,
    /// A rule that reads another construct's answer, run where that construct produced none.
    /// Distinct from every code describing the request: the request was answerable and an
    /// earlier step declined, so the remedy is upstream of the rule that reports this.
    /// Distinct from `DecisionNotMade`, where the caller never chose the earlier rule at all.
    DependencyUnresolved,
    /// A file the reader could not open or could not get through. Distinct from
    /// `ColumnNotFound`, which is a file that was read and does not carry what was asked for:
    /// here nothing was read, so no column could have been missing. It is the one code whose
    /// remedy is the path rather than the request or the data, and it takes its own exit
    /// status for that reason.
    FileNotRead,
}

/// Exit status for a refusal, from `sysexits.h`, which is the convention every workflow
/// manager already reads.
///
/// The match takes no wildcard arm, so a new code has to be ruled on rather than falling
/// through to whichever status happened to be last.
pub fn exit_code(code: RefusalCode) -> i32 {
    match code {
        RefusalCode::NoCrossing
        | RefusalCode::CollapsedBand
        | RefusalCode::TraceTooShort
        | RefusalCode::ColumnNotFound
        | RefusalCode::TrialIdentityUnparsed
        | RefusalCode::AmbiguousForceChannels
        | RefusalCode::SchemaUnsupported
        | RefusalCode::ObservationsNotPaired
        | RefusalCode::NotEnoughObservations
        | RefusalCode::DependencyUnresolved => 65,
        RefusalCode::MethodNotImplemented
        | RefusalCode::UnknownParameter
        | RefusalCode::ParameterNotFinite
        | RefusalCode::ValueNotAccepted
        | RefusalCode::SentinelConventionUnknown
        | RefusalCode::DecisionNotMade
        | RefusalCode::RequiredParameterUnstated
        | RefusalCode::PlateNotLevel
        | RefusalCode::ConventionsNotComparable => 64,
        // EX_NOINPUT. A workflow manager that retries on bad data and stops on a missing
        // file cannot tell the two apart while they share a status.
        RefusalCode::FileNotRead => 66,
        RefusalCode::RegistryInvalid => 78,
    }
}

impl RefusalCode {
    /// The code as it is written on the wire, for a caller that needs the text rather than
    /// the value. Matched exhaustively, so a new code cannot reach a surface unnamed.
    pub fn wire_name(self) -> &'static str {
        match self {
            RefusalCode::NoCrossing => "no_crossing",
            RefusalCode::CollapsedBand => "collapsed_band",
            RefusalCode::MethodNotImplemented => "method_not_implemented",
            RefusalCode::UnknownParameter => "unknown_parameter",
            RefusalCode::ParameterNotFinite => "parameter_not_finite",
            RefusalCode::ValueNotAccepted => "value_not_accepted",
            RefusalCode::TraceTooShort => "trace_too_short",
            RefusalCode::ColumnNotFound => "column_not_found",
            RefusalCode::SentinelConventionUnknown => "sentinel_convention_unknown",
            RefusalCode::RegistryInvalid => "registry_invalid",
            RefusalCode::DecisionNotMade => "decision_not_made",
            RefusalCode::RequiredParameterUnstated => "required_parameter_unstated",
            RefusalCode::TrialIdentityUnparsed => "trial_identity_unparsed",
            RefusalCode::AmbiguousForceChannels => "ambiguous_force_channels",
            RefusalCode::PlateNotLevel => "plate_not_level",
            RefusalCode::SchemaUnsupported => "schema_unsupported",
            RefusalCode::ObservationsNotPaired => "observations_not_paired",
            RefusalCode::ConventionsNotComparable => "conventions_not_comparable",
            RefusalCode::NotEnoughObservations => "not_enough_observations",
            RefusalCode::DependencyUnresolved => "dependency_unresolved",
            RefusalCode::FileNotRead => "file_not_read",
        }
    }
}

/// Which code a failed read is, decided once here so a batch row and a single-trial document
/// cannot answer one failure two ways.
impl From<&crate::signal::TrialError> for RefusalCode {
    fn from(error: &crate::signal::TrialError) -> Self {
        use crate::signal::TrialError;
        match error {
            TrialError::Empty | TrialError::EpochTooLong { .. } => RefusalCode::TraceTooShort,
            TrialError::BadSampleRate(_) => RefusalCode::ValueNotAccepted,
            TrialError::NoCrossing { .. } => RefusalCode::NoCrossing,
            TrialError::CollapsedBand { .. } => RefusalCode::CollapsedBand,
        }
    }
}

impl From<&crate::read::ReadError> for RefusalCode {
    fn from(error: &crate::read::ReadError) -> Self {
        use crate::read::ReadError;
        match error {
            ReadError::ColumnMissing { .. } | ReadError::NoRows { .. } => {
                RefusalCode::ColumnNotFound
            }
            // A file that could not be opened carries no columns, so reporting it as a
            // missing column sends a caller to change an index that was never read.
            ReadError::Io { .. } => RefusalCode::FileNotRead,
            ReadError::NotANumber { .. } => RefusalCode::ParameterNotFinite,
            ReadError::Trace(inner) => RefusalCode::from(inner),
        }
    }
}

/// Marks the pipeline case of `MethodNotImplemented` and carries the population it counts
/// over, so the sentence names a denominator rather than a bare list.
pub(crate) const PRESETS_CARRIED: &str = "presets_this_registry_carries";

/// Marks the case where a name was written where a rule goes and the step it would fill is
/// not known, so the alternatives are every rule rather than one step's.
pub(crate) const RULES_THIS_BUILD_RUNS: &str = "rules_this_build_runs";

/// A declined result, carrying what a caller branches on and the sentence a person reads.
///
/// `message` has no public constructor path of its own: every way of building a `Refusal`
/// generates it from the other fields, so two surfaces cannot describe one failure two ways.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: RefusalCode,
    /// The rule that declined, named as the registry names it.
    pub method_id: String,
    /// The construct the refusal happened under, named as the registry names it:
    /// `system_weight`, `movement_onset`, `takeoff`. None when the refusal is not about a
    /// step.
    ///
    /// The registry declares these and declares no `weighing` or `onset`, so a caller
    /// handed one of those has a word it cannot look up. The two vocabularies collide on
    /// `takeoff`, so the string is pinned to the registry's.
    pub slot: Option<String>,
    pub parameter: Option<String>,
    pub value: Option<f64>,
    /// The declined value where the parameter's values are names rather than numbers, in
    /// the registry's own spelling for them.
    ///
    /// A parameter whose values are named alternatives has no number to put in `value`. The
    /// registry files these as `NamedValue.key`, and this is that key.
    #[serde(default)]
    pub named_value: Option<String>,
    /// Everything else the rule read while declining. Ordered, so the sentence is stable
    /// across runs.
    pub detail: BTreeMap<String, f64>,
    /// What the caller could have asked for instead.
    pub available: Vec<String>,
    message: String,
}

impl Refusal {
    pub(crate) fn build(
        code: RefusalCode,
        method_id: impl Into<String>,
        parameter: Option<String>,
        value: Option<f64>,
        detail: BTreeMap<String, f64>,
        available: Vec<String>,
    ) -> Self {
        let mut refusal = Self {
            code,
            method_id: method_id.into(),
            slot: None,
            parameter,
            value,
            named_value: None,
            detail,
            available,
            message: String::new(),
        };
        refusal.regenerate();
        refusal
    }

    pub(crate) fn regenerate(&mut self) {
        self.message = sentence(
            self.code,
            &self.method_id,
            self.slot.as_deref(),
            self.parameter.as_deref(),
            self.value,
            self.named_value.as_deref(),
            &self.detail,
            &self.available,
        );
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> i32 {
        exit_code(self.code)
    }

    /// Names the construct the refusal happened under. Kept separate from construction because a
    /// core rule does not know which step a caller bound it to.
    pub fn in_slot(mut self, slot: impl Into<String>) -> Self {
        self.slot = Some(slot.into());
        self.regenerate();
        self
    }

    /// Restamps the id the refusal is reported under, and regenerates the sentence so the
    /// two cannot disagree. One rule reached under two ids reports the same name whether it
    /// works or declines.
    pub fn under(mut self, method_id: impl Into<String>) -> Self {
        self.method_id = method_id.into();
        self.regenerate();
        self
    }

    pub fn no_crossing(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
        search_bound_seconds: f64,
    ) -> Self {
        Self::build(
            RefusalCode::NoCrossing,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::from([("search_bound_seconds".to_string(), search_bound_seconds)]),
            Vec::new(),
        )
    }

    pub fn collapsed_band(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
        dispersion_newtons: f64,
        threshold_newtons: f64,
    ) -> Self {
        Self::build(
            RefusalCode::CollapsedBand,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::from([
                ("dispersion_newtons".to_string(), dispersion_newtons),
                ("threshold_newtons".to_string(), threshold_newtons),
            ]),
            Vec::new(),
        )
    }

    pub fn empty_trace(method_id: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::TraceTooShort,
            method_id,
            None,
            None,
            BTreeMap::from([("sample_count".to_string(), 0.0)]),
            Vec::new(),
        )
    }

    /// A file that could not be opened or could not be got through, named with the reason
    /// the operating system or the reader gave.
    pub fn file_not_read(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::FileNotRead,
            "",
            Some(path.into()),
            None,
            BTreeMap::new(),
            vec![reason.into()],
        )
    }

    /// A span that selects none of the recording, named with both of its ends.
    ///
    /// The same code as a trace with no samples, because to the rule they are one answer:
    /// the samples it needs are not there. The remedy is the rule that placed the span, and
    /// both ends are numbers so a caller can see which one to move.
    pub fn span_selects_no_samples(method_id: impl Into<String>, start: usize, end: usize) -> Self {
        Self::build(
            RefusalCode::TraceTooShort,
            method_id,
            None,
            None,
            BTreeMap::from([
                ("span_start_sample".to_string(), start as f64),
                ("span_end_sample".to_string(), end as f64),
            ]),
            Vec::new(),
        )
    }

    pub fn epoch_does_not_fit(
        method_id: impl Into<String>,
        requested_seconds: f64,
        start_seconds: f64,
        available_seconds: f64,
    ) -> Self {
        Self::build(
            RefusalCode::TraceTooShort,
            method_id,
            None,
            None,
            BTreeMap::from([
                ("requested_seconds".to_string(), requested_seconds),
                ("start_seconds".to_string(), start_seconds),
                ("available_seconds".to_string(), available_seconds),
            ]),
            Vec::new(),
        )
    }

    pub fn parameter_not_finite(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
    ) -> Self {
        Self::build(
            RefusalCode::ParameterNotFinite,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::new(),
            Vec::new(),
        )
    }

    /// A number the parameter will not take, with what it takes. `takes` is written as a
    /// reader would say it, because a range is not a list of ids.
    pub fn value_not_accepted(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        value: f64,
        takes: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::ValueNotAccepted,
            method_id,
            Some(parameter.into()),
            Some(value),
            BTreeMap::new(),
            takes,
        )
    }

    /// A name the parameter will not take, with the names it does take.
    ///
    /// The same code as the numeric form, because the fault is the same one: the rule
    /// understood the request and will not do it. The value being a name, it arrives in
    /// `named_value` rather than in `value`.
    pub fn name_not_accepted(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        chosen: impl Into<String>,
        takes: Vec<String>,
    ) -> Self {
        let mut refusal = Self::build(
            RefusalCode::ValueNotAccepted,
            method_id,
            Some(parameter.into()),
            None,
            BTreeMap::new(),
            takes,
        );
        refusal.named_value = Some(chosen.into());
        refusal.regenerate();
        refusal
    }

    /// A rule that reads another construct's answer, run where that construct produced none.
    ///
    /// `needs` names those constructs as the registry names them, so a caller reading this
    /// knows which step to repair rather than which rule to blame.
    pub fn dependency_unresolved(method_id: impl Into<String>, needs: Vec<String>) -> Self {
        Self::build(
            RefusalCode::DependencyUnresolved,
            method_id,
            None,
            None,
            BTreeMap::new(),
            needs,
        )
    }

    /// A rule that read the trace, found candidates, and found none of them qualifying.
    ///
    /// The same code as a rule that found no candidate at all, because to a caller both are
    /// one answer: this rule places nothing on this recording. `read` carries the counts and
    /// the floors it compared against, every one of them a number rather than a figure
    /// inside the sentence.
    pub fn nothing_qualified(
        method_id: impl Into<String>,
        candidates: usize,
        read: BTreeMap<String, f64>,
    ) -> Self {
        let mut detail = read;
        detail.insert("candidates_read".to_string(), candidates as f64);
        Self::build(
            RefusalCode::NoCrossing,
            method_id,
            None,
            None,
            detail,
            Vec::new(),
        )
    }

    pub fn method_not_implemented(
        method_id: impl Into<String>,
        slot: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        let slot = slot.into();
        Self::build(
            RefusalCode::MethodNotImplemented,
            method_id,
            None,
            None,
            BTreeMap::new(),
            available,
        )
        .in_slot(slot)
    }

    /// A document named a construct this build runs no step for.
    ///
    /// Shares `MethodNotImplemented` rather than taking a code of its own: the class is the
    /// same, a request asking for something not on offer.
    pub fn construct_not_on_the_path(
        construct_id: impl Into<String>,
        constructs_this_build_runs: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::MethodNotImplemented,
            construct_id,
            None,
            None,
            BTreeMap::new(),
            constructs_this_build_runs,
        )
    }

    /// A name written where a rule goes that answers to no rule, with the rules it could have
    /// named.
    ///
    /// Separate from `method_not_implemented` because that one knows the step and names its
    /// rules. Where a rule's step is read off the rule, an unresolved name has no step, and
    /// the sentence there attributed every rule in the build to a step it had invented.
    ///
    /// The count rides in `detail` for the reason `preset_not_shipped` puts one there: the
    /// sentence reports its population with a denominator, and a caller tells this case from
    /// its siblings under the same code by a field rather than by the prose.
    pub fn name_answers_to_no_rule(
        written: impl Into<String>,
        rules_this_build_runs: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::MethodNotImplemented,
            written,
            None,
            None,
            BTreeMap::from([(
                RULES_THIS_BUILD_RUNS.to_string(),
                rules_this_build_runs.len() as f64,
            )]),
            rules_this_build_runs,
        )
    }

    /// A named published pipeline this registry does not carry, with the ones it does.
    ///
    /// Shares `MethodNotImplemented` for the reason `construct_not_on_the_path` does.
    ///
    /// The count rides in `detail` so the sentence reports the population with its
    /// denominator, and so a caller tells this case from the two others under the same code
    /// by a field rather than by the sentence.
    pub fn preset_not_shipped(preset_id: impl Into<String>, shipped: Vec<String>) -> Self {
        Self::build(
            RefusalCode::MethodNotImplemented,
            preset_id,
            None,
            None,
            BTreeMap::from([(PRESETS_CARRIED.to_string(), shipped.len() as f64)]),
            shipped,
        )
    }

    pub fn unknown_parameter(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::UnknownParameter,
            method_id,
            Some(parameter.into()),
            None,
            BTreeMap::new(),
            available,
        )
    }

    /// A sweep axis the request it was asked of does not carry.
    ///
    /// The same code as a rule handed a name it does not read: a name was passed, nothing
    /// reads it, and the names that are read are listed. `MethodNotImplemented` would say
    /// the build runs no such step, which is false for a construct the build runs a rule for
    /// and this request did not name.
    ///
    /// `axes_offered` is the denominator the sentence quotes, and it tells this form apart
    /// from a rule declining on a parameter name.
    pub fn axis_not_in_this_request(axis: impl Into<String>, offered: Vec<String>) -> Self {
        Self::build(
            RefusalCode::UnknownParameter,
            "",
            Some(axis.into()),
            None,
            BTreeMap::from([("axes_offered".to_string(), offered.len() as f64)]),
            offered,
        )
    }

    /// A sweep axis naming a step and nothing to compare along it.
    ///
    /// The same code as a required parameter nobody stated, because the fault is the same
    /// one: the name is known and what belongs with it is missing. `UnknownParameter` would
    /// say the name is not read, which sends a caller looking for a typo in a step this
    /// request does carry.
    ///
    /// `alternatives_named` is the denominator the sentence quotes, and it tells this form
    /// apart from a rule whose registry row publishes no default.
    pub fn sweep_axis_states_no_alternative(axis: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::RequiredParameterUnstated,
            "",
            Some(axis.into()),
            None,
            BTreeMap::from([("alternatives_named".to_string(), 0.0)]),
            Vec::new(),
        )
    }

    pub fn column_not_found(column: impl Into<String>, available: Vec<String>) -> Self {
        Self::build(
            RefusalCode::ColumnNotFound,
            "",
            Some(column.into()),
            None,
            BTreeMap::new(),
            available,
        )
    }

    pub fn sentinel_convention_unknown(
        convention: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        Self::build(
            RefusalCode::SentinelConventionUnknown,
            "",
            Some(convention.into()),
            None,
            BTreeMap::new(),
            available,
        )
    }

    pub fn registry_invalid(detail: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::RegistryInvalid,
            "",
            Some(detail.into()),
            None,
            BTreeMap::new(),
            Vec::new(),
        )
    }

    /// A document from a version this build does not read.
    ///
    /// `available` carries the schema this build does implement, which is the one fact that
    /// tells a reader whether to upgrade plateforce or to have written a different file.
    pub fn schema_unsupported(declared: impl Into<String>, implemented: impl Into<String>) -> Self {
        Self::build(
            RefusalCode::SchemaUnsupported,
            "",
            Some(declared.into()),
            None,
            BTreeMap::new(),
            vec![implemented.into()],
        )
    }

    /// Values a comparison paired that did not come from the same repetition, named so a
    /// caller can see which pairing to repair.
    pub fn observations_not_paired(method_id: impl Into<String>, pairs: Vec<String>) -> Self {
        Self::build(
            RefusalCode::ObservationsNotPaired,
            method_id,
            None,
            None,
            BTreeMap::new(),
            pairs,
        )
    }

    /// Two conventions whose difference no published figure describes. Both are named,
    /// because the pair is the fact rather than either one of them.
    pub fn conventions_not_comparable(
        method_id: impl Into<String>,
        left: impl Into<String>,
        right: impl Into<String>,
    ) -> Self {
        Self::build(
            RefusalCode::ConventionsNotComparable,
            method_id,
            None,
            None,
            BTreeMap::new(),
            vec![left.into(), right.into()],
        )
    }

    /// Fewer observations than a rule requires, carrying both counts as numbers so a caller
    /// branches on them rather than reading them out of the sentence.
    pub fn not_enough_observations(method_id: impl Into<String>, had: usize, needs: usize) -> Self {
        Self::build(
            RefusalCode::NotEnoughObservations,
            method_id,
            None,
            None,
            BTreeMap::from([
                ("had".to_string(), had as f64),
                ("needs".to_string(), needs as f64),
            ]),
            Vec::new(),
        )
    }

    /// A parameter the registry marks required with no default, left unstated.
    pub fn required_parameter_unstated(
        method_id: impl Into<String>,
        parameter: impl Into<String>,
    ) -> Self {
        Self::build(
            RefusalCode::RequiredParameterUnstated,
            method_id,
            Some(parameter.into()),
            None,
            BTreeMap::new(),
            Vec::new(),
        )
    }

    /// A file name the declared pattern could not parse, named rather than skipped.
    pub fn trial_identity_unparsed(
        file_name: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        Self::build(
            RefusalCode::TrialIdentityUnparsed,
            "",
            Some(file_name.into()),
            None,
            BTreeMap::new(),
            vec![template.into()],
        )
    }

    /// More than one column looks like a force channel. `available` names what would resolve
    /// it.
    pub fn ambiguous_force_channels(force_like_columns: usize, resolves: Vec<String>) -> Self {
        Self::build(
            RefusalCode::AmbiguousForceChannels,
            "",
            None,
            None,
            BTreeMap::from([("force_like_columns".to_string(), force_like_columns as f64)]),
            resolves,
        )
    }

    /// A result was asked for while a choice the registry forces is still open. `available`
    /// carries the constructs whose choice is outstanding.
    pub fn decision_not_made(artifact: impl Into<String>, outstanding: Vec<String>) -> Self {
        Self::build(
            RefusalCode::DecisionNotMade,
            "",
            Some(artifact.into()),
            None,
            BTreeMap::new(),
            outstanding,
        )
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Refusal {}

/// The one place a refusal becomes a sentence.
#[allow(clippy::too_many_arguments)]
fn sentence(
    code: RefusalCode,
    method_id: &str,
    slot: Option<&str>,
    parameter: Option<&str>,
    value: Option<f64>,
    named_value: Option<&str>,
    detail: &BTreeMap<String, f64>,
    available: &[String],
) -> String {
    let named = |key: &str| detail.get(key).copied().unwrap_or(f64::NAN);
    let subject = match (parameter, value, named_value) {
        (Some(name), Some(number), _) => format!("{method_id}({name} = {number})"),
        (Some(name), None, Some(chosen)) => format!("{method_id}({name} = {chosen})"),
        _ => method_id.to_string(),
    };
    match code {
        // A bounded search that finds nothing has no candidates to count, so the two forms
        // under this code are told apart by the detail.
        RefusalCode::NoCrossing if detail.contains_key("candidates_read") => {
            let read: Vec<String> = detail
                .iter()
                .filter(|(key, _)| key.as_str() != "candidates_read")
                .map(|(key, value)| format!("{key} = {value}"))
                .collect();
            format!(
                "{subject} read {} candidate{} and none of them qualifies against {}",
                named("candidates_read"),
                if named("candidates_read") == 1.0 { "" } else { "s" },
                if read.is_empty() {
                    "what it looks for".to_string()
                } else {
                    read.join(", ")
                }
            )
        }
        RefusalCode::NoCrossing => format!(
            "{subject} found no crossing within the search bound of {} s",
            named("search_bound_seconds")
        ),
        RefusalCode::FileNotRead => format!(
            "{} could not be read: {}",
            parameter.unwrap_or("that path"),
            available.first().map(String::as_str).unwrap_or("no detail")
        ),
        RefusalCode::DependencyUnresolved => format!(
            "{subject} reads what {} placed, and that step placed nothing",
            if available.is_empty() {
                "an earlier step".to_string()
            } else {
                available.join(" and ")
            }
        ),
        RefusalCode::CollapsedBand => format!(
            "{subject} has no band to search: dispersion is {} N and the threshold falls at {} N",
            named("dispersion_newtons"),
            named("threshold_newtons")
        ),
        RefusalCode::TraceTooShort if detail.contains_key("requested_seconds") => format!(
            "weighing epoch of {} s starting at {} s does not fit in a trace of {} s",
            named("requested_seconds"),
            named("start_seconds"),
            named("available_seconds")
        ),
        RefusalCode::TraceTooShort if detail.contains_key("span_start_sample") => format!(
            "{subject} was given samples {} to {}, which selects none of the recording",
            named("span_start_sample"),
            named("span_end_sample")
        ),
        RefusalCode::TraceTooShort => "trace is empty".to_string(),
        RefusalCode::ObservationsNotPaired => format!(
            "{subject} compares values from different repetitions, and the values it paired are {available:?}"
        ),
        RefusalCode::ConventionsNotComparable => format!(
            "no published figure describes how {} and {} differ, so their agreement has no meaning to report",
            available.first().map(String::as_str).unwrap_or("one convention"),
            available.get(1).map(String::as_str).unwrap_or("the other")
        ),
        RefusalCode::NotEnoughObservations => format!(
            "{subject} needs {} observations and this group has {}",
            named("needs"),
            named("had")
        ),
        RefusalCode::ParameterNotFinite => format!(
            "{} must be a finite number, got {}",
            parameter.unwrap_or("the parameter"),
            value.unwrap_or(f64::NAN)
        ),
        RefusalCode::ValueNotAccepted => {
            let takes = if available.is_empty() {
                String::new()
            } else {
                format!(": it takes {}", available.join(", "))
            };
            match (value, named_value) {
                (Some(number), _) => format!(
                    "{} does not accept {number}{takes}",
                    parameter.unwrap_or("the parameter")
                ),
                (None, Some(chosen)) => format!(
                    "{} does not accept {chosen}{takes}",
                    parameter.unwrap_or("the parameter")
                ),
                (None, None) => format!(
                    "{} was given a value it does not accept{takes}",
                    parameter.unwrap_or("the parameter")
                ),
            }
        }
        RefusalCode::MethodNotImplemented if detail.contains_key(PRESETS_CARRIED) => format!(
            "'{method_id}' is not a published pipeline this registry carries, and the {} it carries {} {available:?}",
            named(PRESETS_CARRIED),
            if named(PRESETS_CARRIED) == 1.0 { "is" } else { "are" }
        ),
        RefusalCode::MethodNotImplemented if detail.contains_key(RULES_THIS_BUILD_RUNS) => {
            format!(
                "'{method_id}' answers to no rule, and the {} rules this build runs are {available:?}",
                named(RULES_THIS_BUILD_RUNS)
            )
        }
        RefusalCode::MethodNotImplemented => match slot {
            Some(step) => format!(
                "'{method_id}' was passed as the {step} method, and the rules for that step are {available:?}"
            ),
            None => format!(
                "'{method_id}' is not a step this analysis runs, and the steps it runs are {available:?}"
            ),
        },
        // No rule read the axis, so the sentence names the request's axes rather than a
        // rule's parameters.
        RefusalCode::UnknownParameter if detail.contains_key("axes_offered") => format!(
            "'{}' was passed as a sweep axis, and the {} axes this sweep can vary are {available:?}",
            parameter.unwrap_or("that name"),
            named("axes_offered")
        ),
        RefusalCode::UnknownParameter => format!(
            "{method_id} does not read {}, and the names it reads are {available:?}",
            parameter.unwrap_or("that name")
        ),
        RefusalCode::ColumnNotFound => format!(
            "no column named {}, and the file carries {available:?}",
            parameter.unwrap_or("that")
        ),
        RefusalCode::SentinelConventionUnknown => format!(
            "{} is not a sentinel convention this reader applies, and it applies {available:?}",
            parameter.unwrap_or("that")
        ),
        RefusalCode::SchemaUnsupported => format!(
            "this file declares {}, and this plateforce reads {}",
            parameter.unwrap_or("a schema"),
            available.first().map(String::as_str).unwrap_or("another")
        ),
        RefusalCode::RegistryInvalid => format!(
            "the registry does not load: {}",
            parameter.unwrap_or("no detail")
        ),
        RefusalCode::DecisionNotMade => format!(
            "{} states every choice behind a number, and {available:?} {} still open",
            parameter.unwrap_or("this artifact"),
            if available.len() == 1 { "is" } else { "are" }
        ),
        // No rule published this one, so the sentence names what the sweep was asked to
        // compare rather than a registry row that would have carried a default.
        RefusalCode::RequiredParameterUnstated if detail.contains_key("alternatives_named") => {
            format!(
                "'{}' was passed as a sweep axis and names no alternative to compare along it",
                parameter.unwrap_or("that name")
            )
        }
        RefusalCode::RequiredParameterUnstated => format!(
            "{method_id} publishes no default for {}, so it has to be stated",
            parameter.unwrap_or("that name")
        ),
        RefusalCode::TrialIdentityUnparsed => format!(
            "{} does not match the declared trial-identity pattern {}",
            parameter.unwrap_or("that file name"),
            available.first().map(String::as_str).unwrap_or("that was set")
        ),
        RefusalCode::PlateNotLevel => format!(
            "{} reports the plate out of level, so a vertical force read from it is not vertical",
            parameter.unwrap_or("the acquisition record")
        ),
        RefusalCode::AmbiguousForceChannels => format!(
            "{} columns in this file look like force channels, so a quantity taken over the \
             whole system cannot be read from one of them without declaring the file \
             single-plate",
            named("force_like_columns")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a refusal costs to carry, printed rather than asserted against a figure that
    /// would go stale the next time a field is added.
    #[test]
    fn a_refusal_reports_what_it_costs_to_carry() {
        println!(
            "Refusal is {} bytes, Box<Refusal> is {}",
            std::mem::size_of::<Refusal>(),
            std::mem::size_of::<Box<Refusal>>()
        );
        // The threshold the error-size lint holds a `Result` to, which is why every function
        // returning one of these boxes it.
        assert!(std::mem::size_of::<Refusal>() > 128);
    }

    #[test]
    fn the_name_a_caller_reads_is_the_name_on_the_wire() {
        for code in RefusalCode::ALL {
            let written = serde_json::to_string(code).unwrap();
            assert_eq!(
                format!("\"{}\"", code.wire_name()),
                written,
                "{code:?} is written one way and named another"
            );
        }
    }

    #[test]
    fn an_agreement_that_cannot_be_reported_says_what_it_needed() {
        let short = Refusal::not_enough_observations("agreement.limits_of_agreement", 2, 5);
        assert_eq!(short.code, RefusalCode::NotEnoughObservations);
        // Both counts are numbers a caller reads, not figures inside the sentence.
        assert_eq!(short.detail["had"], 2.0);
        assert_eq!(short.detail["needs"], 5.0);
        assert!(short.message().contains('5') && short.message().contains('2'));

        let conventions = Refusal::conventions_not_comparable(
            "agreement.limits_of_agreement",
            "jumpheight.takeoff.impulse_momentum",
            "jumpheight.takeoff.flight_time",
        );
        assert_eq!(conventions.code, RefusalCode::ConventionsNotComparable);
        assert_eq!(conventions.available.len(), 2);

        let unpaired = Refusal::observations_not_paired(
            "agreement.limits_of_agreement",
            vec!["trial 1".to_string(), "trial 4".to_string()],
        );
        assert_eq!(unpaired.code, RefusalCode::ObservationsNotPaired);
        assert_eq!(unpaired.available.len(), 2);
    }

    #[test]
    fn a_failed_read_answers_with_one_code() {
        use crate::read::ReadError;
        use crate::signal::TrialError;

        assert_eq!(
            RefusalCode::from(&TrialError::Empty),
            RefusalCode::TraceTooShort
        );
        assert_eq!(
            RefusalCode::from(&TrialError::BadSampleRate(0.0)),
            RefusalCode::ValueNotAccepted
        );
        // A cell that is not a number is the other code, and the two are told apart here so
        // the pair cannot drift into one.
        assert_eq!(
            RefusalCode::from(&ReadError::NotANumber {
                line_number: 1,
                column_index: 0,
                text: "n/a".to_string(),
            }),
            RefusalCode::ParameterNotFinite
        );
        // A trace failure reached through a read is the same code as the trace failure
        // itself.
        assert_eq!(
            RefusalCode::from(&ReadError::Trace(TrialError::Empty)),
            RefusalCode::from(&TrialError::Empty)
        );
    }

    #[test]
    fn every_code_carries_an_exit_status() {
        let codes = [
            (RefusalCode::NoCrossing, 65),
            (RefusalCode::CollapsedBand, 65),
            (RefusalCode::TraceTooShort, 65),
            (RefusalCode::ColumnNotFound, 65),
            (RefusalCode::MethodNotImplemented, 64),
            (RefusalCode::UnknownParameter, 64),
            (RefusalCode::ParameterNotFinite, 64),
            (RefusalCode::SentinelConventionUnknown, 64),
            (RefusalCode::DecisionNotMade, 64),
            (RefusalCode::RegistryInvalid, 78),
            (RefusalCode::RequiredParameterUnstated, 64),
            (RefusalCode::TrialIdentityUnparsed, 65),
            (RefusalCode::AmbiguousForceChannels, 65),
        ];
        println!("{} refusal codes", codes.len());
        assert_eq!(codes.len(), 13);
        for (code, expected) in codes {
            assert_eq!(exit_code(code), expected, "{code:?}");
        }
    }

    #[test]
    fn the_wire_spelling_of_every_code_is_snake_case() {
        let spellings = [
            (RefusalCode::NoCrossing, "\"no_crossing\""),
            (RefusalCode::CollapsedBand, "\"collapsed_band\""),
            (
                RefusalCode::MethodNotImplemented,
                "\"method_not_implemented\"",
            ),
            (RefusalCode::UnknownParameter, "\"unknown_parameter\""),
            (RefusalCode::ParameterNotFinite, "\"parameter_not_finite\""),
            (RefusalCode::TraceTooShort, "\"trace_too_short\""),
            (RefusalCode::ColumnNotFound, "\"column_not_found\""),
            (
                RefusalCode::SentinelConventionUnknown,
                "\"sentinel_convention_unknown\"",
            ),
            (RefusalCode::RegistryInvalid, "\"registry_invalid\""),
            (RefusalCode::DecisionNotMade, "\"decision_not_made\""),
            (
                RefusalCode::RequiredParameterUnstated,
                "\"required_parameter_unstated\"",
            ),
            (
                RefusalCode::TrialIdentityUnparsed,
                "\"trial_identity_unparsed\"",
            ),
            (
                RefusalCode::AmbiguousForceChannels,
                "\"ambiguous_force_channels\"",
            ),
        ];
        for (code, expected) in spellings {
            assert_eq!(serde_json::to_string(&code).unwrap(), expected);
        }
    }

    #[test]
    fn the_shipped_sentences_are_reproduced_unchanged() {
        assert_eq!(
            Refusal::no_crossing("onset.threshold.noise_relative", "k", 5.0, 2.5).message(),
            "onset.threshold.noise_relative(k = 5) found no crossing within the search bound of 2.5 s"
        );
        assert_eq!(
            Refusal::collapsed_band("onset.threshold.noise_relative", "k", 5.0, 0.4, 812.1)
                .message(),
            "onset.threshold.noise_relative(k = 5) has no band to search: dispersion is 0.4 N and the threshold falls at 812.1 N"
        );
        assert_eq!(
            Refusal::epoch_does_not_fit("bwepoch.fixed_window", 2.0, 1.5, 3.0).message(),
            "weighing epoch of 2 s starting at 1.5 s does not fit in a trace of 3 s"
        );
        assert_eq!(
            Refusal::empty_trace("bwepoch.fixed_window").message(),
            "trace is empty"
        );
    }

    #[test]
    fn restamping_the_id_rewrites_the_sentence_rather_than_leaving_the_old_one() {
        let refused = Refusal::no_crossing("onset.threshold.percent_bodyweight", "k", 5.0, 2.5)
            .under("onset.threshold.relative_to_system_weight");
        assert_eq!(
            refused.method_id,
            "onset.threshold.relative_to_system_weight"
        );
        assert!(refused
            .message()
            .starts_with("onset.threshold.relative_to_system_weight(k = 5)"));
        assert!(!refused.message().contains("percent_bodyweight"));
    }

    #[test]
    fn a_refusal_round_trips_through_its_wire_form() {
        let refused =
            Refusal::no_crossing("onset.threshold.noise_relative", "k", 5.0, 2.5).in_slot("onset");
        let wire = serde_json::to_string(&refused).unwrap();
        assert_eq!(
            serde_json::from_str::<Refusal>(&wire).unwrap(),
            refused,
            "{wire}"
        );
        assert!(wire.contains("\"slot\":\"onset\""));
        assert!(wire.contains("\"message\":"));
    }

    /// The value a rule declined on has to come back the same double it went out as.
    ///
    /// Asserted on the bits rather than with `==`, because the two doubles either side of a
    /// mis-parse compare equal under every approximate check and differ in the last place.
    /// `serde_json`'s writer emits the shortest string that round-trips and its parser was
    /// not correctly rounded, so the text on the wire is right and the number read back is
    /// wrong, which is invisible to anyone reading the file.
    #[test]
    fn a_declined_value_survives_the_wire_bit_for_bit() {
        let awkward = 10.106284223733105_f64;
        let refused = Refusal::no_crossing(
            "onset.threshold.noise_relative",
            "k",
            awkward,
            3.6408148087849357,
        );
        let read_back: Refusal = serde_json::from_str(&serde_json::to_string(&refused).unwrap())
            .expect("a refusal reads back");

        assert_eq!(
            read_back.value.unwrap().to_bits(),
            awkward.to_bits(),
            "wrote {awkward:?}, read {:?}",
            read_back.value.unwrap()
        );
        assert_eq!(
            read_back.detail["search_bound_seconds"].to_bits(),
            3.6408148087849357_f64.to_bits()
        );
        assert_eq!(read_back.message(), refused.message());
    }

    /// `ALL` is what a manifest reports as this build's vocabulary, so a variant missing from
    /// it would be emittable and absent from the surface that claims to enumerate them.
    #[test]
    fn every_code_is_listed_once_in_all_and_carries_a_known_exit_status() {
        let spelled: BTreeMap<String, RefusalCode> = RefusalCode::ALL
            .iter()
            .map(|code| (serde_json::to_string(code).unwrap(), *code))
            .collect();
        assert_eq!(
            spelled.len(),
            RefusalCode::ALL.len(),
            "a code is listed twice in ALL"
        );

        for code in RefusalCode::ALL {
            assert!(
                matches!(exit_code(*code), 64 | 65 | 66 | 78),
                "{code:?} exits {}, which is not one of the four statuses this build uses",
                exit_code(*code)
            );
        }
    }

    /// The slot names a construct the registry declares. `weighing` and `onset` are the
    /// binding table's own words and resolve to nothing, and `takeoff` belongs to both
    /// vocabularies, so a caller could not tell which one it held.
    #[test]
    fn the_slot_names_a_construct_rather_than_a_binding_table_word() {
        let refused = Refusal::method_not_implemented(
            "onset.threshold.invented",
            "movement_onset",
            vec!["onset.threshold.noise_relative".to_string()],
        );
        assert_eq!(refused.slot.as_deref(), Some("movement_onset"));
        assert!(
            refused.message().contains("movement_onset"),
            "{}",
            refused.message()
        );
    }

    /// The remedy here is a newer plateforce, which no other code says, so the sentence
    /// names both versions rather than offering the caller an alternative to ask for.
    #[test]
    fn a_document_from_a_later_version_says_which_version_it_wants() {
        let refused =
            Refusal::schema_unsupported("plateforce.method-set/2", "plateforce.method-set/1");
        assert_eq!(refused.code, RefusalCode::SchemaUnsupported);
        assert_eq!(refused.exit_code(), 65);
        assert!(
            refused.message().contains("plateforce.method-set/2")
                && refused.message().contains("plateforce.method-set/1"),
            "{}",
            refused.message()
        );
    }

    #[test]
    fn a_construct_off_the_path_names_the_steps_the_build_does_run() {
        let refused = Refusal::construct_not_on_the_path(
            "braking_phase_start",
            vec![
                "system_weight".to_string(),
                "movement_onset".to_string(),
                "takeoff".to_string(),
            ],
        );
        assert_eq!(refused.code, RefusalCode::MethodNotImplemented);
        assert_eq!(refused.exit_code(), 64);
        assert!(
            refused.message().contains("braking_phase_start"),
            "{}",
            refused.message()
        );
        assert!(
            refused.message().contains("system_weight"),
            "{}",
            refused.message()
        );
        // The slotted reading of this code names a step; this one has none to name.
        assert!(refused.slot.is_none());
    }

    /// A parameter whose values are names has no number to decline on, so the name it
    /// declined is a field rather than prose inside the sentence.
    #[test]
    fn a_name_a_parameter_will_not_take_arrives_as_a_field() {
        let refused = Refusal::name_not_accepted(
            "onset.op.direction",
            "direction",
            "above_only",
            vec!["below_only".to_string(), "two_sided".to_string()],
        );
        assert_eq!(refused.code, RefusalCode::ValueNotAccepted);
        assert_eq!(refused.named_value.as_deref(), Some("above_only"));
        assert_eq!(refused.value, None);
        assert_eq!(refused.parameter.as_deref(), Some("direction"));
        assert_eq!(
            refused.message(),
            "direction does not accept above_only: it takes below_only, two_sided"
        );

        let read_back: Refusal =
            serde_json::from_str(&serde_json::to_string(&refused).unwrap()).unwrap();
        assert_eq!(read_back, refused);
    }

    /// A record written before this field existed still reads, because a refusal on a
    /// number carries no name and the absent field is that absence rather than a loss.
    #[test]
    fn a_refusal_written_without_a_named_value_still_reads() {
        let older = r#"{"code":"no_crossing","method_id":"onset.threshold.noise_relative",
            "slot":null,"parameter":"k","value":5.0,"detail":{"search_bound_seconds":2.5},
            "available":[],"message":"anything"}"#;
        let read: Refusal = serde_json::from_str(older).expect("an older refusal reads");
        assert_eq!(read.named_value, None);
        assert_eq!(read.code, RefusalCode::NoCrossing);
    }

    /// The remedy for this one is upstream, which no other code says: the request was
    /// answerable and an earlier step declined.
    #[test]
    fn a_rule_whose_earlier_step_placed_nothing_names_that_step() {
        let refused =
            Refusal::dependency_unresolved("force.peak.gross", vec!["analysis_window".to_string()]);
        assert_eq!(refused.code, RefusalCode::DependencyUnresolved);
        assert_eq!(refused.exit_code(), 65);
        assert_eq!(
            refused.message(),
            "force.peak.gross reads what analysis_window placed, and that step placed nothing"
        );
    }

    /// A rule that read candidates and rejected every one of them reports the count and
    /// the floors it compared against as numbers. The bounded search that finds no
    /// candidate at all keeps its own sentence, and the two are told apart by the detail.
    #[test]
    fn a_rule_that_rejected_every_candidate_reports_the_count_and_what_it_compared() {
        let refused = Refusal::nothing_qualified(
            "takeoff.threshold.landing_shape",
            3,
            BTreeMap::from([("landing_peak_floor_bodyweights".to_string(), 1.5)]),
        );
        assert_eq!(refused.code, RefusalCode::NoCrossing);
        assert_eq!(refused.detail["candidates_read"], 3.0);
        assert_eq!(refused.detail["landing_peak_floor_bodyweights"], 1.5);
        assert_eq!(
            refused.message(),
            "takeoff.threshold.landing_shape read 3 candidates and none of them qualifies \
             against landing_peak_floor_bodyweights = 1.5"
        );

        let bounded = Refusal::no_crossing("onset.threshold.noise_relative", "k", 5.0, 2.5);
        assert!(
            bounded.message().contains("found no crossing"),
            "{}",
            bounded.message()
        );
    }

    #[test]
    fn a_decision_left_open_names_what_is_outstanding() {
        let refused = Refusal::decision_not_made(
            "A methods paragraph",
            vec!["movement_onset".to_string(), "system_weight".to_string()],
        );
        assert_eq!(refused.code, RefusalCode::DecisionNotMade);
        assert_eq!(refused.exit_code(), 64);
        assert!(refused.message().contains("movement_onset"));
    }
}
