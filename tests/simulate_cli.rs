use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_signal-filter-shootout"))
        .args(arguments)
        .env("COLUMNS", "80")
        .output()
        .expect("run signal-filter-shootout")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn temp_report() -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "signal-filter-shootout-{}-{counter}.csv",
        std::process::id()
    ))
}

#[test]
fn simulate_prints_all_traces_and_metrics() {
    let output = run(&["simulate", "--samples", "32", "--seed", "42"]);

    assert!(output.status.success());
    let stdout = stdout(&output);
    for label in ["Raw", "EWMA", "Median", "Kalman"] {
        assert!(stdout.contains(label), "missing label {label}");
    }
    assert!(stdout.contains("RMSE"));
    assert!(stdout.contains("Spike metrics:"));
}

#[test]
fn repeated_seeded_runs_are_identical() {
    let arguments = ["simulate", "--samples", "64", "--seed", "9001"];
    let first = run(&arguments);
    let second = run(&arguments);

    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn report_csv_has_expected_header_and_row_count() {
    let path = temp_report();
    let path_text = path.to_string_lossy().into_owned();
    let output = run(&[
        "simulate",
        "--samples",
        "10",
        "--seed",
        "7",
        "--report-csv",
        &path_text,
    ]);

    assert!(output.status.success());
    let report = std::fs::read_to_string(&path).expect("read report");
    let mut lines = report.lines();
    assert_eq!(
        lines.next(),
        Some("sample,time,truth,raw,ewma,median,kalman,is_spike")
    );
    assert_eq!(lines.count(), 10);

    std::fs::remove_file(path).expect("remove report");
}

#[test]
fn invalid_median_window_exits_nonzero_with_actionable_error() {
    let output = run(&["simulate", "--median-window", "4"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("window"));
    assert!(stderr.contains("odd and at least 1"));
}

#[test]
fn disabled_noise_has_zero_raw_rmse() {
    let output = run(&[
        "simulate",
        "--samples",
        "32",
        "--gaussian-sigma",
        "0",
        "--spike-probability",
        "0",
    ]);

    assert!(output.status.success());
    let stdout = stdout(&output);
    let raw_row = stdout
        .lines()
        .find(|line| line.starts_with("Raw ") && !line.contains('│'))
        .expect("raw metric row");
    assert!(raw_row.contains("0.000000"));
}
