//! CLI parsing, application orchestration, and process-level error handling.

#![forbid(unsafe_code)]

mod cli;

use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result};
use clap::Parser;
use signal_filter_shootout::{
    filters::{FilterOutputs, apply_all},
    metrics::{error, snr, spike},
    render::{
        frame::{TraceView, available_width, render_frame},
        report::{TraceMetrics, format_metric_report},
    },
    signal::synthetic::{SyntheticSeries, generate},
};

use crate::cli::{Cli, Command, SimulateRequest};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Command::Simulate(arguments) => run_simulate(arguments.validate()?),
    }
}

fn run_simulate(request: SimulateRequest) -> Result<()> {
    let series = generate(request.synthetic);
    let outputs = apply_all(&series.noisy, request.filters)?;
    let width = request.width.unwrap_or_else(available_width);

    let mut traces = Vec::with_capacity(if request.show_truth { 5 } else { 4 });
    if request.show_truth {
        traces.push(TraceView {
            label: "Truth",
            values: &series.truth,
        });
    }
    traces.extend([
        TraceView {
            label: "Raw",
            values: &outputs.raw,
        },
        TraceView {
            label: "EWMA",
            values: &outputs.ewma,
        },
        TraceView {
            label: "Median",
            values: &outputs.median,
        },
        TraceView {
            label: "Kalman",
            values: &outputs.kalman,
        },
    ]);

    println!("{}", render_frame(&traces, width, request.layout));

    let values = [
        ("Raw", outputs.raw.as_slice()),
        ("EWMA", outputs.ewma.as_slice()),
        ("Median", outputs.median.as_slice()),
        ("Kalman", outputs.kalman.as_slice()),
    ];
    let input_snr = snr::compute(&series.truth, &outputs.raw)?;
    let errors = values
        .iter()
        .map(|(_, values)| error::compute(&series.truth, values))
        .collect::<Result<Vec<_>, _>>()?;
    let snrs = values
        .iter()
        .map(|(_, values)| snr::compute(&series.truth, values))
        .collect::<Result<Vec<_>, _>>()?;
    let spikes = values
        .iter()
        .map(|(_, values)| {
            spike::compute(&series.truth, values, &series.spike_mask, request.recovery)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let report_rows = values
        .iter()
        .enumerate()
        .map(|(index, (label, _))| TraceMetrics {
            label,
            error: errors[index],
            snr: snrs[index],
            improvement: snr::improvement(input_snr, snrs[index]),
            spike: &spikes[index],
        })
        .collect::<Vec<_>>();

    println!();
    print!("{}", format_metric_report(&report_rows));

    if let Some(path) = request.report_csv {
        write_sample_report(&path, &series, &outputs)
            .with_context(|| format!("failed to write report '{}'", path.display()))?;
        println!("Per-sample CSV: {}", path.display());
    }

    Ok(())
}

fn write_sample_report(
    path: &Path,
    series: &SyntheticSeries,
    outputs: &FilterOutputs,
) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(path)?);
    writeln!(writer, "sample,time,truth,raw,ewma,median,kalman,is_spike")?;

    for index in 0..series.truth.len() {
        writeln!(
            writer,
            "{index},{},{},{},{},{},{},{}",
            series.time[index],
            series.truth[index],
            outputs.raw[index],
            outputs.ewma[index],
            outputs.median[index],
            outputs.kalman[index],
            series.spike_mask[index],
        )?;
    }

    writer.flush()?;
    Ok(())
}
