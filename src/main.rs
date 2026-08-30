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
    metrics::{error, snr, spike::SpikeMetrics},
    render::{
        frame::{TraceView, available_width, render_frame},
        report::{TraceMetrics, format_metric_report},
    },
    signal::{
        csv::read_path,
        synthetic::{SyntheticSeries, generate},
    },
};

use crate::cli::{Cli, Command, CsvRequest, SimulateRequest};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Command::Simulate(arguments) => run_simulate(arguments.validate()?),
        Command::Csv(arguments) => run_csv(arguments.validate()?),
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
    traces.extend(trace_views(&outputs));
    println!("{}", render_frame(&traces, width, request.layout));

    let values = labeled_values(&outputs);
    let spikes = values
        .iter()
        .map(|(_, values)| {
            signal_filter_shootout::metrics::spike::compute(
                &series.truth,
                values,
                &series.spike_mask,
                request.recovery,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    println!();
    print!("{}", metric_report(&series.truth, &values, Some(&spikes))?);

    if let Some(path) = request.report_csv {
        write_sample_report(&path, &series, &outputs)
            .with_context(|| format!("failed to write report '{}'", path.display()))?;
        println!("Per-sample CSV: {}", path.display());
    }

    Ok(())
}

fn run_csv(request: CsvRequest) -> Result<()> {
    let series = read_path(&request.input, &request.read_options)
        .with_context(|| format!("failed to load '{}'", request.input.display()))?;
    let outputs = apply_all(&series.values, request.filters)?;
    let width = request.width.unwrap_or_else(available_width);

    println!(
        "{}",
        render_frame(&trace_views(&outputs), width, request.layout)
    );
    println!();

    if let Some(reference) = &series.reference {
        println!(
            "Reference: CSV column '{}'",
            request.read_options.reference_column
        );
        print!(
            "{}",
            metric_report(reference, &labeled_values(&outputs), None)?
        );
    } else {
        println!("Reference: none (metrics unavailable without a reference column)");
    }

    Ok(())
}

fn trace_views(outputs: &FilterOutputs) -> [TraceView<'_>; 4] {
    [
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
    ]
}

fn labeled_values(outputs: &FilterOutputs) -> [(&'static str, &[f64]); 4] {
    [
        ("Raw", &outputs.raw),
        ("EWMA", &outputs.ewma),
        ("Median", &outputs.median),
        ("Kalman", &outputs.kalman),
    ]
}

fn metric_report(
    reference: &[f64],
    values: &[(&str, &[f64])],
    spikes: Option<&[SpikeMetrics]>,
) -> Result<String> {
    let input_snr = snr::compute(reference, values[0].1)?;
    let errors = values
        .iter()
        .map(|(_, values)| error::compute(reference, values))
        .collect::<Result<Vec<_>, _>>()?;
    let snrs = values
        .iter()
        .map(|(_, values)| snr::compute(reference, values))
        .collect::<Result<Vec<_>, _>>()?;
    let report_rows = values
        .iter()
        .enumerate()
        .map(|(index, (label, _))| TraceMetrics {
            label,
            error: errors[index],
            snr: snrs[index],
            improvement: snr::improvement(input_snr, snrs[index]),
            spike: spikes.map(|spikes| &spikes[index]),
        })
        .collect::<Vec<_>>();

    Ok(format_metric_report(&report_rows))
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
