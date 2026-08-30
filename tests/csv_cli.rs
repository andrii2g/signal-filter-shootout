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

fn temp_csv(contents: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "signal-filter-shootout-csv-{}-{counter}.csv",
        std::process::id()
    ));
    std::fs::write(&path, contents).expect("write temporary CSV");
    path
}

#[test]
fn bundled_reference_csv_runs_with_metrics() {
    let output = run(&["csv", "samples/sensor/with_reference.csv"]);

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("Reference: CSV column 'reference'"));
    assert!(stdout.contains("RMSE"));
    for label in ["Raw", "EWMA", "Median", "Kalman"] {
        assert!(stdout.contains(label));
    }
}

#[test]
fn bundled_csv_without_reference_runs_without_accuracy_claims() {
    let output = run(&["csv", "samples/sensor/no_reference.csv"]);

    assert!(output.status.success());
    let stdout = stdout(&output);
    assert!(stdout.contains("Reference: none"));
    assert!(stdout.contains("metrics unavailable"));
    assert!(!stdout.contains("RMSE"));
}

#[test]
fn explicit_column_overrides_are_used() {
    let path = temp_csv("when,reading,clean\n0,1.0,0.9\n1,2.0,2.1\n");
    let path_text = path.to_string_lossy().into_owned();
    let output = run(&[
        "csv",
        &path_text,
        "--value-column",
        "reading",
        "--time-column",
        "when",
        "--reference-column",
        "clean",
    ]);

    assert!(output.status.success());
    assert!(stdout(&output).contains("Reference: CSV column 'clean'"));
    std::fs::remove_file(path).expect("remove temporary CSV");
}

#[test]
fn malformed_number_exits_nonzero_with_row_and_column() {
    let path = temp_csv("value\n1.0\nbad\n");
    let path_text = path.to_string_lossy().into_owned();
    let output = run(&["csv", &path_text]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("line 3"));
    assert!(stderr.contains("column 'value'"));
    std::fs::remove_file(path).expect("remove temporary CSV");
}

#[test]
fn missing_value_column_exits_nonzero() {
    let path = temp_csv("other\n1.0\n");
    let path_text = path.to_string_lossy().into_owned();
    let output = run(&["csv", &path_text]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(stderr.contains("missing required value column 'value'"));
    std::fs::remove_file(path).expect("remove temporary CSV");
}
