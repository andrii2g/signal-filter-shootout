# signal-filter-shootout

A small, deterministic Rust CLI that compares scalar Kalman, EWMA, and sliding-window median filters on synthetic sensor data, CSV measurements, and PCM WAV speech.

The same online filters are deliberately reused across very different signals. The point is not to crown a universal winner: it is to make model fit, lag, impulse rejection, tuning, and speech distortion visible and audible.

## Quick start

Requires a Rust toolchain with edition 2024 support.

```bash
cargo run --release -- simulate --seed 42
cargo run --release -- csv samples/sensor/with_reference.csv --auto-tune-kalman
cargo run --release -- audio compare \
  samples/audio/noisy/01_general_mixed.wav \
  --reference samples/audio/clean/01_general.wav \
  --output-dir out/01-general
```

Run `cargo run -- --help` or a subcommand's `--help` for every option.

## Synthetic shootout

`simulate` creates a known sine-wave truth, adds deterministic Gaussian noise and sparse impulses, applies all filters to the same measurements, and prints shared-scale Unicode sparklines and metrics.

```bash
cargo run --release -- simulate \
  --samples 1000 \
  --gaussian-sigma 0.20 \
  --spike-probability 0.015 \
  --spike-amplitude 2.5 \
  --seed 42
```

Representative output captured from v0.1:

```text
Trace    RMSE       MAE        MaxAbs     SNR(dB)    ΔSNR(dB)
Raw        0.302974   0.183714   2.698456      7.362       0.000
EWMA       0.315734   0.278355   0.817008      7.003      -0.358
Median     0.203876   0.171202   0.587641     10.802       3.441
Kalman     0.405694   0.361628   0.900997      4.826      -2.536
Winners: RMSE=Median  MAE=Median  MaxAbs=Median
```

The output also reports spike RMSE and recovery length. Use `--show-truth` to include the truth trace and `--report-csv <PATH>` for per-sample data. Setting both noise sources to zero produces exact raw RMSE 0.

## CSV data and Kalman tuning

CSV input must have a header and defaults to the `value`, `timestamp`, and `reference` column names. Time and reference are optional; malformed or non-finite numbers fail with row and column context.

```bash
cargo run --release -- csv samples/sensor/with_reference.csv --auto-tune-kalman
```

Captured tuning result:

```text
Reference: CSV column 'reference'
Best Kalman: Q=1.000000e-1 R=3.000000e-1 RMSE=0.226759
Winners: RMSE=Kalman  MAE=Kalman  MaxAbs=Median
```

Without a reference column, tuning uses an offline Hampel-style despiker followed by forward/backward EWMA smoothing:

```bash
cargo run --release -- csv samples/sensor/no_reference.csv \
  --auto-tune-kalman \
  --tuning-csv out/tuning.csv
```

That output is explicitly labeled `Reference: offline pseudo-reference (not ground truth)`. It supports relative parameter selection only and must not be interpreted as ground-truth accuracy.

## WAV speech experiment

Audio support is intentionally limited to signed PCM16 WAV, mono or stereo. Samples are normalized to `f64`, channels receive independent filter state, and results preserve sample rate, channel count, and sample count. Final samples are clamped only during PCM conversion.

Inject reproducible Gaussian and impulse noise:

```bash
cargo run --release -- audio inject-noise \
  samples/audio/clean/01_general.wav \
  out/regenerated.wav \
  --gaussian-sigma 0.035 \
  --spike-probability 0.0008 \
  --spike-amplitude 0.85 \
  --seed 4101
```

Apply one filter:

```bash
cargo run --release -- audio filter \
  samples/audio/noisy/01_general_mixed.wav \
  out/median.wav \
  --kind median \
  --median-window 5
```

Compare all filters:

```bash
cargo run --release -- audio compare \
  samples/audio/noisy/01_general_mixed.wav \
  --reference samples/audio/clean/01_general.wav \
  --output-dir out/01-general
```

This writes `ewma.wav`, `median.wav`, `kalman.wav`, and—when a compatible clean reference is supplied—`metrics.csv`. The terminal frame shows only the selected short window; configure it with `--window-start-ms`, `--window-duration-ms`, and `--width`.

Captured waveform metrics for Sample 01:

```text
Trace    RMSE       MAE        MaxAbs     SNR(dB)    ΔSNR(dB)
Raw        0.038974   0.028431   0.869629     11.392       0.000
EWMA       0.091444   0.047670   0.860218      3.985      -7.408
Median     0.088694   0.045458   1.166321      4.250      -7.143
Kalman     0.105458   0.054347   0.921549      2.746      -8.646
```

Waveform RMSE and SNR are not perceptual speech-quality metrics. In this example the noisy raw signal scores best by RMSE even though sparse clicks or hiss may still be objectionable. Listen in this order:

1. clean reference;
2. noisy input;
3. median output;
4. EWMA output;
5. Kalman output.

Listen for click removal, muffling, lag, and damage to plosives or sibilants. The scalar random-walk Kalman model does not model pitch, formants, harmonics, or speech transients.

## Metrics and numerical policy

For reference `r[i]` and estimate `y[i]`:

```text
error[i] = y[i] - r[i]
RMSE     = sqrt(mean(error²))
MAE      = mean(abs(error))
MaxAbs   = max(abs(error))
SNR(dB)  = 10 * log10(mean(reference²) / mean(error²))
ΔSNR     = output SNR - raw input SNR
```

Perfect estimates report `inf` SNR. Zero-power references report `n/a`. Configuration and measurements reject NaN and infinity; parameters are validated rather than silently clamped.

## Determinism and fixtures

A fixed command line, input bytes, seed, and executable version produce the same synthetic data, injected noise, and tuning order. One seeded RNG stream processes audio in interleaved order, so changing channel count changes RNG consumption.

Bundled files under `samples/audio/` are 16 kHz mono PCM16 synthetic speech generated from project-authored text. `samples/audio/manifest.csv` records transcripts, clean/noisy paths, TTS settings, noise parameters, and seeds. eSpeak, FFmpeg, and Python are developer-only fixture-generation tools; the Rust application has no runtime dependency on them.

See `samples/audio/README.md` for regeneration instructions.

## Project structure

- `src/filters/` — online EWMA, median, and scalar Kalman algorithms.
- `src/signal/` — synthetic data and CSV ingestion.
- `src/tuning/` — pseudo-reference construction and deterministic Kalman grid search.
- `src/metrics/` — error, SNR, and spike statistics.
- `src/render/` — Unicode sparklines and reports.
- `src/audio/` — PCM WAV I/O, noise injection, and per-channel experiments.
- `docs/` — requirements, architecture, algorithms, CLI contract, testing, and release verification.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

The runtime is local and synchronous: no unsafe code, async runtime, network service, GUI, codec bindings, or cloud dependency.
