//! A spread nested inside an analysed result inherits that result's identity. A spread that
//! leaves on its own carries a document of its own, and that document is where the four
//! identity fields sit: which build and which registry produced the sweep.
//!
//! Every key set below is read off the serialised document rather than written down, so a
//! field added to either document is compared rather than remembered.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::document::{ResultDocument, SpreadDocument, TrialSource};
use plateforce_analysis::spread::{run as sweep, Axis, SpreadRequest, SpreadResponse};
use plateforce_analysis::{AnalysisRequest, MethodChoice, WeighingChoice};
use plateforce_core::Trial;

const SAMPLE_RATE_HZ: f64 = 1200.0;

/// What the schema reserves for identity, in the spelling `docs/schema.md` gives each one.
///
/// `registry_digest` names the files that were read whether or not anybody declared a
/// revision; `registry_version` is the revision a caller pinned and is absent when nobody
/// pinned one; `registry_declared_version` is the revision the registry names about itself,
/// which is what the data claims rather than what the caller cited; `plateforce_version` is
/// the build. Four questions, four fields.
///
/// `the_two_documents_spell_their_identity_the_same_way` asserts the sweep's document adds
/// exactly these and no others, and an assertion that a set equals a list is only as good as
/// the list.
const IDENTITY: [&str; 4] = [
    "plateforce_version",
    "registry_version",
    "registry_declared_version",
    "registry_digest",
];

/// A stand-in, deliberately not in the `content-` plus sixteen hex digits shape a real digest
/// prints in. What these guards read is the key and its spelling, never the value, and a
/// digest-shaped literal in a committed file is a provenance figure nobody checks:
/// `every_digest_in_prose_is_the_one_the_registry_answers` holds every one of them to the
/// registry's own answer.
const A_DIGEST_THIS_TEST_NEVER_READ: &str = "registry-digest-stand-in";

fn a_jump_that_lands() -> Trial {
    let mut force = vec![600.0; 1200];
    for (index, sample) in force.iter_mut().enumerate() {
        *sample += ((index % 17) as f64 - 8.0) * 0.4;
    }
    force.extend((0..240).map(|index| 600.0 - 220.0 * (index as f64 / 240.0)));
    force.extend((0..240).map(|index| 380.0 + 220.0 * (index as f64 / 240.0)));
    force.extend((0..660).map(|index| 600.0 + 900.0 * (index as f64 / 660.0)));
    force.extend(std::iter::repeat_n(0.0, 811));
    force.extend(std::iter::repeat_n(2400.0, 240));
    force.extend(std::iter::repeat_n(600.0, 600));
    Trial::new(force, SAMPLE_RATE_HZ).unwrap()
}

fn base() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            parameters: BTreeMap::from([("duration".to_string(), 0.8)]),
            ..Default::default()
        },
        onset: MethodChoice {
            method_id: "onset.threshold.noise_relative".into(),
            ..Default::default()
        },
        takeoff: MethodChoice {
            method_id: "takeoff.threshold.absolute_force".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn swept() -> SpreadResponse {
    let request = SpreadRequest {
        base: base(),
        axes: vec![Axis {
            slot: "onset".to_string(),
            parameter: Some("k".to_string()),
            values: vec![2.0, 5.0, 8.0],
            options: Vec::new(),
            method_ids: Vec::new(),
        }],
        quantity_key: "jump_height_from_takeoff_meters".into(),
        maximum_combinations: 512,
    };
    sweep(&a_jump_that_lands(), &request).expect("a known axis sweeps")
}

fn keys_of<T: serde::Serialize>(value: &T) -> BTreeSet<String> {
    serde_json::to_value(value)
        .expect("the document serialises")
        .as_object()
        .expect("the document is an object")
        .keys()
        .cloned()
        .collect()
}

/// The property, with the control that has to hold first: the sweep on its own says nothing
/// about what produced it.
#[test]
fn the_sweep_alone_names_no_build_and_the_document_around_it_does() {
    let response = swept();
    let bare = keys_of(&response);
    for name in IDENTITY {
        assert!(
            !bare.contains(name),
            "the sweep itself carries {name}, so this guard is not testing the document"
        );
    }

    let document = SpreadDocument::of(
        "0.1.0",
        &plateforce_core::provenance::RegistryStamp::unpinned(
            Some("2026-07-25".to_string()),
            Some(A_DIGEST_THIS_TEST_NEVER_READ.to_string()),
        )
        .pinned_to(Some("2026-07-25".to_string())),
        response,
    );
    let named = keys_of(&document);
    for name in IDENTITY {
        assert!(named.contains(name), "the document does not name {name}");
    }
}

/// The identity the sweep's document adds is the identity an analysed result already carries,
/// spelled the same way.
///
/// Both sides are read off serialised documents. A field renamed on one and not the other
/// fails here rather than reaching a reader as two vocabularies for one fact, which is the
/// defect one layer out from two implementations of one method.
#[test]
fn the_two_documents_spell_their_identity_the_same_way() {
    let response = swept();
    let added: BTreeSet<String> = keys_of(&SpreadDocument::of(
        "0.1.0",
        &plateforce_core::provenance::RegistryStamp::unpinned(
            None,
            Some(A_DIGEST_THIS_TEST_NEVER_READ.to_string()),
        ),
        response.clone(),
    ))
    .difference(&keys_of(&response))
    .cloned()
    .collect();

    assert_eq!(
        added,
        IDENTITY.iter().map(|name| name.to_string()).collect(),
        "the sweep's document added something other than the identity"
    );

    let analysed = keys_of(&ResultDocument {
        plateforce_version: "0.1.0".into(),
        trial: TrialSource {
            name: "trial".into(),
            rows_read: 0,
            samples_matching_the_convention: 0,
        },
        registry_version: None,
        registry_declared_version: None,
        registry_digest: None,
        acquisition: plateforce_core::Acquisition::default(),
        acquisition_complete: false,
        plate_profile: None,
        samples_carrying_no_number: 0,
        weighing_start_index: 0,
        weighing_end_index: 0,
        onset_index: None,
        takeoff_index: None,
        touchdown_index: None,
        metrics: Vec::new(),
        bound_methods: Vec::new(),
        bound_globals: Vec::new(),
        levels: plateforce_analysis::Levels {
            system_weight_newtons: None,
            weighing_standard_deviation_newtons: None,
            onset_band_lower_newtons: None,
            onset_band_upper_newtons: None,
            takeoff_threshold_newtons: None,
        },
        signals: Vec::new(),
        warnings: Vec::new(),
        refusals: Vec::new(),
        descriptions: BTreeMap::new(),
        spread: None,
    });
    for name in &added {
        assert!(
            analysed.contains(name),
            "the sweep's document names {name} and an analysed result does not"
        );
    }
}

/// Flattened rather than nested, so the fifteen keys a reader already reads stay where they
/// are. A reader of `spread_absolute` keeps reading it at the top level.
#[test]
fn wrapping_the_sweep_moved_none_of_its_own_keys() {
    let response = swept();
    let bare = keys_of(&response);
    let named = keys_of(&SpreadDocument::of(
        "0.1.0",
        &plateforce_core::provenance::RegistryStamp::none(),
        response,
    ));
    assert!(
        bare.is_subset(&named),
        "wrapping the sweep lost or renamed one of its own keys: {:?}",
        bare.difference(&named).collect::<Vec<_>>()
    );
}
