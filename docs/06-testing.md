# 06 — Testing Strategy

## Unit tests by module

### `filters/ewma.rs`
- rejects alpha <= 0, > 1, NaN, infinity;
- alpha=1 returns input exactly;
- known recurrence sequence;
- reset makes next sample a new initial condition;
- constant input remains constant.

### `filters/median.rs`
- rejects zero/even window;
- window 1 is identity;
- startup prefix behavior is deterministic;
- one large impulse in a stable neighborhood is rejected;
- reset clears history;
- negative values and duplicates.

### `filters/kalman.rs`
- validates Q/R/P0;
- constant observations converge/stay near constant;
- increasing R reduces response to measurement relative to lower R;
- increasing Q increases responsiveness relative to lower Q;
- covariance stays non-negative and finite;
- reset restores initial state.

### `signal/synthetic.rs`
- same seed produces byte-for-byte equal vectors;
- different seeds normally differ;
- sigma=0 and spike probability=0 yields exact truth;
- probability=1 marks all samples as spikes;
- invalid parameters fail.

### `signal/csv.rs`
- happy path with value/timestamp/reference;
- happy path value only;
- missing value column;
- malformed number includes row information;
- NaN/infinite value rejected;
- unequal optional columns cannot happen after row-wise parse.

### `tuning/reference.rs`
- constant input remains constant;
- isolated extreme spike is replaced by Hampel stage;
- forward/backward smoother preserves sample count;
- output is deterministic.

### `tuning/grid_search.rs`
- deterministic candidate generation;
- deduplicated, sorted candidates;
- known synthetic data selects a better candidate than a deliberately bad baseline;
- tie-break order is stable;
- top N sorting.

### `metrics/error.rs`
- identical vectors -> zero RMSE/MAE/max error;
- hand-computed values;
- mismatched lengths rejected;
- empty vectors rejected.

### `metrics/snr.rs`
- known power ratio;
- perfect estimate -> infinite SNR representation;
- zero-signal policy returns n/a;
- improvement calculation.

### `render/sparkline.rs`
- constant series uses stable middle glyph policy;
- min/max mapping uses full glyph range;
- shared-range rendering produces comparable amplitudes;
- downsampling length exact;
- impulse-preserving bucket selection.

### `audio/wav.rs`
- write/read PCM16 fixture preserves metadata and sample count;
- normalized conversion endpoints;
- mono and stereo deinterleaving/reinterleaving;
- unsupported format rejected.

### `audio/noise.rs`
- deterministic seeded injection;
- zero noise returns exact samples;
- spike mask count and channel handling;
- values may exceed normalized range internally but writer clamps.

### `audio/process.rs`
- one filter state per channel;
- output sample count preserved;
- constant audio through median remains constant;
- alpha=1 EWMA preserves PCM within quantization tolerance.

## Integration tests

### `tests/simulate_cli.rs`
- `simulate --seed 42` exits 0;
- output contains all four labels;
- report CSV has expected header and row count;
- invalid median window exits non-zero.

### `tests/csv_cli.rs`
- bundled `with_reference.csv` runs;
- auto-tune prints explicit reference source;
- no-reference fixture prints pseudo-reference warning;
- tuning CSV has rank 1 and candidate rows.

### `tests/audio_cli.rs`
- compare bundled noisy speech -> creates three WAVs;
- outputs match sample rate/channel count/sample count;
- reference metrics CSV contains raw + 3 filters;
- mismatch reference fails cleanly.

## Deterministic golden data

Avoid snapshotting huge terminal frames unless needed. Prefer invariant checks plus a few small exact fixture sequences.

The bundled WAV files are input fixtures, not expected filtered-output goldens; algorithm changes should not require binary fixture churn unless the input sample generation changes.

## Performance sanity

No strict benchmark gate in v0.1, but a release build should process a few seconds of 16 kHz mono audio effectively instantaneously on a normal development workstation. If an implementation allocates per sample unnecessarily, simplify it before adding optimization complexity.
