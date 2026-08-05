//! The whole of the binding framework's surface, and the only file that names it.
//!
//! Every function here forwards. None computes, none formats a message, none decides
//! anything. Changing which framework binds R to this engine edits this file and the
//! package manifest, and nothing else.

use extendr_api::prelude::*;

use crate::TrialHandle;

#[extendr]
pub fn pf_version_json() -> String {
    crate::version_json()
}

#[extendr]
pub fn pf_registry_json(root: &str) -> String {
    crate::registry_json(root)
}

#[extendr]
pub fn pf_registry_entry_json(root: &str, id: &str) -> String {
    crate::registry_entry_json(root, id)
}

#[extendr]
pub fn pf_bindings_json() -> String {
    crate::bindings_json()
}

#[extendr]
pub fn pf_acquisition_members_json() -> String {
    crate::acquisition_members_json()
}

#[extendr]
pub fn pf_plate_save_json(request_json: &str) -> String {
    crate::plate_save_json(request_json)
}

#[extendr]
pub fn pf_plate_json(request_json: &str) -> String {
    crate::plate_json(request_json)
}

#[extendr]
pub fn pf_plate_stated_json(request_json: &str) -> String {
    crate::plate_stated_json(request_json)
}

#[extendr]
pub fn pf_plates_json(request_json: &str) -> String {
    crate::plates_json(request_json)
}

#[extendr]
pub fn pf_plate_forget_json(request_json: &str) -> String {
    crate::plate_forget_json(request_json)
}

#[extendr]
pub fn pf_trial_from_force(force_newtons: &[f64], request_json: &str) -> List {
    carried(crate::trial_from_force(force_newtons, request_json))
}

#[extendr]
pub fn pf_trial_from_file(request_json: &str) -> List {
    carried(crate::trial_from_file(request_json))
}

#[extendr]
pub fn pf_trial_report_json(handle: Robj) -> String {
    match held(&handle) {
        Some(trial) => crate::trial_report_json(trial),
        None => crate::handle_lost_json(),
    }
}

#[extendr]
pub fn pf_trial_force(handle: Robj) -> Doubles {
    match held(&handle) {
        Some(trial) => crate::trial_force(trial).iter().collect(),
        None => Doubles::new(0),
    }
}

#[extendr]
pub fn pf_analyse_under_preset_json(
    handle: Robj,
    root: &str,
    preset_id: &str,
    request_json: &str,
) -> String {
    match held(&handle) {
        Some(trial) => crate::analyse_under_preset_json(trial, root, preset_id, request_json),
        None => crate::handle_lost_json(),
    }
}

#[extendr]
pub fn pf_spread_json(handle: Robj, request_json: &str) -> String {
    match held(&handle) {
        Some(trial) => crate::spread_json(trial, request_json),
        None => crate::handle_lost_json(),
    }
}

#[extendr]
pub fn pf_spread_under_preset_json(
    handle: Robj,
    root: &str,
    preset_id: &str,
    request_json: &str,
) -> String {
    match held(&handle) {
        Some(trial) => crate::spread_under_preset_json(trial, root, preset_id, request_json),
        None => crate::handle_lost_json(),
    }
}

#[extendr]
pub fn pf_analyse_json(handle: Robj, request_json: &str) -> String {
    match held(&handle) {
        Some(trial) => crate::analyse_json(trial, request_json),
        None => crate::handle_lost_json(),
    }
}

#[extendr]
pub fn pf_capability_json() -> String {
    crate::capability_json()
}

#[extendr]
pub fn pf_double_probe_json(count: i32) -> String {
    crate::double_probe_json(count.max(0) as usize)
}

/// Turns one of this crate's answers into R data.
///
/// R never parses JSON. A parser written in R would be a second reading of a document this
/// side already holds parsed, and the two would disagree on a number's precision before
/// they disagreed on anything interesting.
#[extendr]
pub fn pf_decode(document: &str) -> Robj {
    match serde_json::from_str::<serde_json::Value>(document) {
        Ok(value) => as_r(&value),
        Err(error) => Robj::from(error.to_string()),
    }
}

fn as_r(value: &serde_json::Value) -> Robj {
    use serde_json::Value;
    match value {
        Value::Null => r!(NULL),
        Value::Bool(flag) => Robj::from(*flag),
        // Integers stay integral where R can hold them, so an index does not arrive as
        // 3.0000000001 after a round trip nobody asked for.
        Value::Number(number) => match number.as_i64() {
            Some(whole) if whole.abs() <= i32::MAX as i64 => Robj::from(whole as i32),
            _ => Robj::from(number.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(text) => Robj::from(text.as_str()),
        Value::Array(items) => {
            let converted: Vec<Robj> = items.iter().map(as_r).collect();
            List::from_values(converted).into()
        }
        Value::Object(fields) => {
            let names: Vec<&str> = fields.keys().map(String::as_str).collect();
            let values: Vec<Robj> = fields.values().map(as_r).collect();
            List::from_names_and_values(names, values)
                .map(Robj::from)
                .unwrap_or_else(|_| r!(NULL))
        }
    }
}

/// The envelope travels as text and the trace stays behind a pointer, so R holds one
/// object that carries both what happened and what it can go on to analyse.
fn carried((envelope, handle): (String, Option<TrialHandle>)) -> List {
    let pointer = match handle {
        Some(handle) => Robj::from(ExternalPtr::new(handle)),
        None => r!(NULL),
    };
    list!(envelope = envelope, handle = pointer)
}

fn held(handle: &Robj) -> Option<&TrialHandle> {
    <&ExternalPtr<TrialHandle>>::try_from(handle)
        .ok()
        .map(|pointer| pointer.as_ref())
}

extendr_module! {
    mod plateforce;
    fn pf_decode;
    fn pf_version_json;
    fn pf_registry_json;
    fn pf_registry_entry_json;
    fn pf_bindings_json;
    fn pf_acquisition_members_json;
    fn pf_plate_save_json;
    fn pf_plate_json;
    fn pf_plate_stated_json;
    fn pf_plates_json;
    fn pf_plate_forget_json;
    fn pf_trial_from_force;
    fn pf_trial_from_file;
    fn pf_trial_report_json;
    fn pf_trial_force;
    fn pf_analyse_json;
    fn pf_analyse_under_preset_json;
    fn pf_spread_json;
    fn pf_spread_under_preset_json;
    fn pf_double_probe_json;
    fn pf_capability_json;
}
