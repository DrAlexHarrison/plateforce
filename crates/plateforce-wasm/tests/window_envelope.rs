use plateforce_wasm::LoadedTrial;

#[test]
fn a_window_envelope_draws_only_the_requested_samples() {
    let trial = LoadedTrial::demonstration();
    let info: serde_json::Value =
        serde_json::from_str(&trial.info_json().expect("the trial describes itself"))
            .expect("the trial description is JSON");
    let samples = info["sample_count"]
        .as_u64()
        .expect("the sample count is an integer") as usize;

    let full: serde_json::Value = serde_json::from_str(
        &trial
            .envelope_json(samples)
            .expect("one bucket per sample is available"),
    )
    .expect("the full envelope is JSON");

    let start = 300;
    let end = 420;
    let window: serde_json::Value = serde_json::from_str(
        &trial
            .window_envelope_json(end - start, start, end)
            .expect("the requested window is available"),
    )
    .expect("the window envelope is JSON");

    assert_eq!(window["sample_count"], full["sample_count"]);
    assert_eq!(window["sample_rate_hz"], full["sample_rate_hz"]);
    assert_eq!(
        window["lower"].as_array().expect("window lower envelope"),
        &full["lower"].as_array().expect("full lower envelope")[start..end]
    );
    assert_eq!(
        window["upper"].as_array().expect("window upper envelope"),
        &full["upper"].as_array().expect("full upper envelope")[start..end]
    );
}

#[test]
fn a_window_envelope_never_allocates_more_buckets_than_visible_samples() {
    let trial = LoadedTrial::demonstration();
    let window: serde_json::Value = serde_json::from_str(
        &trial
            .window_envelope_json(10_000, 300, 420)
            .expect("the requested window is available"),
    )
    .expect("the window envelope is JSON");

    assert_eq!(window["lower"].as_array().unwrap().len(), 120);
    assert_eq!(window["upper"].as_array().unwrap().len(), 120);
}
