//! What `net` means for peak force, held to the reading the registry states.
//!
//! Net is the peak after system weight has been subtracted. The other reading in the
//! literature, peak minus the force at onset, makes peak force move when the onset rule
//! moves, and the two are not distinguishable from the word `net` alone.

use plateforce_analysis::run;

use crate::common::{committed_trial, default_request, COMMITTED_TRIALS};

/// Subtracting a constant commutes with taking a maximum, so the peak of the net series
/// and the net of the peak are one number. Computed by both routes, because a check that
/// takes one and subtracts is the identity written twice rather than tested.
#[test]
fn net_peak_force_is_the_gross_peak_less_one_system_weight_by_either_route() {
    let mut fractions = Vec::new();

    for name in COMMITTED_TRIALS {
        let trial = committed_trial(name);
        let response = run(&trial, &default_request())
            .unwrap_or_else(|error| panic!("{name} did not run: {error}"));
        let (Some(onset), Some(takeoff)) = (response.onset_index, response.takeoff_index) else {
            panic!("{name} placed no landmarks, so there is no window to take a peak over");
        };
        let system_weight_newtons = response.levels.system_weight_newtons;
        let window = &trial.force()[onset..takeoff];

        let gross_peak_newtons = window.iter().copied().fold(f64::MIN, f64::max);
        let net_of_the_peak = gross_peak_newtons - system_weight_newtons;
        let peak_of_the_net = window
            .iter()
            .map(|force| force - system_weight_newtons)
            .fold(f64::MIN, f64::max);

        assert!(
            (net_of_the_peak - peak_of_the_net).abs() < 1e-9,
            "{name}: subtracting then maximising gives {peak_of_the_net} N and maximising then \
             subtracting gives {net_of_the_peak} N"
        );

        fractions.push(system_weight_newtons / net_of_the_peak);
        println!(
            "{name}: system weight {system_weight_newtons:.1} N, gross peak {gross_peak_newtons:.1} N, \
             net peak {net_of_the_peak:.1} N, gross exceeds net by {:.4} of net",
            system_weight_newtons / net_of_the_peak
        );
    }

    assert_eq!(
        fractions.len(),
        COMMITTED_TRIALS.len(),
        "the check read fewer trials than are committed"
    );
    let mean = fractions.iter().sum::<f64>() / fractions.len() as f64;
    println!(
        "over {} committed trials the fraction runs {:.4} to {:.4}, mean {mean:.4}",
        fractions.len(),
        fractions.iter().copied().fold(f64::MAX, f64::min),
        fractions.iter().copied().fold(f64::MIN, f64::max)
    );
}

/// How far apart the two readings of `net` can be, on a trial where the onset rule places
/// onset well below system weight.
///
/// Under the default onset rule they nearly coincide, because a small departure threshold
/// puts onset where force is still essentially at system weight. That makes the default
/// request unable to tell the two conventions apart, so the case is built rather than
/// assumed.
#[test]
fn the_two_readings_of_net_diverge_once_onset_sits_below_system_weight() {
    let trial = committed_trial(COMMITTED_TRIALS[0]);

    let mut deep = default_request();
    deep.onset.method_id = "onset.threshold.relative_to_system_weight".into();
    deep.onset.parameters.insert("pct".into(), 20.0);

    let shallow = run(&trial, &default_request()).expect("the default request ran");
    let deep = run(&trial, &deep).expect("the deep-threshold request ran");

    let gap = |response: &plateforce_analysis::AnalysisResponse| {
        let onset = response.onset_index.expect("onset resolved");
        (response.levels.system_weight_newtons - trial.force()[onset]).abs()
    };

    let shallow_gap = gap(&shallow);
    let deep_gap = gap(&deep);
    println!(
        "force at onset sits {shallow_gap:.2} N below system weight under the default rule and \
         {deep_gap:.2} N below it at a 20 percent threshold"
    );

    assert!(
        deep_gap > 50.0,
        "the deep threshold put onset {deep_gap} N below system weight, which is not far enough to \
         separate the two readings"
    );
    assert!(
        deep_gap > shallow_gap * 5.0,
        "the two requests differ by too little for this to be a comparison: {shallow_gap} N against \
         {deep_gap} N"
    );
}
