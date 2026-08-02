//! The power spectrum of a force trace, and the cutoff rules that read it.
//!
//! Conditioning resolves upstream of every metric, so a cutoff estimator sits on the path
//! that reruns when a marker is dragged. A transform quadratic in samples costs about
//! thirty-six million operations on a five-second trace at 1200 Hz, which is why the
//! transform here is a radix-two fast one over a zero-padded length.
//!
//! Zero padding changes the frequency grid and not the spectrum: it interpolates between
//! the bins the untrimmed length would have given. The padding length travels back with
//! the spectrum so a caller can say what resolution the answer was read at.

use crate::statistics::{compensated_sum, mean};

#[derive(Debug, thiserror::Error)]
pub enum SpectrumError {
    #[error("the power spectrum needs at least two samples and the trace holds {sample_count}")]
    TraceTooShort { sample_count: usize },
    #[error(
        "cutoff(retained_power_fraction = {fraction}) must sit above zero and at or below one"
    )]
    FractionOutsideUnitInterval { fraction: f64 },
}

/// The one-sided power spectrum of a trace, with the frequency each bin sits at.
#[derive(Debug, Clone)]
pub struct PowerSpectrum {
    pub bin_frequencies_hz: Vec<f64>,
    pub bin_power: Vec<f64>,
    /// The length the trace was padded to, which is what set the frequency spacing.
    pub transform_length: usize,
}

impl PowerSpectrum {
    pub fn total_power(&self) -> f64 {
        compensated_sum(&self.bin_power)
    }

    pub fn frequency_spacing_hz(&self) -> f64 {
        self.bin_frequencies_hz.get(1).copied().unwrap_or(0.0)
    }
}

/// In-place radix-two decimation in time, over a length that is a power of two.
fn fast_fourier_transform(real: &mut [f64], imaginary: &mut [f64]) {
    let length = real.len();
    let mut target = 0usize;
    for source in 1..length {
        let mut bit = length >> 1;
        while target & bit != 0 {
            target ^= bit;
            bit >>= 1;
        }
        target |= bit;
        if source < target {
            real.swap(source, target);
            imaginary.swap(source, target);
        }
    }

    let mut span = 2usize;
    while span <= length {
        let angle_step = -2.0 * std::f64::consts::PI / span as f64;
        for block in (0..length).step_by(span) {
            for offset in 0..span / 2 {
                let angle = angle_step * offset as f64;
                let (sine, cosine) = angle.sin_cos();
                let upper = block + offset + span / 2;
                let lower = block + offset;
                let rotated_real = real[upper] * cosine - imaginary[upper] * sine;
                let rotated_imaginary = real[upper] * sine + imaginary[upper] * cosine;
                real[upper] = real[lower] - rotated_real;
                imaginary[upper] = imaginary[lower] - rotated_imaginary;
                real[lower] += rotated_real;
                imaginary[lower] += rotated_imaginary;
            }
        }
        span <<= 1;
    }
}

/// The one-sided power spectrum, with the mean removed first.
///
/// A force trace stands at system weight, so its zero-frequency term carries more power
/// than everything else together and any rule reading a fraction of total power would
/// otherwise be reading the athlete's mass.
pub fn power_spectrum(values: &[f64], sample_rate_hz: f64) -> Result<PowerSpectrum, SpectrumError> {
    if values.len() < 2 {
        return Err(SpectrumError::TraceTooShort {
            sample_count: values.len(),
        });
    }
    let centre = mean(values).unwrap_or(0.0);
    let mut transform_length = 1usize;
    while transform_length < values.len() {
        transform_length <<= 1;
    }

    let mut real = vec![0.0f64; transform_length];
    let mut imaginary = vec![0.0f64; transform_length];
    for (slot, &value) in real.iter_mut().zip(values) {
        *slot = value - centre;
    }
    fast_fourier_transform(&mut real, &mut imaginary);

    let one_sided = transform_length / 2 + 1;
    let spacing = sample_rate_hz / transform_length as f64;
    Ok(PowerSpectrum {
        bin_frequencies_hz: (0..one_sided).map(|bin| bin as f64 * spacing).collect(),
        bin_power: (0..one_sided)
            .map(|bin| real[bin] * real[bin] + imaginary[bin] * imaginary[bin])
            .collect(),
        transform_length,
    })
}

/// The lowest frequency below which a stated fraction of the trace's power lies.
///
/// The rule reports a cutoff rather than a filter: it says where to put the boundary, and
/// what happens at the boundary is a separate entry. The returned frequency is the first
/// bin at which the running total reaches the fraction, so it is a bin edge and its
/// resolution is the spectrum's frequency spacing.
pub fn cutoff_retaining_power_fraction(
    values: &[f64],
    sample_rate_hz: f64,
    retained_power_fraction: f64,
) -> Result<f64, SpectrumError> {
    if !(retained_power_fraction > 0.0 && retained_power_fraction <= 1.0) {
        return Err(SpectrumError::FractionOutsideUnitInterval {
            fraction: retained_power_fraction,
        });
    }
    let spectrum = power_spectrum(values, sample_rate_hz)?;
    let wanted = spectrum.total_power() * retained_power_fraction;
    let mut running = 0.0f64;
    for (frequency, power) in spectrum.bin_frequencies_hz.iter().zip(&spectrum.bin_power) {
        running += power;
        if running >= wanted {
            return Ok(*frequency);
        }
    }
    Ok(*spectrum.bin_frequencies_hz.last().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE_HZ: f64 = 1200.0;

    fn sine(frequency_hz: f64, amplitude: f64, sample_count: usize) -> Vec<f64> {
        (0..sample_count)
            .map(|index| {
                amplitude
                    * (2.0 * std::f64::consts::PI * frequency_hz * index as f64 / SAMPLE_RATE_HZ)
                        .sin()
            })
            .collect()
    }

    /// One tone, so the spectrum has one place to put its power and the bin it lands in is
    /// known before the test runs.
    #[test]
    fn a_single_tone_puts_its_power_at_its_own_frequency() {
        let spectrum = power_spectrum(&sine(60.0, 100.0, 4096), SAMPLE_RATE_HZ).unwrap();
        let loudest = spectrum
            .bin_power
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .unwrap()
            .0;
        let frequency = spectrum.bin_frequencies_hz[loudest];
        assert!(
            (frequency - 60.0).abs() <= spectrum.frequency_spacing_hz(),
            "the loudest bin sat at {frequency} Hz"
        );
    }

    /// A plate standing under an athlete reads system weight, and that constant carries
    /// more power than the movement. Removing the mean is what stops a fraction-of-power
    /// rule from reading the athlete's mass.
    #[test]
    fn a_standing_offset_does_not_reach_the_spectrum() {
        let movement = sine(10.0, 50.0, 4096);
        let standing: Vec<f64> = movement.iter().map(|value| value + 600.0).collect();
        let without = power_spectrum(&movement, SAMPLE_RATE_HZ).unwrap();
        let with = power_spectrum(&standing, SAMPLE_RATE_HZ).unwrap();
        assert!((with.total_power() - without.total_power()).abs() / without.total_power() < 1e-9);
    }

    /// Two tones with known power, so the fraction the rule retains decides which of them
    /// the cutoff falls above.
    #[test]
    fn the_retained_fraction_moves_the_cutoff_past_the_higher_tone() {
        let low = sine(8.0, 100.0, 4096);
        let high = sine(120.0, 10.0, 4096);
        let mixed: Vec<f64> = low.iter().zip(&high).map(|(a, b)| a + b).collect();

        let tight = cutoff_retaining_power_fraction(&mixed, SAMPLE_RATE_HZ, 0.90).unwrap();
        let generous = cutoff_retaining_power_fraction(&mixed, SAMPLE_RATE_HZ, 0.999).unwrap();
        assert!(tight < 60.0, "a 90 percent cutoff landed at {tight} Hz");
        assert!(
            generous > 100.0,
            "a 99.9 percent cutoff landed at {generous} Hz"
        );
    }

    #[test]
    fn a_fraction_outside_the_unit_interval_names_the_fraction() {
        let values = sine(10.0, 1.0, 1024);
        let error = cutoff_retaining_power_fraction(&values, SAMPLE_RATE_HZ, 1.5).unwrap_err();
        assert!(error.to_string().contains("1.5"), "{error}");
    }

    /// Parseval, which ties the spectrum back to the trace it came from and fails on a
    /// wrong normalisation or a wrong one-sided fold.
    #[test]
    fn the_spectrums_power_matches_the_traces_own() {
        let values = sine(37.0, 12.0, 4096);
        let spectrum = power_spectrum(&values, SAMPLE_RATE_HZ).unwrap();
        let centre = mean(&values).unwrap();
        let squares: Vec<f64> = values.iter().map(|v| (v - centre).powi(2)).collect();
        let in_the_trace = compensated_sum(&squares) * spectrum.transform_length as f64;
        // Every bin but the two ends stands for a conjugate pair, so the one-sided total
        // counts half the power of each.
        let doubled: f64 = spectrum.bin_power[1..spectrum.bin_power.len() - 1]
            .iter()
            .sum::<f64>()
            * 2.0
            + spectrum.bin_power[0]
            + spectrum.bin_power[spectrum.bin_power.len() - 1];
        assert!(
            (doubled - in_the_trace).abs() / in_the_trace < 1e-9,
            "{doubled} against {in_the_trace}"
        );
    }
}
