//! Signed PCM16 WAV decoding, encoding, and channel-shape handling.

use std::{
    io::{Read, Seek, Write},
    path::Path,
};

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use thiserror::Error;

/// Validated normalized interleaved mono or stereo audio.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub interleaved: Vec<f64>,
}

impl AudioBuffer {
    /// Construct a finite, non-empty, frame-aligned audio buffer.
    pub fn new(sample_rate: u32, channels: u16, interleaved: Vec<f64>) -> Result<Self, WavError> {
        validate_metadata(sample_rate, channels, &interleaved)?;
        Ok(Self {
            sample_rate,
            channels,
            interleaved,
        })
    }

    /// Return the number of frames, excluding channel interleaving.
    pub fn frame_count(&self) -> usize {
        self.interleaved.len() / usize::from(self.channels)
    }

    /// Copy one channel from the interleaved buffer.
    pub fn channel(&self, channel: u16) -> Result<Vec<f64>, WavError> {
        if channel >= self.channels {
            return Err(WavError::InvalidChannel {
                channel,
                channels: self.channels,
            });
        }
        Ok(self
            .interleaved
            .iter()
            .skip(usize::from(channel))
            .step_by(usize::from(self.channels))
            .copied()
            .collect())
    }

    /// Build an interleaved buffer from equally sized mono channel vectors.
    pub fn from_channels(sample_rate: u32, channels: &[Vec<f64>]) -> Result<Self, WavError> {
        let channel_count = u16::try_from(channels.len())
            .map_err(|_| WavError::InvalidChannels { channels: u16::MAX })?;
        if channels.is_empty() || channels.len() > 2 {
            return Err(WavError::InvalidChannels {
                channels: channel_count,
            });
        }
        let frames = channels[0].len();
        if channels.iter().any(|channel| channel.len() != frames) {
            return Err(WavError::UnequalChannelLengths);
        }

        let mut interleaved = Vec::with_capacity(frames * channels.len());
        for frame in 0..frames {
            for channel in channels {
                interleaved.push(channel[frame]);
            }
        }
        Self::new(sample_rate, channel_count, interleaved)
    }
}

/// WAV format, shape, and sample errors.
#[derive(Debug, Error)]
pub enum WavError {
    #[error("WAV I/O failed: {0}")]
    Hound(#[from] hound::Error),

    #[error(
        "unsupported WAV format: expected signed PCM16, got {sample_format:?} {bits_per_sample}-bit"
    )]
    UnsupportedFormat {
        sample_format: SampleFormat,
        bits_per_sample: u16,
    },

    #[error("unsupported channel count {channels}; expected mono or stereo")]
    InvalidChannels { channels: u16 },

    #[error("sample rate must be greater than 0")]
    InvalidSampleRate,

    #[error("audio contains no samples")]
    Empty,

    #[error("interleaved sample count {samples} is not divisible by {channels} channels")]
    InvalidShape { samples: usize, channels: u16 },

    #[error("audio contains a non-finite sample at interleaved index {index}")]
    NonFiniteSample { index: usize },

    #[error("requested channel {channel} does not exist in {channels}-channel audio")]
    InvalidChannel { channel: u16, channels: u16 },

    #[error("channel vectors must have equal lengths")]
    UnequalChannelLengths,
}

/// Decode a PCM16 WAV file into normalized interleaved samples.
pub fn read_path(path: impl AsRef<Path>) -> Result<AudioBuffer, WavError> {
    decode_reader(WavReader::open(path)?)
}

/// Decode normalized samples from an already opened WAV reader.
pub fn decode_reader<R: Read>(mut reader: WavReader<R>) -> Result<AudioBuffer, WavError> {
    let spec = reader.spec();
    validate_spec(spec)?;

    let interleaved = reader
        .samples::<i16>()
        .map(|sample| sample.map(normalize_pcm16).map_err(WavError::from))
        .collect::<Result<Vec<_>, _>>()?;

    AudioBuffer::new(spec.sample_rate, spec.channels, interleaved)
}

/// Encode normalized interleaved samples as signed PCM16 WAV.
pub fn write_path(path: impl AsRef<Path>, audio: &AudioBuffer) -> Result<(), WavError> {
    let writer = WavWriter::create(path, pcm16_spec(audio))?;
    encode_writer(writer, audio)
}

/// Encode into an already opened WAV writer.
pub fn encode_writer<W: Write + Seek>(
    mut writer: WavWriter<W>,
    audio: &AudioBuffer,
) -> Result<(), WavError> {
    validate_metadata(audio.sample_rate, audio.channels, &audio.interleaved)?;
    for &sample in &audio.interleaved {
        writer.write_sample(quantize_pcm16(sample))?;
    }
    writer.finalize()?;
    Ok(())
}

/// Normalize one signed PCM16 sample to the documented scalar range.
pub fn normalize_pcm16(sample: i16) -> f64 {
    f64::from(sample) / 32768.0
}

/// Clamp and quantize one normalized sample to signed PCM16.
pub fn quantize_pcm16(sample: f64) -> i16 {
    let scaled = (sample.clamp(-1.0, 1.0) * 32768.0).round();
    scaled.clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

fn pcm16_spec(audio: &AudioBuffer) -> WavSpec {
    WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    }
}

fn validate_spec(spec: WavSpec) -> Result<(), WavError> {
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(WavError::UnsupportedFormat {
            sample_format: spec.sample_format,
            bits_per_sample: spec.bits_per_sample,
        });
    }
    if !(1..=2).contains(&spec.channels) {
        return Err(WavError::InvalidChannels {
            channels: spec.channels,
        });
    }
    if spec.sample_rate == 0 {
        return Err(WavError::InvalidSampleRate);
    }
    Ok(())
}

fn validate_metadata(sample_rate: u32, channels: u16, samples: &[f64]) -> Result<(), WavError> {
    if sample_rate == 0 {
        return Err(WavError::InvalidSampleRate);
    }
    if !(1..=2).contains(&channels) {
        return Err(WavError::InvalidChannels { channels });
    }
    if samples.is_empty() {
        return Err(WavError::Empty);
    }
    if !samples.len().is_multiple_of(usize::from(channels)) {
        return Err(WavError::InvalidShape {
            samples: samples.len(),
            channels,
        });
    }
    if let Some((index, _)) = samples
        .iter()
        .enumerate()
        .find(|(_, sample)| !sample.is_finite())
    {
        return Err(WavError::NonFiniteSample { index });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

    use super::{
        AudioBuffer, WavError, decode_reader, encode_writer, normalize_pcm16, quantize_pcm16,
    };

    fn encode(audio: &AudioBuffer) -> Vec<u8> {
        let mut bytes = Vec::new();
        let writer = WavWriter::new(
            Cursor::new(&mut bytes),
            WavSpec {
                channels: audio.channels,
                sample_rate: audio.sample_rate,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            },
        )
        .unwrap();
        encode_writer(writer, audio).unwrap();
        bytes
    }

    #[test]
    fn pcm16_round_trip_preserves_metadata_and_samples() {
        let audio = AudioBuffer::new(16_000, 2, vec![-1.0, 0.0, 0.5, 32767.0 / 32768.0]).unwrap();
        let decoded = decode_reader(WavReader::new(Cursor::new(encode(&audio))).unwrap()).unwrap();

        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.frame_count(), 2);
        assert_eq!(decoded.interleaved, audio.interleaved);
    }

    #[test]
    fn normalized_conversion_endpoints_are_explicit() {
        assert_eq!(normalize_pcm16(i16::MIN), -1.0);
        assert_eq!(normalize_pcm16(i16::MAX), 32767.0 / 32768.0);
        assert_eq!(quantize_pcm16(-2.0), i16::MIN);
        assert_eq!(quantize_pcm16(2.0), i16::MAX);
        assert_eq!(quantize_pcm16(0.5), 16_384);
    }

    #[test]
    fn mono_and_stereo_channel_conversion_round_trips() {
        let audio = AudioBuffer::from_channels(8_000, &[vec![1.0, 2.0], vec![-1.0, -2.0]]).unwrap();

        assert_eq!(audio.interleaved, [1.0, -1.0, 2.0, -2.0]);
        assert_eq!(audio.channel(0).unwrap(), [1.0, 2.0]);
        assert_eq!(audio.channel(1).unwrap(), [-1.0, -2.0]);
    }

    #[test]
    fn rejects_unsupported_format() {
        let mut bytes = Vec::new();
        {
            let mut writer = WavWriter::new(
                Cursor::new(&mut bytes),
                WavSpec {
                    channels: 1,
                    sample_rate: 16_000,
                    bits_per_sample: 32,
                    sample_format: SampleFormat::Float,
                },
            )
            .unwrap();
            writer.write_sample(0.0_f32).unwrap();
            writer.finalize().unwrap();
        }

        assert!(matches!(
            decode_reader(WavReader::new(Cursor::new(bytes)).unwrap()),
            Err(WavError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn rejects_invalid_shapes_and_samples() {
        assert!(matches!(
            AudioBuffer::new(16_000, 2, vec![0.0]),
            Err(WavError::InvalidShape { .. })
        ));
        assert!(matches!(
            AudioBuffer::new(16_000, 1, vec![f64::NAN]),
            Err(WavError::NonFiniteSample { .. })
        ));
        assert!(matches!(
            AudioBuffer::from_channels(16_000, &[vec![0.0], vec![0.0, 1.0]]),
            Err(WavError::UnequalChannelLengths)
        ));
    }
}
