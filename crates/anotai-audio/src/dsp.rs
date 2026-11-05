//! Processamento puro de amostras (sem dispositivo).

use cpal::{FromSample, Sample};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::{Error, Result};

/// Converte quadros intercalados de `channels` canais em mono `f32`
/// (média dos canais) e acrescenta em `out`.
pub fn downmix_into<T>(data: &[T], channels: usize, out: &mut Vec<f32>)
where
    T: Sample,
    f32: FromSample<T>,
{
    let channels = channels.max(1);
    if channels == 1 {
        out.extend(data.iter().map(|&s| f32::from_sample(s)));
        return;
    }
    let scale = 1.0 / channels as f32;
    out.extend(
        data.chunks_exact(channels)
            .map(|frame| frame.iter().map(|&s| f32::from_sample(s)).sum::<f32>() * scale),
    );
}

/// Valor RMS (0 = silêncio, ~0,707 = senoide em escala cheia).
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

/// Reamostra um trecho mono inteiro de `from_rate` para `to_rate`.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
    if from_rate == to_rate || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    let input = InterleavedSlice::new(samples, 1, samples.len())
        .map_err(|e| Error::Resample(e.to_string()))?;
    let mut resampler = Fft::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        1024,
        1,
        FixedSync::Input,
    )
    .map_err(|e| Error::Resample(e.to_string()))?;
    let output = resampler
        .process_all(&input, samples.len(), None)
        .map_err(|e| Error::Resample(e.to_string()))?;
    let mut data = output.take_data();
    let expected =
        (samples.len() as u64 * u64::from(to_rate)).div_ceil(u64::from(from_rate)) as usize;
    data.truncate(expected);
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: u32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn downmix_averages_stereo_frames() {
        let mut out = Vec::new();
        downmix_into(&[0.5f32, -0.5, 1.0, 0.0], 2, &mut out);
        assert_eq!(out, [0.0, 0.5]);
    }

    #[test]
    fn downmix_converts_i16() {
        let mut out = Vec::new();
        downmix_into(&[i16::MAX, 0i16], 1, &mut out);
        assert!((out[0] - 1.0).abs() < 1e-3);
        assert_eq!(out[1], 0.0);
    }

    #[test]
    fn rms_of_full_scale_sine() {
        assert!((rms(&sine(440.0, 48_000, 0.5)) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn resample_48k_to_16k_keeps_length_and_level() -> Result<()> {
        let input = sine(440.0, 48_000, 1.0);
        let out = resample(&input, 48_000, 16_000)?;
        assert_eq!(out.len(), 16_000);
        // Ignora as bordas (transitório do filtro).
        let mid = &out[2_000..14_000];
        assert!((rms(mid) - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02);
        Ok(())
    }

    #[test]
    fn resample_44k1_to_16k() -> Result<()> {
        let out = resample(&sine(300.0, 44_100, 2.0), 44_100, 16_000)?;
        assert_eq!(out.len(), 32_000);
        Ok(())
    }

    #[test]
    fn same_rate_is_passthrough() -> Result<()> {
        assert_eq!(resample(&[0.1, 0.2], 16_000, 16_000)?, [0.1, 0.2]);
        Ok(())
    }
}
