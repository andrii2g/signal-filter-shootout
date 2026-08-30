# 01 — Requirements

## Product statement

`signal-filter-shootout` is a local Rust CLI for learning and practical comparison of scalar online filters. It is deterministic, small, inspectable, and works with generated IoT-like data, real scalar CSV data, and WAV samples that let users hear model strengths and weaknesses.

## Implemented functionality

### Synthetic sensor mode
- Generate `N` samples of a configurable sine-wave truth signal.
- Add zero-mean Gaussian noise with configurable standard deviation.
- Add independent random impulse spikes with configurable probability and amplitude range.
- Seed all randomness from one CLI seed.
- Run raw measurements through EWMA, sliding median, and scalar Kalman filters.
- Preserve one output sample per input sample.
- Compute metrics against known truth.
- Render raw + 3 filtered traces in one terminal frame.
- Print a compact metric table and category winners.

### CSV mode
- Read a headered CSV.
- Default value column: `value`.
- Optional time column: `timestamp`.
- Optional reference column: `reference`.
- Allow explicit column overrides.
- Ignore neither malformed numeric values nor non-finite values silently: return line/column-aware errors.
- Run all filters using CLI/default parameters.
- If `--auto-tune-kalman` is present, grid-search `Q` and `R`.
- If reference exists, optimize RMSE against it.
- Otherwise build the specified offline pseudo-reference and clearly label metrics as reference-relative rather than ground-truth accuracy.

### Audio mode
- Read uncompressed PCM WAV via `hound`.
- Supported formats: signed 16-bit integer PCM; mono or stereo.
- Internally normalize each sample to `f64` in `[-1, 1]`.
- Process channels independently with identical filter parameters but separate filter state.
- Preserve sample rate and channel count.
- Write signed 16-bit PCM WAV outputs.
- Support deterministic Gaussian and impulse-noise injection.
- Support compare mode writing EWMA, median, and Kalman outputs in one output directory.
- If a clean reference WAV is supplied, require matching sample rate/channel count/sample count and compute waveform RMSE and SNR metrics.

## Metrics

Synthetic/CSV with reference:
- RMSE;
- MAE;
- maximum absolute error;
- input SNR and output SNR;
- SNR improvement;
- spike-region RMSE where spike mask is known;
- recovery length after known injected spikes.

Audio with clean reference:
- waveform RMSE;
- input/output SNR;
- SNR improvement;
- peak absolute error.

Do not claim that waveform RMSE or SNR is a perceptual speech-quality metric.

## Determinism

Given the same command line, input file bytes, seed, and executable version:
- synthetic samples must match;
- noise injection must match;
- grid-search candidate order and selected winner must match;
- ties must be resolved deterministically.

Tie-break Kalman tuning by:
1. lower RMSE;
2. lower `Q`;
3. lower `R`.

## Defaults

The CLI uses these defaults:

| Parameter | Default |
|---|---:|
| synthetic samples | 1000 |
| sine amplitude | 1.0 |
| sine cycles/sample | 0.02 |
| Gaussian sigma | 0.20 |
| spike probability | 0.015 |
| spike amplitude | 2.5 |
| seed | 42 |
| EWMA alpha | 0.20 |
| median window | 5 |
| Kalman Q | 0.001 |
| Kalman R | 0.04 |
| Kalman initial covariance | 1.0 |
| terminal width | auto, minimum 40 |

## Non-goals for v0.1
- Multidimensional/vector/matrix Kalman filtering.
- Kalman motion models with velocity/acceleration state.
- FFT/STFT, spectral subtraction, Wiener filtering, LMS/RLS adaptive filters.
- ML speech enhancement.
- MP3/AAC/FLAC decoding.
- Real-time microphone capture.
- GUI/TUI interaction.
- Network/cloud services.
- Automatic perceptual audio-quality scoring such as PESQ/STOI.
- Production-grade statistical inference of process models.

## Possible future work (not part of v0.1)
- Median -> Kalman hybrid filter.
- Auto-tune EWMA alpha and median window.
- Float WAV input.
- CSV/JSON machine-readable reports.
- Criterion benchmarks.
- Synthetic vowel/formant generator.
- Sweep noise level and plot metric tables as CSV artifacts.
