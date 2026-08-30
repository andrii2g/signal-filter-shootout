//! Clap argument definitions and conversion into validated requests.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use signal_filter_shootout::{
    audio::{noise::AudioNoiseConfig, process::AudioFilterKind},
    error::{ConfigError, ConfigResult},
    filters::{ShootoutConfig, ewma::EwmaConfig, kalman::KalmanConfig, median::MedianConfig},
    metrics::spike::RecoveryConfig,
    render::frame::Layout,
    signal::{
        csv::CsvReadOptions,
        synthetic::{NoiseConfig, SineConfig, SyntheticConfig},
    },
    tuning::grid_search::GridSearchConfig,
};

/// Compare scalar online filters on sensor data and PCM WAV audio.
#[derive(Debug, Parser)]
#[command(name = "signal-filter-shootout", version, about)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Compare all filters on deterministic noisy sine-wave data.
    Simulate(SimulateArgs),
    /// Apply all filters to a headered scalar CSV file.
    Csv(CsvArgs),
    /// Work with PCM16 WAV audio.
    Audio(AudioArgs),
}

#[derive(Debug, Args)]
pub(crate) struct AudioArgs {
    #[command(subcommand)]
    pub command: AudioCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AudioCommand {
    /// Inject deterministic Gaussian and impulse noise into PCM16 WAV audio.
    InjectNoise(AudioInjectNoiseArgs),
    /// Apply one filter to PCM16 WAV audio.
    Filter(AudioFilterArgs),
    /// Apply and compare all three filters on PCM16 WAV audio.
    Compare(AudioCompareArgs),
}

#[derive(Debug, Args)]
pub(crate) struct AudioFilterArgs {
    input: PathBuf,
    output: PathBuf,
    #[arg(long, value_enum)]
    kind: CliAudioFilterKind,
    #[command(flatten)]
    filters: FilterArgs,
}

#[derive(Debug, Args)]
pub(crate) struct AudioCompareArgs {
    input: PathBuf,
    #[arg(long)]
    reference: Option<PathBuf>,
    #[arg(long, default_value = "out/audio")]
    output_dir: PathBuf,
    #[command(flatten)]
    filters: FilterArgs,
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    window_start_ms: f64,
    #[arg(long, default_value_t = 30.0, allow_hyphen_values = true)]
    window_duration_ms: f64,
    #[arg(long)]
    width: Option<usize>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliAudioFilterKind {
    Ewma,
    Median,
    Kalman,
}

#[derive(Debug, Args)]
pub(crate) struct AudioInjectNoiseArgs {
    input: PathBuf,
    output: PathBuf,
    #[arg(long, default_value_t = 0.03, allow_hyphen_values = true)]
    gaussian_sigma: f64,
    #[arg(long, default_value_t = 0.0008, allow_hyphen_values = true)]
    spike_probability: f64,
    #[arg(long, default_value_t = 0.85, allow_hyphen_values = true)]
    spike_amplitude: f64,
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

#[derive(Debug, Args)]
pub(crate) struct SimulateArgs {
    #[arg(long, default_value_t = 1000)]
    samples: usize,
    #[arg(long, default_value_t = 1.0)]
    amplitude: f64,
    /// Sine cycles per sample.
    #[arg(long = "frequency", default_value_t = 0.02)]
    frequency: f64,
    /// Initial sine phase in radians.
    #[arg(long, default_value_t = 0.0)]
    phase: f64,
    #[arg(long, default_value_t = 0.20)]
    gaussian_sigma: f64,
    #[arg(long, default_value_t = 0.015)]
    spike_probability: f64,
    #[arg(long, default_value_t = 2.5)]
    spike_amplitude: f64,
    #[arg(long, default_value_t = 42)]
    seed: u64,

    #[command(flatten)]
    filters: FilterArgs,

    /// Explicit terminal width; must be at least 40.
    #[arg(long)]
    width: Option<usize>,
    #[arg(long, value_enum, default_value_t = CliLayout::Auto)]
    layout: CliLayout,
    /// Include the clean truth trace in the terminal frame.
    #[arg(long)]
    show_truth: bool,
    /// Write one CSV row per generated sample.
    #[arg(long)]
    report_csv: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct CsvArgs {
    /// Input headered CSV file.
    input: PathBuf,
    #[arg(long, default_value = "value")]
    value_column: String,
    #[arg(long, default_value = "timestamp")]
    time_column: String,
    #[arg(long, default_value = "reference")]
    reference_column: String,

    #[command(flatten)]
    filters: FilterArgs,

    /// Explicit terminal width; must be at least 40.
    #[arg(long)]
    width: Option<usize>,
    #[arg(long, value_enum, default_value_t = CliLayout::Auto)]
    layout: CliLayout,

    /// Search the configured logarithmic Kalman Q/R grid.
    #[arg(long)]
    auto_tune_kalman: bool,
    #[arg(long, default_value_t = -8, allow_hyphen_values = true)]
    q_min_exp: i32,
    #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
    q_max_exp: i32,
    #[arg(long, default_value_t = -6, allow_hyphen_values = true)]
    r_min_exp: i32,
    #[arg(long, default_value_t = 1, allow_hyphen_values = true)]
    r_max_exp: i32,
    #[arg(long, value_delimiter = ',', default_value = "1,3")]
    grid_multipliers: Vec<f64>,
    #[arg(long, default_value_t = 5)]
    top: usize,
    /// Write every ranked tuning candidate to CSV.
    #[arg(long)]
    tuning_csv: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct FilterArgs {
    #[arg(long, default_value_t = 0.20)]
    ewma_alpha: f64,
    #[arg(long, default_value_t = 5)]
    median_window: usize,
    #[arg(long, default_value_t = 0.001)]
    kalman_q: f64,
    #[arg(long, default_value_t = 0.04)]
    kalman_r: f64,
    #[arg(long, default_value_t = 1.0)]
    kalman_p0: f64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliLayout {
    Auto,
    Columns,
    Rows,
}

pub(crate) struct SimulateRequest {
    pub synthetic: SyntheticConfig,
    pub filters: ShootoutConfig,
    pub recovery: RecoveryConfig,
    pub width: Option<usize>,
    pub layout: Layout,
    pub show_truth: bool,
    pub report_csv: Option<PathBuf>,
}

pub(crate) struct CsvRequest {
    pub input: PathBuf,
    pub read_options: CsvReadOptions,
    pub filters: ShootoutConfig,
    pub width: Option<usize>,
    pub layout: Layout,
    pub tuning: Option<TuningRequest>,
}

pub(crate) struct TuningRequest {
    pub grid: GridSearchConfig,
    pub output_csv: Option<PathBuf>,
}

pub(crate) struct AudioInjectNoiseRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub noise: AudioNoiseConfig,
}

pub(crate) struct AudioFilterRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub kind: AudioFilterKind,
    pub filters: ShootoutConfig,
}

pub(crate) struct AudioCompareRequest {
    pub input: PathBuf,
    pub reference: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub filters: ShootoutConfig,
    pub window_start_ms: f64,
    pub window_duration_ms: f64,
    pub width: Option<usize>,
}

impl SimulateArgs {
    pub fn validate(self) -> ConfigResult<SimulateRequest> {
        validate_width(self.width)?;
        let sine = SineConfig::new(self.samples, self.amplitude, self.frequency, self.phase)?;
        let noise = NoiseConfig::new(
            self.gaussian_sigma,
            self.spike_probability,
            self.spike_amplitude,
        )?;
        let tolerance = 0.10 * self.amplitude.abs().max(1.0);

        Ok(SimulateRequest {
            synthetic: SyntheticConfig {
                sine,
                noise,
                seed: self.seed,
            },
            filters: self.filters.validate()?,
            recovery: RecoveryConfig::new(tolerance, 3)?,
            width: self.width,
            layout: self.layout.into(),
            show_truth: self.show_truth,
            report_csv: self.report_csv,
        })
    }
}

impl CsvArgs {
    pub fn validate(self) -> ConfigResult<CsvRequest> {
        validate_width(self.width)?;
        let tuning = self
            .auto_tune_kalman
            .then(|| {
                GridSearchConfig::new(
                    self.q_min_exp,
                    self.q_max_exp,
                    self.r_min_exp,
                    self.r_max_exp,
                    self.grid_multipliers,
                    self.top,
                )
                .map(|grid| TuningRequest {
                    grid,
                    output_csv: self.tuning_csv,
                })
            })
            .transpose()?;

        Ok(CsvRequest {
            input: self.input,
            read_options: CsvReadOptions {
                value_column: self.value_column,
                time_column: self.time_column,
                reference_column: self.reference_column,
            },
            filters: self.filters.validate()?,
            width: self.width,
            layout: self.layout.into(),
            tuning,
        })
    }
}

impl AudioInjectNoiseArgs {
    pub fn validate(self) -> ConfigResult<AudioInjectNoiseRequest> {
        Ok(AudioInjectNoiseRequest {
            input: self.input,
            output: self.output,
            noise: AudioNoiseConfig::new(
                self.gaussian_sigma,
                self.spike_probability,
                self.spike_amplitude,
                self.seed,
            )?,
        })
    }
}

impl AudioFilterArgs {
    pub fn validate(self) -> ConfigResult<AudioFilterRequest> {
        Ok(AudioFilterRequest {
            input: self.input,
            output: self.output,
            kind: self.kind.into(),
            filters: self.filters.validate()?,
        })
    }
}

impl AudioCompareArgs {
    pub fn validate(self) -> ConfigResult<AudioCompareRequest> {
        validate_width(self.width)?;
        if !self.window_start_ms.is_finite() {
            return Err(ConfigError::NonFinite {
                parameter: "window_start_ms",
            });
        }
        if self.window_start_ms < 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "window_start_ms",
                requirement: "must be at least 0",
            });
        }
        if !self.window_duration_ms.is_finite() {
            return Err(ConfigError::NonFinite {
                parameter: "window_duration_ms",
            });
        }
        if self.window_duration_ms <= 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "window_duration_ms",
                requirement: "must be greater than 0",
            });
        }

        Ok(AudioCompareRequest {
            input: self.input,
            reference: self.reference,
            output_dir: self.output_dir,
            filters: self.filters.validate()?,
            window_start_ms: self.window_start_ms,
            window_duration_ms: self.window_duration_ms,
            width: self.width,
        })
    }
}

impl From<CliAudioFilterKind> for AudioFilterKind {
    fn from(value: CliAudioFilterKind) -> Self {
        match value {
            CliAudioFilterKind::Ewma => Self::Ewma,
            CliAudioFilterKind::Median => Self::Median,
            CliAudioFilterKind::Kalman => Self::Kalman,
        }
    }
}

impl FilterArgs {
    fn validate(self) -> ConfigResult<ShootoutConfig> {
        Ok(ShootoutConfig {
            ewma: EwmaConfig::new(self.ewma_alpha)?,
            median: MedianConfig::new(self.median_window)?,
            kalman: KalmanConfig::new(self.kalman_q, self.kalman_r, self.kalman_p0)?,
        })
    }
}

impl From<CliLayout> for Layout {
    fn from(value: CliLayout) -> Self {
        match value {
            CliLayout::Auto => Self::Auto,
            CliLayout::Columns => Self::Columns,
            CliLayout::Rows => Self::Rows,
        }
    }
}

fn validate_width(width: Option<usize>) -> ConfigResult<()> {
    if width.is_some_and(|width| width < 40) {
        Err(ConfigError::InvalidValue {
            parameter: "width",
            requirement: "must be at least 40",
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn help_contains_implemented_commands() {
        let mut command = Cli::command();
        let mut help = Vec::new();

        command.write_long_help(&mut help).expect("write help");
        let help = String::from_utf8(help).expect("help is UTF-8");

        assert!(help.contains("signal-filter-shootout"));
        assert!(help.contains("simulate"));
        assert!(help.contains("csv"));
        assert!(help.contains("audio"));
    }
}
