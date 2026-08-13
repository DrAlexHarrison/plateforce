//! The missing-rate refusal distinguishes quantities that scale once with time from the
//! heights and displacements that scale twice.

use std::process::Command;

const TRIAL: &str = "../plateforce-conformance/fixtures/subject01_trial1.force.txt";

fn missing_rate_refusal() -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_plateforce"))
        .args([
            "--registry",
            "../../registry",
            "analyse",
            TRIAL,
            "--column",
            "0",
            "--sentinel",
            "none",
            "--preset",
            "sams",
        ])
        .env("NO_COLOR", "1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the built binary runs");
    assert_eq!(
        output.status.code(),
        Some(64),
        "a required fact about the recording was left unstated"
    );
    assert!(
        output.stdout.is_empty(),
        "a refusal wrote a result before the missing rate was supplied"
    );
    String::from_utf8(output.stderr).expect("the refusal is UTF-8")
}

#[test]
fn the_refusal_says_velocity_and_impulse_are_out_by_a_fifth() {
    let told = missing_rate_refusal();
    assert!(
        told.contains("velocity and impulse out by a fifth"),
        "the two quantities that scale once with the rate are not stated together: {told}"
    );
}

#[test]
fn the_refusal_says_height_and_displacement_are_out_by_nearly_half() {
    let told = missing_rate_refusal();
    assert!(
        told.contains("height and displacement") && told.contains("out by nearly half"),
        "the two quantities that scale with the square of the rate carry no separate magnitude: {told}"
    );
    assert!(
        !told.contains("velocity, displacement and impulse by a fifth"),
        "the old one-magnitude claim remains: {told}"
    );
}
