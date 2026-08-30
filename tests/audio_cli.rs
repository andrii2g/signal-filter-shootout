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

fn noisy_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples/audio/noisy")
        .join(name)
}

fn clean_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("samples/audio/clean")
        .join(name)
}

fn temporary_directory(label: &str) -> PathBuf {
    let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "signal-filter-shootout-{}-{label}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn compare_processes_all_bundled_pairs_and_writes_outputs_and_metrics() {
    for (clean, noisy) in [
        ("01_general.wav", "01_general_mixed.wav"),
        ("02_iot.wav", "02_iot_mixed.wav"),
        ("03_filter_terms.wav", "03_filter_terms_mixed.wav"),
        ("04_transients.wav", "04_transients_mixed.wav"),
    ] {
        let output_dir = temporary_directory(noisy);
        let result = binary()
            .arg("audio")
            .arg("compare")
            .arg(noisy_fixture(noisy))
            .arg("--reference")
            .arg(clean_fixture(clean))
            .arg("--output-dir")
            .arg(&output_dir)
            .arg("--width")
            .arg("80")
            .output()
            .expect("run compare CLI");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );

        let input = read_path(noisy_fixture(noisy)).unwrap();
        for output_name in ["ewma.wav", "median.wav", "kalman.wav"] {
            let output = read_path(output_dir.join(output_name)).unwrap();
            assert_eq!(output.sample_rate, input.sample_rate);
            assert_eq!(output.channels, input.channels);
            assert_eq!(output.interleaved.len(), input.interleaved.len());
        }

        let metrics = fs::read_to_string(output_dir.join("metrics.csv")).unwrap();
        assert_eq!(
            metrics.lines().next().unwrap(),
            "trace,rmse,snr_db,snr_improvement_db,max_abs_error"
        );
        for label in ["raw,", "ewma,", "median,", "kalman,"] {
            assert!(metrics.lines().any(|line| line.starts_with(label)));
        }
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("not perceptual quality measures")
        );

        fs::remove_dir_all(output_dir).unwrap();
    }
}

#[test]
fn filter_alpha_one_preserves_pcm_samples() {
    let output = temporary_wav("filter-ewma");
    let result = binary()
        .arg("audio")
        .arg("filter")
        .arg(fixture())
        .arg(&output)
        .arg("--kind")
        .arg("ewma")
        .arg("--ewma-alpha")
        .arg("1")
        .output()
        .expect("run filter CLI");
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
fn mismatched_reference_fails_before_outputs_are_created() {
    let output_dir = temporary_directory("mismatch");
    let result = binary()
        .arg("audio")
        .arg("compare")
        .arg(noisy_fixture("01_general_mixed.wav"))
        .arg("--reference")
        .arg(clean_fixture("02_iot.wav"))
        .arg("--output-dir")
        .arg(&output_dir)
        .output()
        .expect("run compare CLI");

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("reference sample count mismatch"));
    assert!(!output_dir.exists());
}
