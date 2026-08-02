//! A deterministic multi-subject corpus, because the public fixtures are one subject.
//!
//! Every trace here is generated arithmetic, so nothing in this module is athlete data and
//! the corpus constraint is not engaged. Between-subject aggregation and every reliability
//! figure can only be exercised on a set like this, and one of them needs two distinct
//! athletes so it can never run on public real data at all.

use std::fmt::Write as _;

/// One trace, and the system weight it was generated around.
pub struct GeneratedTrial {
    pub file_name: String,
    pub text: String,
    pub system_weight_newtons: f64,
}

/// A countermovement jump: quiet standing, an unweighting dip, a propulsive rise, flight at
/// zero, then landing. The shape is what the landmark rules look for, not a physical model.
pub fn trace(
    system_weight_newtons: f64,
    sample_rate_hz: f64,
    jitter_newtons: f64,
    seed: u64,
) -> Vec<f64> {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let mut noise = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 33) as f64 / (u32::MAX as f64) - 0.5) * 2.0
    };

    let samples_for = |seconds: f64| (seconds * sample_rate_hz).round() as usize;
    let quiet = samples_for(1.5);
    let unweight = samples_for(0.30);
    let propulsion = samples_for(0.25);
    let flight = samples_for(0.45);
    let landing = samples_for(0.40);

    let mut force = Vec::with_capacity(quiet + unweight + propulsion + flight + landing);
    for _ in 0..quiet {
        force.push(system_weight_newtons + noise() * 1.2);
    }
    for index in 0..unweight {
        let phase = index as f64 / unweight as f64;
        let dip = (phase * std::f64::consts::PI).sin() * 0.42 * system_weight_newtons;
        force.push(system_weight_newtons - dip + noise() * 1.2);
    }
    for index in 0..propulsion {
        let phase = index as f64 / propulsion as f64;
        let rise = (phase * std::f64::consts::PI).sin()
            * (1.05 + jitter_newtons / 100.0)
            * system_weight_newtons;
        force.push(system_weight_newtons + rise);
    }
    force.extend(std::iter::repeat_n(0.0, flight));
    for index in 0..landing {
        let phase = index as f64 / landing as f64;
        let spike = (1.0 - phase) * 2.4 * system_weight_newtons;
        force.push(system_weight_newtons + spike);
    }
    force
}

/// `subjects` athletes with `trials_per_subject` traces each, named `AT{subject}_{trial}.txt`,
/// one bare value per line at 1200 Hz. Deterministic from the seed, so a test that fails
/// fails the same way twice.
pub fn corpus(subjects: usize, trials_per_subject: usize, seed: u64) -> Vec<GeneratedTrial> {
    let mut generated = Vec::with_capacity(subjects * trials_per_subject);
    for subject in 1..=subjects {
        let system_weight_newtons = 520.0 + (subject as f64) * 37.5;
        for trial in 1..=trials_per_subject {
            let trial_seed = seed
                .wrapping_mul(1_000_003)
                .wrapping_add((subject * 97 + trial) as u64);
            let jitter = ((trial_seed % 17) as f64) - 8.0;
            let force = trace(system_weight_newtons, 1200.0, jitter, trial_seed);
            let mut text = String::with_capacity(force.len() * 10);
            for value in &force {
                let _ = writeln!(text, "{value:.4}");
            }
            generated.push(GeneratedTrial {
                file_name: format!("AT{subject:02}_{trial}.txt"),
                text,
                system_weight_newtons,
            });
        }
    }
    generated
}

/// The same corpus written to a directory, for the paths a directory walk needs.
pub fn write_corpus(
    directory: &std::path::Path,
    subjects: usize,
    trials_per_subject: usize,
    seed: u64,
) -> std::io::Result<Vec<GeneratedTrial>> {
    let generated = corpus(subjects, trials_per_subject, seed);
    std::fs::create_dir_all(directory)?;
    for trial in &generated {
        std::fs::write(directory.join(&trial.file_name), &trial.text)?;
    }
    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_seed_generates_one_corpus() {
        let first = corpus(2, 2, 7);
        let second = corpus(2, 2, 7);
        assert_eq!(first.len(), 4);
        for (left, right) in first.iter().zip(second.iter()) {
            assert_eq!(left.file_name, right.file_name);
            assert_eq!(left.text, right.text);
        }
    }

    #[test]
    fn a_generated_trace_stands_still_then_leaves_the_plate() {
        let force = trace(600.0, 1200.0, 0.0, 3);
        assert!(force.contains(&0.0), "it takes off");
        assert!(
            force[..1800]
                .iter()
                .all(|value| (*value - 600.0).abs() < 5.0),
            "it stands still first"
        );
    }
}
