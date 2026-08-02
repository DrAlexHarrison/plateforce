//! Single-trial cost of the modules that sit on the recompute path, measured rather than
//! quoted.
//!
//! Dragging a marker reruns conditioning and everything downstream of it, and that redraw
//! has a 100 ms budget. This asserts nothing: a wall-clock assertion fails on a loaded
//! machine and says nothing about the code. It prints, so the number a report carries is a
//! number somebody can take again on their own hardware.
//!
//! Run it with `cargo test --release -p plateforce-core --test recompute_budget -- --nocapture`.
use std::hint::black_box;
use std::time::Instant;

fn trace(sample_count: usize) -> Vec<f64> {
    (0..sample_count)
        .map(|index| {
            let seconds = index as f64 / 1200.0;
            600.0
                + 300.0 * (2.0 * std::f64::consts::PI * 3.0 * seconds).sin()
                + 4.0 * ((index % 17) as f64 - 8.0)
        })
        .collect()
}

#[test]
fn the_recompute_path_reports_its_cost_with_the_sample_count_beside_it() {
    let values = trace(6000);
    let n = values.len();
    let interval = 1.0 / 1200.0;

    let at = Instant::now();
    for _ in 0..20 {
        black_box(plateforce_core::rate::steepest_chord(
            black_box(&values),
            24,
            0,
            n - 1,
            interval,
        ));
    }
    println!(
        "rate::steepest_chord {:?} over {n} samples",
        at.elapsed() / 20
    );

    let at = Instant::now();
    for _ in 0..20 {
        black_box(
            plateforce_core::butterworth::low_pass_dual_pass_zero_lag(
                &values,
                50.0,
                2,
                1200.0,
                plateforce_core::butterworth::StateInitialisation::SteadyStateAtOpeningLevel,
            )
            .unwrap(),
        );
    }
    println!(
        "butterworth::dual_pass {:?} over {n} samples",
        at.elapsed() / 20
    );

    let at = Instant::now();
    for _ in 0..20 {
        black_box(
            plateforce_core::spectrum::cutoff_retaining_power_fraction(
                black_box(&values),
                1200.0,
                0.99,
            )
            .unwrap(),
        );
    }
    println!(
        "spectrum::cutoff_retaining_power_fraction {:?} over {n} samples",
        at.elapsed() / 20
    );

    let at = Instant::now();
    for _ in 0..20 {
        black_box(
            plateforce_core::resample::resample_interval(black_box(&values), 0, n - 1, 101)
                .unwrap(),
        );
    }
    println!(
        "resample::resample_interval {:?} over {n} samples",
        at.elapsed() / 20
    );

    let basis = plateforce_core::bspline::Basis::clamped_uniform(20, 3).unwrap();
    let grid = plateforce_core::resample::resample_interval(&values, 0, n - 1, 101).unwrap();
    let at = Instant::now();
    for _ in 0..20 {
        black_box(basis.fit(black_box(&grid)).unwrap());
    }
    println!(
        "bspline::fit {:?} over 101 points, 20 functions",
        at.elapsed() / 20
    );

    let at = Instant::now();
    for _ in 0..20 {
        black_box(
            plateforce_core::smoothing::moving_average_boxcar(black_box(&values), 121).unwrap(),
        );
    }
    println!(
        "smoothing::moving_average_boxcar {:?} over {n} samples",
        at.elapsed() / 20
    );
}
