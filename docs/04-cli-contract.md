# 04 — CLI Contract

Binary name: `signal-filter-shootout`.

## Global behavior

```text
signal-filter-shootout <COMMAND> [OPTIONS]
```

Commands:
- `simulate`
- `csv`
- `audio`

All failures return non-zero exit code and a concise diagnostic on stderr.

## `simulate`

```bash
signal-filter-shootout simulate [OPTIONS]
```

Options:

```text
--samples <N>                 default 1000
--amplitude <F64>             default 1.0
--frequency <F64>             cycles/sample, default 0.02
--phase <F64>                 radians, default 0
--gaussian-sigma <F64>        default 0.20
--spike-probability <F64>     default 0.015
--spike-amplitude <F64>       default 2.5
--seed <U64>                  default 42

--ewma-alpha <F64>            default 0.20
--median-window <ODD_USIZE>   default 5
--kalman-q <F64>              default 0.001
--kalman-r <F64>              default 0.04
--kalman-p0 <F64>             default 1.0

--width <USIZE>               optional explicit sparkline width
--layout <auto|columns|rows>  default auto
--show-truth                   include truth trace if layout permits
--report-csv <PATH>           optional per-sample output CSV
```

Per-sample report columns:

```csv
sample,time,truth,raw,ewma,median,kalman,is_spike
```

## `csv`

```bash
signal-filter-shootout csv <INPUT.csv> [OPTIONS]
```

Input options:

```text
--value-column <NAME>         default value
--time-column <NAME>          default timestamp; optional if missing
--reference-column <NAME>     default reference; optional if missing
```

Filter options are identical to `simulate`.

Tuning:

```text
--auto-tune-kalman
--q-min-exp <-INTEGER>        default -8
--q-max-exp <-INTEGER>        default -1
--r-min-exp <-INTEGER>        default -6
--r-max-exp <INTEGER>         default 1
--grid-multipliers <LIST>     default 1,3
--top <N>                     default 5
--tuning-csv <PATH>           optional all-candidate report
```

Tuning CSV:

```csv
rank,q,r,rmse
```

Normal output must state one of:
- `Reference: CSV column '<name>'`
- `Reference: offline pseudo-reference (not ground truth)`

## `audio`

Subcommands:

```text
audio inject-noise
audio filter
audio compare
```

### `audio inject-noise`

```bash
signal-filter-shootout audio inject-noise <INPUT.wav> <OUTPUT.wav> [OPTIONS]
```

Options:

```text
--gaussian-sigma <F64>        default 0.03
--spike-probability <F64>     default 0.0008
--spike-amplitude <F64>       default 0.85
--seed <U64>                  default 42
```

### `audio filter`

```bash
signal-filter-shootout audio filter <INPUT.wav> <OUTPUT.wav> \
  --kind <ewma|median|kalman> [FILTER OPTIONS]
```

Relevant options:

```text
--ewma-alpha <F64>
--median-window <ODD_USIZE>
--kalman-q <F64>
--kalman-r <F64>
--kalman-p0 <F64>
```

### `audio compare`

```bash
signal-filter-shootout audio compare <INPUT.wav> [OPTIONS]
```

Options:

```text
--reference <CLEAN.wav>       optional clean reference
--output-dir <DIR>            required or default out/audio
--ewma-alpha <F64>            default 0.20
--median-window <ODD_USIZE>   default 5
--kalman-q <F64>              default 0.001
--kalman-r <F64>              default 0.04
--kalman-p0 <F64>             default 1.0
--window-start-ms <F64>       default 0
--window-duration-ms <F64>    default 30
--width <USIZE>               waveform sparkline width
```

Outputs:

```text
<output-dir>/
  ewma.wav
  median.wav
  kalman.wav
  metrics.csv             # only if reference supplied
```

Metrics CSV:

```csv
trace,rmse,snr_db,snr_improvement_db,max_abs_error
raw,...
ewma,...
median,...
kalman,...
```

## Terminal frame policy

`--layout auto`:
- if terminal width can fit 4 panels at >= 20 sparkline characters each, render columns;
- otherwise render one labeled sparkline per row.

All four filter-comparison traces should share a common y-range for a given frame so visual amplitudes are comparable.

If the series is too long, downsample deterministically. Recommended bucket policy: choose the sample with the largest absolute deviation from the bucket mean, which tends to preserve impulses better than plain averaging.

For audio compare, render only the selected short time window, never the entire multi-second waveform as a sparkline.
