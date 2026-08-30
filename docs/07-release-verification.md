# 07 — Release Verification

The v0.1 implementation was verified on 2026-08-30 with the commands and checks below. Re-run them before a release or after behavior changes.

## Build quality
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --all-targets --all-features`
- [x] `cargo build --release`
- [x] no `unsafe`
- [x] no runtime network access

## Core filter shootout
- [x] EWMA, median, Kalman use the same raw input samples.
- [x] All filter outputs preserve sample count.
- [x] Synthetic generation is deterministic with a seed.
- [x] Gaussian and impulse noise can be independently disabled.
- [x] Raw + EWMA + median + Kalman appear in one terminal frame.
- [x] A shared y-range makes traces visually comparable.
- [x] The metric table contains RMSE/MAE/max error/SNR.
- [x] Spike metrics appear when a spike mask exists.

Smoke command:

```bash
cargo run --release -- simulate \
  --samples 1000 \
  --gaussian-sigma 0.20 \
  --spike-probability 0.015 \
  --spike-amplitude 2.5 \
  --seed 42
```

## CSV
- [x] `samples/sensor/with_reference.csv` loads.
- [x] `samples/sensor/no_reference.csv` loads.
- [x] An explicit reference is used for true RMSE comparison.
- [x] A missing reference uses a pseudo-reference with a clear warning.
- [x] Grid search prints the best Q/R and top candidates.
- [x] A tuning CSV can be emitted.

Smoke commands:

```bash
cargo run --release -- csv samples/sensor/with_reference.csv --auto-tune-kalman

cargo run --release -- csv samples/sensor/no_reference.csv \
  --auto-tune-kalman \
  --tuning-csv out/tuning.csv
```

## Audio
- [x] Clean bundled WAVs decode.
- [x] Mixed-noise bundled WAVs decode.
- [x] `audio inject-noise` is deterministic.
- [x] Mono/stereo logic uses independent filter state per channel.
- [x] Compare writes three filtered WAVs.
- [x] Reference metrics require exact compatible shape/metadata.
- [x] The waveform view uses a short time window.

Smoke commands:

```bash
cargo run --release -- audio inject-noise \
  samples/audio/clean/01_general.wav \
  out/regenerated.wav \
  --gaussian-sigma 0.035 \
  --spike-probability 0.0008 \
  --spike-amplitude 0.85 \
  --seed 4101

cargo run --release -- audio compare \
  samples/audio/noisy/01_general_mixed.wav \
  --reference samples/audio/clean/01_general.wav \
  --output-dir out/01-general
```

## Documentation correctness
- [x] README CLI examples match implemented flags.
- [x] Metric definitions match code.
- [x] The pseudo-reference is never called ground truth.
- [x] The audio section explicitly states RMSE/SNR are not perceptual metrics.
- [ ] bundled WAV manifest exactly matches generated assets.
