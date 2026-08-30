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
    audio::{
        noise::inject_noise,
        wav::{AudioBuffer, read_path as read_wav, write_path as write_wav},
    },
    filters::{FilterOutputs, apply_all, kalman::KalmanConfig},
    metrics::{error, snr, spike::SpikeMetrics},
    render::{
        frame::{TraceView, available_width, render_frame},
        report::{TraceMetrics, format_metric_report, format_tuning_report},
    },
    signal::{
        csv::read_path,
        synthetic::{SyntheticSeries, generate},
    },
    tuning::{
        grid_search::{GridSearchResult, search},
        reference::{ReferenceConfig, build_pseudo_reference},
    },
};

use crate::cli::{
    AudioCommand, AudioInjectNoiseRequest, Cli, Command, CsvRequest, SimulateRequest,
};

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
        Command::Audio(arguments) => match arguments.command {
            AudioCommand::InjectNoise(arguments) => run_audio_inject_noise(arguments.validate()?),
        },
    }
}

fn run_audio_inject_noise(request: AudioInjectNoiseRequest) -> Result<()> {
    let audio = read_wav(&request.input)
        .with_context(|| format!("failed to read WAV '{}'", request.input.display()))?;
    let result = inject_noise(&audio, request.noise)?;
    let output = AudioBuffer::new(audio.sample_rate, audio.channels, result.samples)?;

    if let Some(parent) = request
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    write_wav(&request.output, &output)
        .with_context(|| format!("failed to write WAV '{}'", request.output.display()))?;
    println!(
        "Wrote {}: {} Hz, {} channel(s), {} frames",
        request.output.display(),
        output.sample_rate,
        output.channels,
        output.frame_count()
    );

    Ok(())
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
    let (reference, reference_label) = if let Some(reference) = &series.reference {
        (
            reference.clone(),
            format!(
                "Reference: CSV column '{}'",
                request.read_options.reference_column
            ),
        )
    } else {
        (
            build_pseudo_reference(&series.values, ReferenceConfig::default())?,
            "Reference: offline pseudo-reference (not ground truth)".to_owned(),
        )
    };

    let mut filters = request.filters;
    let mut tuning_result = None;
    let mut tuning_top = 0;
    if let Some(tuning) = request.tuning {
        let result = search(
            &series.values,
            &reference,
            &tuning.grid,
            filters.kalman.p0(),
        )?;
        filters.kalman = KalmanConfig::new(result.best.q, result.best.r, filters.kalman.p0())?;
        tuning_top = tuning.grid.top();

        if let Some(path) = tuning.output_csv {
            write_tuning_csv(&path, &result)
                .with_context(|| format!("failed to write tuning CSV '{}'", path.display()))?;
            println!("Tuning CSV: {}", path.display());
        }
        tuning_result = Some(result);
    }

    let outputs = apply_all(&series.values, filters)?;
    let width = request.width.unwrap_or_else(available_width);
    println!(
        "{}",
        render_frame(&trace_views(&outputs), width, request.layout)
    );
    println!();
    println!("{reference_label}");

    if let Some(result) = &tuning_result {
        print!(
            "{}",
            format_tuning_report(result.best, result.top(tuning_top))
        );
        println!();
    }
    print!(
        "{}",
        metric_report(&reference, &labeled_values(&outputs), None)?
    );

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
fn write_tuning_csv(path: &Path, result: &GridSearchResult) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut writer = BufWriter::new(File::create(path)?);
    writeln!(writer, "rank,q,r,rmse")?;
    for (index, candidate) in result.ranked.iter().enumerate() {
        writeln!(
            writer,
            "{},{:.17e},{:.17e},{:.17e}",
            index + 1,
            candidate.q,
            candidate.r,
            candidate.rmse
        )?;
    }
    writer.flush()?;
    Ok(())
}
