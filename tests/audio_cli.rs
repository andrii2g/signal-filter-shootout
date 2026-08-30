use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use signal_filter_shootout::audio::wav::read_path;

static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_signal-filter-shootout"))
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/audio/clean/01_general.wav")
}

fn temporary_wav(label: &str) -> PathBuf {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "signal-filter-shootout-{}-{label}-{sequence}.wav",
        std::process::id()
    ))
}

fn run_injection(output: &Path, extra: &[&str]) -> Output {
    let mut command = binary();
    command
        .arg("audio")
        .arg("inject-noise")
        .arg(fixture())
        .arg(output);
    command.args(extra).output().expect("run audio CLI")
}

#[test]
fn injection_preserves_wav_shape_and_metadata() {
    let output = temporary_wav("metadata");
    let result = run_injection(&output, &["--seed", "4101"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let input = read_path(fixture()).unwrap();
    let generated = read_path(&output).unwrap();
    assert_eq!(generated.sample_rate, input.sample_rate);
    assert_eq!(generated.channels, input.channels);
    assert_eq!(generated.interleaved.len(), input.interleaved.len());

    fs::remove_file(output).unwrap();
}

#[test]
fn injection_is_byte_deterministic_for_a_fixed_seed() {
    let first = temporary_wav("deterministic-a");
    let second = temporary_wav("deterministic-b");
    let arguments = [
        "--gaussian-sigma",
        "0.035",
        "--spike-probability",
        "0.0008",
        "--spike-amplitude",
        "0.85",
        "--seed",
        "4101",
    ];

    assert!(run_injection(&first, &arguments).status.success());
    assert!(run_injection(&second, &arguments).status.success());
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());

    fs::remove_file(first).unwrap();
    fs::remove_file(second).unwrap();
}

#[test]
fn zero_noise_preserves_pcm_samples_exactly() {
    let output = temporary_wav("zero-noise");
    let result = run_injection(
        &output,
        &[
            "--gaussian-sigma",
            "0",
            "--spike-probability",
            "0",
            "--spike-amplitude",
            "0",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        read_path(fixture()).unwrap().interleaved,
        read_path(&output).unwrap().interleaved
    );

    fs::remove_file(output).unwrap();
}

#[test]
fn invalid_noise_configuration_fails_with_an_actionable_message() {
    let output = temporary_wav("invalid");
    let result = run_injection(&output, &["--gaussian-sigma", "-0.1"]);

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("configuration parameter"));
    assert!(stderr.contains("gaussian_sigma") && stderr.contains("must be at least 0"));
    assert!(!output.exists());
}
