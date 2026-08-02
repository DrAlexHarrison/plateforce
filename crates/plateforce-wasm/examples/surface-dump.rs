//! Everything the browser can ask this module for, printed as the bytes it would receive.
//!
//! The interface talks to this crate over JSON strings, so a dump of every answer at two
//! commits is the whole of what a user could observe changing. Moving code between crates
//! is proven by diffing two runs of this, not by reading the diff that caused them.
//!
//! A request the module refuses outright is absent from this record and covered by the
//! characterisation baseline instead: a refusal is carried out of here as `JsError`, whose
//! constructor is a wasm import and aborts when it is called off a wasm target.

use plateforce_wasm::{build_info_json, registry_json, ForceFile, LoadedTrial};

fn quiet_standing(samples: usize, weight_newtons: f64) -> Vec<f64> {
    (0..samples)
        .map(|index| weight_newtons + ((index % 17) as f64 - 8.0) * 0.4)
        .collect()
}

/// The shape the in-crate characterisation runs against, so both records cover one trace.
fn synthetic() -> Vec<f64> {
    let mut force = quiet_standing(1200, 600.0);
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(1400.0, 240));
    force
}

fn two_flight_phases() -> Vec<f64> {
    let mut force = vec![600.0; 1200];
    force.extend(std::iter::repeat_n(0.0, 400));
    force.extend(std::iter::repeat_n(600.0, 300));
    force.extend(std::iter::repeat_n(0.0, 1200));
    force
}

fn pre_movement_bump() -> Vec<f64> {
    let mut force = quiet_standing(1000, 600.0);
    force.extend(std::iter::repeat_n(680.0, 120));
    force.extend(std::iter::repeat_n(600.0, 240));
    force.extend((0..360).map(|index| 600.0 - 300.0 * (index as f64 / 360.0)));
    force.extend((0..360).map(|index| 300.0 + 1200.0 * (index as f64 / 360.0)));
    force.extend(std::iter::repeat_n(0.0, 600));
    force.extend(std::iter::repeat_n(1400.0, 240));
    force
}

/// A vendor-shaped export, so the reader's delimiter, preamble and rate decisions are in
/// the record alongside the analysis they feed.
fn as_export_text(force: &[f64]) -> String {
    let mut text = String::from("Exported 2011-03-04\nPlate 1\ntime,fx,fz\n");
    for (index, value) in force.iter().enumerate() {
        text.push_str(&format!(
            "{:.6},{:.4},{:.4}\n",
            index as f64 / 1200.0,
            1.1,
            value
        ));
    }
    text
}

type Parameters = &'static [(&'static str, f64)];
type Options = &'static [(&'static str, &'static str)];
type Case = (&'static str, &'static str, Parameters, Options);

const ONSET_CASES: &[Case] = &[
    ("bare", "onset.threshold.noise_relative", &[], &[]),
    ("k", "onset.threshold.noise_relative", &[("k", 2.0)], &[]),
    (
        "below_only",
        "onset.threshold.noise_relative",
        &[],
        &[("direction", "below_only")],
    ),
    (
        "two_sided",
        "onset.threshold.noise_relative",
        &[],
        &[("direction", "two_sided")],
    ),
    (
        "selection last",
        "onset.threshold.noise_relative",
        &[],
        &[("selection", "last")],
    ),
    (
        "persistence",
        "onset.threshold.noise_relative",
        &[("span_ms", 10.0)],
        &[],
    ),
    (
        "search floor",
        "onset.threshold.noise_relative",
        &[("floor_seconds", 0.9)],
        &[],
    ),
    (
        "back offset",
        "onset.threshold.noise_relative",
        &[("offset_ms", 50.0)],
        &[],
    ),
    (
        "degenerate fraction",
        "onset.threshold.noise_relative",
        &[("degenerate_fraction", 0.2)],
        &[],
    ),
    (
        "every value stated",
        "onset.threshold.noise_relative",
        &[
            ("k", 3.0),
            ("span_ms", 5.0),
            ("floor_seconds", 0.85),
            ("offset_ms", 20.0),
            ("degenerate_fraction", 0.1),
        ],
        &[("direction", "below_only"), ("selection", "last")],
    ),
    (
        "names another rule carries",
        "onset.threshold.noise_relative",
        &[("threshold_n", 50.0), ("pct", 1.0)],
        &[],
    ),
    (
        "bare",
        "onset.threshold.relative_to_system_weight",
        &[],
        &[],
    ),
    (
        "pct",
        "onset.threshold.relative_to_system_weight",
        &[("pct", 5.0)],
        &[],
    ),
    (
        "superseded spelling",
        "onset.threshold.relative_to_system_weight",
        &[("percent", 5.0)],
        &[],
    ),
    ("bare", "onset.threshold.absolute_force", &[], &[]),
    (
        "threshold",
        "onset.threshold.absolute_force",
        &[("threshold_n", 50.0)],
        &[],
    ),
    (
        "superseded spelling",
        "onset.threshold.absolute_force",
        &[("threshold_newtons", 50.0)],
        &[],
    ),
    (
        "two_sided",
        "onset.threshold.absolute_force",
        &[],
        &[("direction", "two_sided")],
    ),
    (
        "above_only refused",
        "onset.threshold.absolute_force",
        &[],
        &[("direction", "above_only")],
    ),
    ("bare", "onset.threshold.last_within_band", &[], &[]),
    ("k", "onset.threshold.last_within_band", &[("k", 3.0)], &[]),
    (
        "inverse lookback",
        "onset.threshold.last_within_band",
        &[("inverse_lookback", 0.25)],
        &[],
    ),
    (
        "back offset",
        "onset.threshold.last_within_band",
        &[("offset_ms", 50.0)],
        &[],
    ),
    ("bare", "onset.threshold.adaptive_trailing_window", &[], &[]),
    (
        "window",
        "onset.threshold.adaptive_trailing_window",
        &[("window_seconds", 0.25)],
        &[],
    ),
    (
        "population",
        "onset.threshold.adaptive_trailing_window",
        &[],
        &[("dispersion", "population")],
    ),
];

const TAKEOFF_CASES: &[Case] = &[
    ("bare", "takeoff.threshold.absolute_force", &[], &[]),
    (
        "threshold",
        "takeoff.threshold.absolute_force",
        &[("threshold_n", 25.0)],
        &[],
    ),
    (
        "superseded spelling",
        "takeoff.threshold.absolute_force",
        &[("threshold_newtons", 30.0), ("minimum_flight", 0.03)],
        &[],
    ),
    (
        "persistence",
        "takeoff.threshold.absolute_force",
        &[("persistence_ms", 50.0)],
        &[],
    ),
    (
        "magnitude",
        "takeoff.threshold.absolute_force",
        &[],
        &[("comparison", "magnitude")],
    ),
    ("bare", "takeoff.threshold.longest_run", &[], &[]),
    (
        "filter then rank",
        "takeoff.threshold.longest_run",
        &[],
        &[("short_run_handling", "filter_then_rank")],
    ),
    (
        "every value stated",
        "takeoff.threshold.longest_run",
        &[("threshold_n", 25.0), ("persistence_ms", 50.0)],
        &[
            ("comparison", "magnitude"),
            ("short_run_handling", "filter_then_rank"),
        ],
    ),
    ("bare", "takeoff.threshold.descending_crossing", &[], &[]),
    (
        "confirmation",
        "takeoff.threshold.descending_crossing",
        &[("persistence_ms", 50.0)],
        &[],
    ),
    ("bare", "takeoff.threshold.flight_noise_k_sd", &[], &[]),
    (
        "trim",
        "takeoff.threshold.flight_noise_k_sd",
        &[("trim_fraction", 0.4)],
        &[],
    ),
    (
        "every value stated",
        "takeoff.threshold.flight_noise_k_sd",
        &[
            ("trim_fraction", 0.1),
            ("k", 8.0),
            ("bounding_threshold_n", 20.0),
        ],
        &[("dispersion", "population")],
    ),
];

/// Name, rule, where the window starts, how long it runs, and what else was stated.
type WeighingCase = (
    &'static str,
    &'static str,
    Option<usize>,
    f64,
    Parameters,
    Options,
);

const WEIGHING_CASES: &[WeighingCase] = &[
    ("bare", "bwepoch.fixed_window", None, 0.8, &[], &[]),
    ("moved", "bwepoch.fixed_window", Some(240), 0.8, &[], &[]),
    (
        "median",
        "bwepoch.fixed_window",
        None,
        0.8,
        &[],
        &[("centre", "median")],
    ),
    (
        "population",
        "bwepoch.fixed_window",
        None,
        0.8,
        &[],
        &[("dispersion", "population")],
    ),
    (
        "superseded spelling",
        "bwepoch.fixed_window",
        None,
        0.8,
        &[("duration_seconds", 0.2)],
        &[],
    ),
    (
        "moved",
        "bwepoch.manual_placement",
        Some(240),
        0.5,
        &[],
        &[],
    ),
    (
        "bare",
        "bwepoch.adaptive_lowest_variance",
        None,
        0.8,
        &[],
        &[],
    ),
    (
        "cumulative",
        "bwepoch.adaptive_lowest_variance",
        None,
        0.8,
        &[],
        &[("accumulation", "cumulative_sum_of_squares")],
    ),
    (
        "floor published",
        "bwepoch.adaptive_lowest_variance",
        None,
        0.8,
        &[("variance_floor_pct_bodyweight", 0.5)],
        &[],
    ),
    (
        "floor binding",
        "bwepoch.adaptive_lowest_variance",
        None,
        0.8,
        &[("variance_floor_pct_bodyweight", 2.0)],
        &[],
    ),
];

fn window_length_parameter(method_id: &str) -> &'static str {
    match method_id {
        "bwepoch.adaptive_lowest_variance" => "window_seconds",
        "bwepoch.manual_placement" => "span_seconds",
        _ => "duration",
    }
}

fn numbers(pairs: Parameters) -> String {
    let body: Vec<String> = pairs
        .iter()
        .map(|(name, value)| format!("\"{name}\": {value}"))
        .collect();
    format!("{{{}}}", body.join(", "))
}

fn strings(pairs: Options) -> String {
    let body: Vec<String> = pairs
        .iter()
        .map(|(name, value)| format!("\"{name}\": \"{value}\""))
        .collect();
    format!("{{{}}}", body.join(", "))
}

fn weighing_json(
    method_id: &str,
    start_index: Option<usize>,
    duration_seconds: f64,
    parameters: Parameters,
    options: Options,
) -> String {
    let mut values: Vec<String> = parameters
        .iter()
        .map(|(name, value)| format!("\"{name}\": {value}"))
        .collect();
    values.push(format!(
        "\"{}\": {}",
        window_length_parameter(method_id),
        duration_seconds
    ));
    format!(
        "{{\"method_id\": \"{}\", \"start_index\": {}, \"parameters\": {{{}}}, \"options\": {}}}",
        method_id,
        match start_index {
            Some(index) => index.to_string(),
            None => "null".to_string(),
        },
        values.join(", "),
        strings(options)
    )
}

fn choice_json(method_id: &str, parameters: Parameters, options: Options) -> String {
    format!(
        "{{\"method_id\": \"{}\", \"parameters\": {}, \"options\": {}, \"manual_index\": null}}",
        method_id,
        numbers(parameters),
        strings(options)
    )
}

fn request_json(weighing: &str, onset: &str, takeoff: &str, gravity: f64) -> String {
    format!(
        "{{\"weighing\": {weighing}, \"onset\": {onset}, \"takeoff\": {takeoff}, \
         \"touchdown_index\": null, \"gravity_meters_per_second_squared\": {gravity}, \
         \"registry_backed_ids\": [\"onset.threshold.noise_relative\"]}}"
    )
}

/// Every request the record covers, as the text that crosses the boundary.
fn requests() -> Vec<(String, String)> {
    let default_weighing = weighing_json("bwepoch.fixed_window", None, 0.8, &[], &[]);
    let default_onset = choice_json("onset.threshold.noise_relative", &[], &[]);
    let default_takeoff = choice_json("takeoff.threshold.absolute_force", &[], &[]);
    let mut cases = Vec::new();

    for (name, method_id, parameters, options) in ONSET_CASES {
        cases.push((
            format!("onset {method_id} {name}"),
            request_json(
                &default_weighing,
                &choice_json(method_id, parameters, options),
                &default_takeoff,
                9.80665,
            ),
        ));
    }
    for (name, method_id, parameters, options) in TAKEOFF_CASES {
        cases.push((
            format!("takeoff {method_id} {name}"),
            request_json(
                &default_weighing,
                &default_onset,
                &choice_json(method_id, parameters, options),
                9.80665,
            ),
        ));
    }
    for (name, method_id, start_index, duration_seconds, parameters, options) in WEIGHING_CASES {
        cases.push((
            format!("weighing {method_id} {name}"),
            request_json(
                &weighing_json(
                    method_id,
                    *start_index,
                    *duration_seconds,
                    parameters,
                    options,
                ),
                &default_onset,
                &default_takeoff,
                9.80665,
            ),
        ));
    }

    cases.push((
        "gravity 9.8".into(),
        request_json(&default_weighing, &default_onset, &default_takeoff, 9.8),
    ));
    cases.push((
        "onset dragged".into(),
        request_json(
            &default_weighing,
            &choice_json("onset.threshold.noise_relative", &[], &[])
                .replace("\"manual_index\": null", "\"manual_index\": 1100"),
            &default_takeoff,
            9.80665,
        ),
    ));
    cases.push((
        "both dragged".into(),
        request_json(
            &default_weighing,
            &choice_json("onset.threshold.noise_relative", &[], &[])
                .replace("\"manual_index\": null", "\"manual_index\": 1150"),
            &choice_json("takeoff.threshold.absolute_force", &[], &[])
                .replace("\"manual_index\": null", "\"manual_index\": 2050"),
            9.80665,
        ),
    ));
    cases
}

/// The sweeps the spread panel runs, including the axes that were inert before the
/// registry's own parameter names reached the rules.
fn sweeps() -> Vec<(String, String)> {
    let base = request_json(
        &weighing_json("bwepoch.fixed_window", None, 0.8, &[], &[]),
        &choice_json("onset.threshold.noise_relative", &[], &[]),
        &choice_json("takeoff.threshold.absolute_force", &[], &[]),
        9.80665,
    );
    let axis = |slot: &str, parameter: &str, values: &str| {
        format!("{{\"slot\": \"{slot}\", \"parameter\": \"{parameter}\", \"values\": {values}, \"method_ids\": []}}")
    };
    let sweep = |axes: String, quantity: &str| {
        format!(
            "{{\"base\": {base}, \"axes\": [{axes}], \"quantity_key\": \"{quantity}\", \"maximum_combinations\": 512}}"
        )
    };

    vec![
        (
            "onset k".into(),
            sweep(
                axis("onset", "k", "[2.0, 3.0, 5.0, 10.0]"),
                "time_to_takeoff_seconds",
            ),
        ),
        (
            "onset offset_ms".into(),
            sweep(
                axis("onset", "offset_ms", "[30.0, 200.0, 250.0, 300.0]"),
                "time_to_takeoff_seconds",
            ),
        ),
        (
            "takeoff threshold_n".into(),
            sweep(
                axis("takeoff", "threshold_n", "[1.0, 5.0, 10.0, 20.0, 30.0, 50.0]"),
                "jump_height_from_flight_time_meters",
            ),
        ),
        (
            "takeoff persistence_ms".into(),
            sweep(
                axis("takeoff", "persistence_ms", "[15.0, 30.0, 250.0]"),
                "flight_time_seconds",
            ),
        ),
        (
            "weighing duration".into(),
            sweep(
                axis("weighing", "duration", "[0.3, 0.5, 1.0, 2.0]"),
                "system_weight_newtons",
            ),
        ),
        (
            "onset method ids".into(),
            sweep(
                "{\"slot\": \"onset\", \"parameter\": null, \"values\": [], \"method_ids\": [\"onset.threshold.noise_relative\", \"onset.threshold.relative_to_system_weight\", \"onset.threshold.absolute_force\", \"onset.threshold.last_within_band\"]}".into(),
                "time_to_takeoff_seconds",
            ),
        ),
        (
            "two axes".into(),
            sweep(
                format!(
                    "{}, {}",
                    axis("onset", "k", "[2.0, 5.0, 10.0]"),
                    axis("takeoff", "threshold_n", "[10.0, 20.0, 30.0]")
                ),
                "jump_height_from_takeoff_meters",
            ),
        ),
    ]
}

fn report(label: &str, outcome: Result<String, impl std::fmt::Debug>) {
    match outcome {
        Ok(text) => println!("{label}\n  {text}"),
        Err(error) => println!("{label}\n  refused {error:?}"),
    }
}

fn main() {
    report("build info", build_info_json());
    report("registry", registry_json());

    let traces: Vec<(&str, Vec<f64>)> = vec![
        ("synthetic", synthetic()),
        ("two flight phases", two_flight_phases()),
        ("pre movement bump", pre_movement_bump()),
    ];

    let mut loaded: Vec<(String, LoadedTrial)> =
        vec![("demonstration".into(), LoadedTrial::demonstration())];
    for (name, force) in &traces {
        let text = as_export_text(force);
        let file = ForceFile::parse_text(&text).expect("the reader stopped reading its own export");
        report(&format!("reader {name}"), file.summary_json());
        let trial = LoadedTrial::from_force_file(&file, 2, 1200.0, "none")
            .expect("the reader's own column stopped binding");
        loaded.push(((*name).to_string(), trial));
    }

    for (trace, trial) in &loaded {
        report(&format!("session {trace} info"), trial.info_json());
        report(
            &format!("session {trace} envelope"),
            trial.envelope_json(64),
        );
        for (name, payload) in requests() {
            report(&format!("analyze {trace} {name}"), trial.analyse(&payload));
        }
        for (name, payload) in sweeps() {
            report(&format!("spread {trace} {name}"), trial.spread(&payload));
        }
    }
}
