//! Per-channel application of the core filters to audio and compare orchestration.

use thiserror::Error;

use crate::filters::{
    FilterError, OnlineFilter, ShootoutConfig, ewma::EwmaFilter, kalman::ScalarKalmanFilter,
    median::MedianFilter,
};

use super::wav::{AudioBuffer, WavError};

/// Filter selected for a single audio output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFilterKind {
    Ewma,
    Median,
    Kalman,
}

/// All three independently filtered versions of one input buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioComparison {
    pub ewma: AudioBuffer,
    pub median: AudioBuffer,
    pub kalman: AudioBuffer,
}

/// Audio filtering and reference-compatibility failures.
#[derive(Debug, Error)]
pub enum AudioProcessError {
    #[error(transparent)]
    Filter(#[from] FilterError),

    #[error(transparent)]
    Wav(#[from] WavError),

    #[error("reference sample rate mismatch: input is {input} Hz, reference is {reference} Hz")]
    ReferenceSampleRate { input: u32, reference: u32 },

    #[error("reference channel count mismatch: input has {input}, reference has {reference}")]
    ReferenceChannels { input: u16, reference: u16 },

    #[error("reference sample count mismatch: input has {input}, reference has {reference}")]
    ReferenceSampleCount { input: usize, reference: usize },
}

/// Apply one selected filter with an independent state instance per channel.
pub fn filter_audio(
    audio: &AudioBuffer,
    kind: AudioFilterKind,
    config: ShootoutConfig,
) -> Result<AudioBuffer, AudioProcessError> {
    match kind {
        AudioFilterKind::Ewma => apply_per_channel(audio, || EwmaFilter::new(config.ewma)),
        AudioFilterKind::Median => apply_per_channel(audio, || MedianFilter::new(config.median)),
        AudioFilterKind::Kalman => {
            apply_per_channel(audio, || ScalarKalmanFilter::new(config.kalman))
        }
    }
}

/// Apply EWMA, median, and Kalman independently to the same audio.
pub fn compare_audio(
    audio: &AudioBuffer,
    config: ShootoutConfig,
) -> Result<AudioComparison, AudioProcessError> {
    Ok(AudioComparison {
        ewma: filter_audio(audio, AudioFilterKind::Ewma, config)?,
        median: filter_audio(audio, AudioFilterKind::Median, config)?,
        kalman: filter_audio(audio, AudioFilterKind::Kalman, config)?,
    })
}

/// Require an exact metadata and interleaved-sample-count match.
pub fn validate_reference(
    input: &AudioBuffer,
    reference: &AudioBuffer,
) -> Result<(), AudioProcessError> {
    if input.sample_rate != reference.sample_rate {
        return Err(AudioProcessError::ReferenceSampleRate {
            input: input.sample_rate,
            reference: reference.sample_rate,
        });
    }
    if input.channels != reference.channels {
        return Err(AudioProcessError::ReferenceChannels {
            input: input.channels,
            reference: reference.channels,
        });
    }
    if input.interleaved.len() != reference.interleaved.len() {
        return Err(AudioProcessError::ReferenceSampleCount {
            input: input.interleaved.len(),
            reference: reference.interleaved.len(),
        });
    }
    Ok(())
}

fn apply_per_channel<F>(
    audio: &AudioBuffer,
    mut create_filter: impl FnMut() -> F,
) -> Result<AudioBuffer, AudioProcessError>
where
    F: OnlineFilter,
{
    let channels = usize::from(audio.channels);
    let mut filters = (0..channels).map(|_| create_filter()).collect::<Vec<_>>();
    let mut interleaved = Vec::with_capacity(audio.interleaved.len());

    for (index, &sample) in audio.interleaved.iter().enumerate() {
        interleaved.push(filters[index % channels].update(sample)?);
    }

    Ok(AudioBuffer::new(
        audio.sample_rate,
        audio.channels,
        interleaved,
    )?)
}

#[cfg(test)]
mod tests {
    use super::{
        AudioFilterKind, AudioProcessError, compare_audio, filter_audio, validate_reference,
    };
    use crate::{
        audio::wav::AudioBuffer,
        filters::{ShootoutConfig, ewma::EwmaConfig, kalman::KalmanConfig, median::MedianConfig},
    };

    fn config(alpha: f64, median_window: usize) -> ShootoutConfig {
        ShootoutConfig {
            ewma: EwmaConfig::new(alpha).unwrap(),
            median: MedianConfig::new(median_window).unwrap(),
            kalman: KalmanConfig::new(0.001, 0.04, 1.0).unwrap(),
        }
    }

    #[test]
    fn stereo_channels_use_independent_filter_state() {
        let audio = AudioBuffer::new(16_000, 2, vec![0.0, 100.0, 10.0, 100.0]).unwrap();

        let filtered = filter_audio(&audio, AudioFilterKind::Ewma, config(0.5, 3)).unwrap();

        assert_eq!(filtered.interleaved, [0.0, 100.0, 5.0, 100.0]);
    }

    #[test]
    fn every_output_preserves_metadata_and_sample_count() {
        let audio = AudioBuffer::new(16_000, 2, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();

        let outputs = compare_audio(&audio, config(0.2, 3)).unwrap();

        for output in [&outputs.ewma, &outputs.median, &outputs.kalman] {
            assert_eq!(output.sample_rate, audio.sample_rate);
            assert_eq!(output.channels, audio.channels);
            assert_eq!(output.interleaved.len(), audio.interleaved.len());
        }
    }

    #[test]
    fn constant_audio_through_median_remains_constant() {
        let audio = AudioBuffer::new(8_000, 1, vec![0.25; 12]).unwrap();

        let filtered = filter_audio(&audio, AudioFilterKind::Median, config(0.2, 5)).unwrap();

        assert_eq!(filtered.interleaved, audio.interleaved);
    }

    #[test]
    fn alpha_one_ewma_preserves_samples() {
        let audio = AudioBuffer::new(8_000, 1, vec![-1.0, -0.25, 0.0, 0.75]).unwrap();

        let filtered = filter_audio(&audio, AudioFilterKind::Ewma, config(1.0, 3)).unwrap();

        assert_eq!(filtered.interleaved, audio.interleaved);
    }

    #[test]
    fn reference_requires_exact_compatibility() {
        let input = AudioBuffer::new(16_000, 1, vec![0.0, 1.0]).unwrap();
        let wrong_rate = AudioBuffer::new(8_000, 1, vec![0.0, 1.0]).unwrap();
        let wrong_channels = AudioBuffer::new(16_000, 2, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
        let wrong_count = AudioBuffer::new(16_000, 1, vec![0.0]).unwrap();

        assert!(matches!(
            validate_reference(&input, &wrong_rate),
            Err(AudioProcessError::ReferenceSampleRate { .. })
        ));
        assert!(matches!(
            validate_reference(&input, &wrong_channels),
            Err(AudioProcessError::ReferenceChannels { .. })
        ));
        assert!(matches!(
            validate_reference(&input, &wrong_count),
            Err(AudioProcessError::ReferenceSampleCount { .. })
        ));
        assert!(validate_reference(&input, &input).is_ok());
    }
}
