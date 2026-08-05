//! What a rule uses when nobody chooses has to be what the registry says it is.
//!
//! A fallback is a default with no paperwork. When the registry declares one and the code
//! carries a different value, a user who reads the entry and a user who runs the software
//! get different answers from the same named method, and the fingerprint reports whichever
//! the code holds. Both shapes of default are held to that: a quantity the registry states
//! under `default`, and a name it states under `default_key`.
//!
//! A name is the harder of the two, because a rule records one for two different reasons. It
//! may offer the choice and fall back, which is what a declared default describes. Or its own
//! identity may fix the value, the way `onset.threshold.last_within_band` takes the last
//! crossing and no other, and there the operator entry's default describes what some other
//! rule composing it does. The two are one word in the record, so this file separates them by
//! asking the software: state the registry's declared value and see whether the rule refuses
//! it.
//!
//! The registry is not linked by the binding layer, on purpose: it takes bound values and
//! knows nothing about where they came from. So this comparison lives here, in the one crate
//! that has both the rules and the registry.

use std::collections::{BTreeMap, BTreeSet};

use plateforce_analysis::{
    run, AnalysisRequest, Binding, MethodChoice, WeighingChoice, BINDINGS, ONSET_OPERATOR_IDS,
    TAKEOFF_OPERATOR_IDS,
};
use plateforce_core::STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED;
use plateforce_wasm::demo::synthetic_countermovement_jump;
use plateforce_wasm::registry_embed;

fn base_request() -> AnalysisRequest {
    AnalysisRequest {
        weighing: WeighingChoice {
            method_id: "bwepoch.fixed_window".into(),
            start_index: None,
            parameters: BTreeMap::from([("duration".to_string(), 1.0)]),
            options: BTreeMap::new(),
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
        touchdown_index: None,
        gravity_meters_per_second_squared: STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
        registry_backed_ids: Vec::new(),
        ..Default::default()
    }
}

/// A request reaching one rule in the slot its binding is filed under, carrying the names a
/// probe states on it.
///
/// One builder for the bare walk and for the probe, so a value stated through a path the walk
/// never took cannot read as dropped. A rule reached by construct id assigned to the takeoff
/// field instead left `run` refusing an id filed elsewhere and the walk stepping over it.
fn request_naming(binding: &Binding, options: BTreeMap<String, String>) -> AnalysisRequest {
    let mut request = base_request();
    let choice = MethodChoice {
        method_id: binding.id.to_string(),
        options: options.clone(),
        ..Default::default()
    };
    match binding.slot {
        "weighing" => {
            request.weighing.method_id = binding.id.to_string();
            request.weighing.parameters = BTreeMap::new();
            request.weighing.options = options;
        }
        "onset" => request.onset = choice,
        "takeoff" => request.takeoff = choice,
        construct if conditioning_constructs().contains(&construct) => {
            request.conditioning.insert(construct.to_string(), choice);
        }
        construct => {
            request.derived.insert(construct.to_string(), choice);
            // The window rules place what several of these read, so one is named beside the
            // rule under test. A rule that declines for want of it records nothing.
            request
                .derived
                .entry("analysis_window".to_string())
                .or_insert(MethodChoice {
                    method_id: "window_end.takeoff.detected".to_string(),
                    ..Default::default()
                });
        }
    }
    request
}

fn conditioning_constructs() -> Vec<&'static str> {
    plateforce_analysis::binding::conditioning_constructs()
}

/// The entries this build composes onto a landmark rule, taken from the engine's own lists
/// rather than written out here. A hand-written copy carried six of the thirteen and nothing
/// said so, which left every takeoff operator outside this comparison.
fn composed_operator_ids() -> Vec<&'static str> {
    ONSET_OPERATOR_IDS
        .iter()
        .chain(TAKEOFF_OPERATOR_IDS)
        .copied()
        .collect()
}

/// One rule's row from one run, with the binding whose request produced it.
///
/// The binding travels with the row because a value is stated through the rule a caller can
/// name, never through the operator it composes, so a probe has to restate along the path the
/// row came back on.
struct AssumedRow {
    through: &'static Binding,
    entry_id: String,
    /// Name to the text the record shows, for the values this row fell back to rather than
    /// being given.
    values: BTreeMap<String, String>,
}

/// Run every rule this build offers with nothing stated, and collect what each one used.
fn rows_the_rules_assumed() -> Vec<AssumedRow> {
    let trial = synthetic_countermovement_jump();
    let mut rows = Vec::new();

    for binding in BINDINGS {
        let Ok(response) = run(&trial, &request_naming(binding, BTreeMap::new())) else {
            continue;
        };
        for bound in &response.bound_methods {
            let assumed = bound.assumed_parameters();
            rows.push(AssumedRow {
                through: binding,
                entry_id: bound.method_id.clone(),
                values: bound
                    .bound_parameters
                    .iter()
                    .filter(|(name, _)| assumed.contains(name))
                    .cloned()
                    .collect(),
            });
        }
    }
    rows
}

/// The same walk folded to one row per entry, which is what the quantity comparison reads.
fn values_the_rules_assumed() -> BTreeMap<String, BTreeMap<String, String>> {
    let mut assumed: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for row in rows_the_rules_assumed() {
        assumed.entry(row.entry_id).or_default().extend(row.values);
    }
    assumed
}

/// What one rule did with a name a caller stated on it.
#[derive(Debug, PartialEq, Eq)]
enum Stating {
    /// The rule read the name and ran the value.
    Honoured,
    /// The rule declined, naming the parameter, so the value did not run and the caller knows.
    Refused,
    /// The rule never read the name.
    Dropped,
}

/// What happens when a caller states one name on one rule, measured by running it.
///
/// The discriminator this file needs: a rule that offers a choice honours the registry's
/// declared value, and a rule whose own identity fixes the name refuses it. Read off the
/// refusal's parameter field rather than its sentence, because a rule declining over some
/// other name has not answered for this one.
fn stating(binding: &Binding, name: &str, value: &str) -> Stating {
    let trial = synthetic_countermovement_jump();
    let options = BTreeMap::from([(name.to_string(), value.to_string())]);
    let Ok(response) = run(&trial, &request_naming(binding, options)) else {
        return Stating::Refused;
    };
    let refused = response.refusals.iter().any(|declined| {
        plateforce_core::Refusal::from(declined.refusal.clone())
            .parameter
            .as_deref()
            == Some(name)
    });
    if refused {
        return Stating::Refused;
    }
    let unread = response
        .bound_methods
        .iter()
        .any(|bound| bound.unread_parameters.iter().any(|held| held == name));
    if unread {
        Stating::Dropped
    } else {
        Stating::Honoured
    }
}

/// Whether a row belongs to the rule the request named, rather than having come back beside
/// it.
///
/// Onset and takeoff run on every request, so a run sweeping a braking rule carries their
/// operator rows too, and restating one of their names through the braking slot puts it
/// somewhere no rule reads. The probe would come back `Dropped` and the pair would be
/// classified on a question that was never asked. `offered_parameters.rs` reports the same
/// check catching 26 such questions.
fn belongs_to(row: &AssumedRow, entry_construct: &str) -> bool {
    row.entry_id == row.through.id || entry_construct == row.through.construct
}

/// The entries this comparison can reach: every rule a request can name, and every operator
/// those rules compose.
fn reachable_entry_ids() -> Vec<String> {
    BINDINGS
        .iter()
        .map(|binding| binding.id.to_string())
        .chain(composed_operator_ids().into_iter().map(str::to_string))
        .collect()
}

#[test]
fn every_value_a_rule_assumes_is_the_one_the_registry_declares() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let assumed = values_the_rules_assumed();

    let mut compared = 0usize;
    let mut disagreements = Vec::new();

    for id in reachable_entry_ids() {
        let Some(entry) = loaded.registry.methods.get(id.as_str()) else {
            continue;
        };
        let Some(used) = assumed.get(&id) else {
            continue;
        };
        for parameter in &entry.parameters {
            let Some(declared) = parameter.default else {
                continue;
            };
            let Some(shown) = used.get(&parameter.name) else {
                continue;
            };
            let Ok(taken) = shown.parse::<f64>() else {
                continue;
            };
            compared += 1;
            if (taken - declared).abs() > 1e-9 {
                disagreements.push(format!(
                    "{id} declares {} = {declared} and the rule used {taken}",
                    parameter.name
                ));
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "a rule ran on a value the registry does not declare:\n  {}",
        disagreements.join("\n  ")
    );
    // Two populations, counted apart. A rule can record a value and declare no default, so
    // the second number is not a denominator for the first.
    println!(
        "{compared} declared defaults compared, from {} rules that recorded one, of {} binding rows and {} composed operators",
        assumed.len(),
        BINDINGS.len(),
        composed_operator_ids().len()
    );
    assert!(
        compared >= 5,
        "only {compared} declared defaults were reached, so this comparison has stopped covering the rules"
    );
    // The rules reached, rather than the values compared. A rule with no declared default
    // contributes nothing above and still says whether this comparison can see it at all,
    // which is the half that was silently zero for every rule reached by construct id.
    assert!(
        assumed.len() >= 34,
        "only {} rules were reached, so rules this build runs are outside this comparison",
        assumed.len()
    );
}

/// The same rule for a parameter the literature varies by name rather than by number.
///
/// `registry show bwepoch.fixed_window` reporting `centre = median` while a run of that entry
/// binds `mean` is one build reading its own registry two ways, and the record publishes the
/// second while a reader checks the first. Nothing else in the suite sees it: registry, core
/// and Python suites all pass, and the only red is the digest pin, which moves on any registry
/// byte and so says nothing about this.
///
/// A row is out of this population when the rule refuses the declared value, because that is
/// a rule whose own choice fixes the name and the operator's default belongs to the rules that
/// do offer it. Both counts are printed, so a change that moved every row into the refusing
/// population would show as coverage collapsing rather than as agreement.
#[test]
fn every_name_a_rule_assumes_is_the_one_the_registry_declares() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let rows = rows_the_rules_assumed();

    let mut offered = 0usize;
    let mut fixed_by_the_rule = Vec::new();
    let mut dropped = Vec::new();
    let mut disagreements = Vec::new();

    for row in &rows {
        let Some(entry) = loaded.registry.methods.get(row.entry_id.as_str()) else {
            continue;
        };
        if !belongs_to(row, &entry.construct) {
            continue;
        }
        for parameter in &entry.parameters {
            let Some(declared) = &parameter.default_key else {
                continue;
            };
            let Some(taken) = row.values.get(&parameter.name) else {
                continue;
            };
            let named = format!(
                "{} on {} via {}",
                parameter.name, row.entry_id, row.through.id
            );
            match stating(row.through, &parameter.name, declared) {
                Stating::Refused => fixed_by_the_rule.push(named),
                Stating::Dropped => dropped.push(named),
                Stating::Honoured if taken == declared => offered += 1,
                Stating::Honoured => {
                    offered += 1;
                    disagreements.push(format!(
                        "{} declares {} = {declared}, {} runs it as {taken}, and stating \
                         {declared} there is taken",
                        row.entry_id, parameter.name, row.through.id
                    ));
                }
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "a rule bound a name the registry declares otherwise, so the entry and the record \
         disagree about the same run:\n  {}",
        disagreements.join("\n  ")
    );
    assert!(
        dropped.is_empty(),
        "{} rules record a value under a declared name and read nothing when it is stated, so \
         the value in the record answers to nobody:\n  {}",
        dropped.len(),
        dropped.join("\n  ")
    );

    let declared_in_the_registry = loaded
        .registry
        .methods
        .values()
        .flat_map(|entry| &entry.parameters)
        .filter(|parameter| parameter.default_key.is_some())
        .count();
    println!(
        "{offered} name bindings compared over rules that offer the choice, {} more over rules \
         whose own choice fixes the name, against {declared_in_the_registry} parameters \
         declaring one in the registry and {} rows walked",
        fixed_by_the_rule.len(),
        rows.len()
    );

    assert!(
        offered >= 10,
        "only {offered} name bindings were reached through a rule that offers the choice, so \
         this comparison has stopped covering the declared names"
    );
    // Without a row here the comparison above is one arm of a two-armed rule, and a build
    // that had stopped refusing a contradicted value would read as agreement.
    assert!(
        !fixed_by_the_rule.is_empty(),
        "no rule refused the value the registry declares for a name it binds, so the half of \
         this rule that separates a default from an entailed value is out of reach"
    );
}

/// A name the software picks that the registry publishes no default for.
///
/// The same disagreement with the sign reversed: the entry states the values a reader may
/// choose between and says nothing about which one runs, so `registry show` answers the
/// question with silence while every run answers it with a value.
///
/// A rule that refuses every alternative is out of this population, because its own choice
/// fixes the name and the entry has nothing to declare.
#[test]
fn a_name_no_entry_declares_is_not_chosen_in_silence() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let rows = rows_the_rules_assumed();

    let mut silent = Vec::new();
    let mut examined = 0usize;

    for row in &rows {
        let Some(entry) = loaded.registry.methods.get(row.entry_id.as_str()) else {
            continue;
        };
        if !belongs_to(row, &entry.construct) {
            continue;
        }
        for parameter in &entry.parameters {
            if parameter.default_key.is_some() || parameter.named_values.is_empty() {
                continue;
            }
            let Some(taken) = row.values.get(&parameter.name) else {
                continue;
            };
            examined += 1;
            let alternatives: Vec<&str> = parameter
                .named_values
                .iter()
                .map(|value| value.key.as_str())
                .filter(|key| *key != taken)
                .collect();
            if alternatives
                .iter()
                .any(|key| stating(row.through, &parameter.name, key) == Stating::Honoured)
            {
                silent.push(format!(
                    "{} runs {} = {taken} through {}, the entry declares no default for it, \
                     and {alternatives:?} are taken when stated",
                    row.entry_id, parameter.name, row.through.id
                ));
            }
        }
    }

    assert!(
        silent.is_empty(),
        "a rule chose between published names with the registry declaring none:\n  {}",
        silent.join("\n  ")
    );
    println!(
        "{examined} name bindings examined where the entry declares no default, of {} rows walked",
        rows.len()
    );
    assert!(
        examined > 0,
        "no rule bound a name its entry declares no default for, so this guard is watching \
         nothing"
    );
}

/// A default the registry declares for a name no rule reads.
///
/// A published choice a caller cannot make. The entry tells a reader which value runs when
/// they state none, and no rule in this build ever binds that name, so the sentence describes
/// nothing that happens.
#[test]
fn every_declared_name_reaches_a_rule() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let bound_names: BTreeSet<(String, String)> = rows_the_rules_assumed()
        .into_iter()
        .flat_map(|row| {
            row.values
                .into_keys()
                .map(move |name| (row.entry_id.clone(), name))
        })
        .collect();

    let mut declared = 0usize;
    let mut unread = Vec::new();
    for id in reachable_entry_ids() {
        let Some(entry) = loaded.registry.methods.get(id.as_str()) else {
            continue;
        };
        for parameter in &entry.parameters {
            if parameter.default_key.is_none() {
                continue;
            }
            declared += 1;
            if !bound_names.contains(&(id.clone(), parameter.name.clone())) {
                unread.push(format!("{id}.{}", parameter.name));
            }
        }
    }

    assert!(
        unread.is_empty(),
        "{} of {declared} declared names on the entries this build runs are bound by no rule, \
         so the registry publishes a choice no caller can make:\n  {}",
        unread.len(),
        unread.join("\n  ")
    );
    println!("{declared} declared names on the entries this build runs, all of them bound");
    assert!(
        declared >= 10,
        "only {declared} declared names sit on the entries this build runs, so this guard has \
         stopped covering them"
    );
}

/// The interface must not turn a published value into a choice. A parameter the registry
/// publishes values for but declares no default on is unresolved, and sending the first of
/// the list makes the record report a decision nobody took.
#[test]
fn a_published_value_is_not_a_default() {
    let loaded = registry_embed::load().expect("a registry file did not parse");
    let decision_model = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/registry.js"
    ))
    .expect("the decision model is where the interface keeps it");

    assert!(
        !decision_model.contains("= choices[0]"),
        "web/registry.js binds the first published value when a parameter declares no default, \
         which the record then reports as stated rather than assumed"
    );

    let without_default: Vec<String> = BINDINGS
        .iter()
        .filter_map(|binding| loaded.registry.methods.get(binding.id))
        .flat_map(|entry| {
            entry
                .parameters
                .iter()
                .filter(|parameter| {
                    parameter.default.is_none() && !parameter.published_values.is_empty()
                })
                .map(move |parameter| format!("{}.{}", entry.id, parameter.name))
        })
        .collect();

    assert!(
        !without_default.is_empty(),
        "no parameter publishes values without declaring a default, so this guard is watching nothing"
    );
}
