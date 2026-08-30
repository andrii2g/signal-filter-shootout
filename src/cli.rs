//! Clap argument definitions and conversion into validated requests.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use signal_filter_shootout::{
    error::{ConfigError, ConfigResult},
    filters::{ShootoutConfig, ewma::EwmaConfig, kalman::KalmanConfig, median::MedianConfig},
    metrics::spike::RecoveryConfig,
    render::frame::Layout,
    signal::{
        csv::CsvReadOptions,
        synthetic::{NoiseConfig, SineConfig, SyntheticConfig},
    },
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
        })
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
    }
}
