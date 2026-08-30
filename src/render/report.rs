//! Metric-table and winner formatting without metric computation.

use crate::metrics::{
    error::ErrorMetrics,
    snr::{SnrImprovement, SnrValue},
    spike::SpikeMetrics,
};

/// Precomputed metrics for one labeled trace.
#[derive(Debug, Clone, Copy)]
pub struct TraceMetrics<'a> {
    pub label: &'a str,
    pub error: ErrorMetrics,
    pub snr: SnrValue,
    pub improvement: SnrImprovement,
    pub spike: &'a SpikeMetrics,
}

/// Format a compact sensor comparison report.
pub fn format_metric_report(traces: &[TraceMetrics<'_>]) -> String {
    let mut output =
        String::from("Trace    RMSE       MAE        MaxAbs     SNR(dB)    ΔSNR(dB)\n");
    for trace in traces {
        output.push_str(&format!(
            "{:<8} {:>10.6} {:>10.6} {:>10.6} {:>10} {:>11}\n",
            trace.label,
            trace.error.rmse,
            trace.error.mae,
            trace.error.max_abs,
            format_snr(trace.snr),
            format_improvement(trace.improvement),
        ));
    }

    if let (Some(rmse), Some(mae), Some(max_abs)) = (
        winner(traces, |trace| trace.error.rmse),
        winner(traces, |trace| trace.error.mae),
        winner(traces, |trace| trace.error.max_abs),
    ) {
        output.push_str(&format!(
            "Winners: RMSE={}  MAE={}  MaxAbs={}\n",
            rmse.label, mae.label, max_abs.label
        ));
    }

    output.push_str("Spike metrics:\n");
    for trace in traces {
        output.push_str(&format!(
            "  {:<8} count={} spike_rmse={} recovery_mean={} recovery_max={} unrecovered={}\n",
            trace.label,
            trace.spike.count,
            format_optional_f64(trace.spike.spike_rmse),
            format_optional_f64(trace.spike.mean_recovery_samples),
            trace
                .spike
                .max_recovery_samples
                .map_or_else(|| "n/a".to_owned(), |value| value.to_string()),
            trace.spike.unrecovered_count,
        ));
    }

    output
}

fn winner<'a>(
    traces: &'a [TraceMetrics<'a>],
    metric: impl Fn(&TraceMetrics<'_>) -> f64,
) -> Option<&'a TraceMetrics<'a>> {
    traces
        .iter()
        .min_by(|left, right| metric(left).total_cmp(&metric(right)))
}

fn format_snr(value: SnrValue) -> String {
    match value {
        SnrValue::Finite(value) => format!("{value:.3}"),
        SnrValue::Infinite => "inf".to_owned(),
        SnrValue::Undefined => "n/a".to_owned(),
    }
}

fn format_improvement(value: SnrImprovement) -> String {
    match value {
        SnrImprovement::Finite(value) => format!("{value:.3}"),
        SnrImprovement::PositiveInfinity => "inf".to_owned(),
        SnrImprovement::NegativeInfinity => "-inf".to_owned(),
        SnrImprovement::Undefined => "n/a".to_owned(),
    }
}

fn format_optional_f64(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_owned(), |value| format!("{value:.3}"))
}

#[cfg(test)]
mod tests {
    use super::{TraceMetrics, format_metric_report};
    use crate::metrics::{
        error::ErrorMetrics,
        snr::{SnrImprovement, SnrValue},
        spike::SpikeMetrics,
    };

    #[test]
    fn report_contains_metrics_winners_and_spikes() {
        let spikes = SpikeMetrics {
            count: 1,
            spike_rmse: Some(2.0),
            recovered_count: 1,
            unrecovered_count: 0,
            mean_recovery_samples: Some(3.0),
            max_recovery_samples: Some(3),
        };
        let traces = [
            TraceMetrics {
                label: "Raw",
                error: ErrorMetrics {
                    rmse: 2.0,
                    mae: 1.0,
                    max_abs: 3.0,
                },
                snr: SnrValue::Finite(1.0),
                improvement: SnrImprovement::Finite(0.0),
                spike: &spikes,
            },
            TraceMetrics {
                label: "EWMA",
                error: ErrorMetrics {
                    rmse: 1.0,
                    mae: 0.5,
                    max_abs: 2.0,
                },
                snr: SnrValue::Infinite,
                improvement: SnrImprovement::PositiveInfinity,
                spike: &spikes,
            },
        ];

        let report = format_metric_report(&traces);

        assert!(report.contains("Trace"));
        assert!(report.contains("Winners: RMSE=EWMA"));
        assert!(report.contains("Spike metrics:"));
    }
}
