# 02 — Architecture

## Top-level flow

```mermaid
flowchart LR
    CLI["CLI / clap"] --> APP["Command orchestration"]

    APP --> SYN["Synthetic signal"]
    APP --> CSV["CSV reader"]
    APP --> WAV["WAV reader"]

    SYN --> SERIES["SignalSeries"]
    CSV --> SERIES
    WAV --> AUDIO["AudioBuffer"]

    SERIES --> FILTERS["EWMA / Median / Kalman"]
    AUDIO --> FILTERS

    FILTERS --> METRICS["Metrics"]
    FILTERS --> RENDER["Terminal renderer"]
    FILTERS --> WAVOUT["WAV writer"]

    CSV --> TUNE["Kalman grid search"]
    TUNE --> FILTERS
```

## Core dependency direction

```mermaid
flowchart TB
    MAIN["main.rs"] --> CLI["cli.rs"]
    MAIN --> SIGNAL["signal/*"]
    MAIN --> FILTERS["filters/*"]
    MAIN --> TUNING["tuning/*"]
    MAIN --> METRICS["metrics/*"]
    MAIN --> RENDER["render/*"]
    MAIN --> AUDIO["audio/*"]

    TUNING --> FILTERS
    TUNING --> METRICS
    RENDER --> METRICS
    AUDIO --> FILTERS
    AUDIO --> METRICS

    FILTERS -. "must not depend on" .-> CLI
    FILTERS -. "must not depend on" .-> AUDIO
    FILTERS -. "must not depend on" .-> SIGNAL
```

The dotted arrows represent forbidden reverse coupling.

## Core data model

Suggested structs:

```rust
pub struct SignalSeries {
    pub time: Option<Vec<f64>>,
    pub values: Vec<f64>,
    pub reference: Option<Vec<f64>>,
}

pub struct SyntheticSeries {
    pub time: Vec<f64>,
    pub truth: Vec<f64>,
    pub noisy: Vec<f64>,
    pub spike_mask: Vec<bool>,
}

pub struct FilterOutputs {
    pub raw: Vec<f64>,
    pub ewma: Vec<f64>,
    pub median: Vec<f64>,
    pub kalman: Vec<f64>,
}
```

Keep these domain types simple. Avoid a generic framework unless a later phase proves it is needed.

## Synthetic pipeline

```mermaid
flowchart LR
    PARAMS["SignalConfig + NoiseConfig + seed"] --> TRUTH["Sine ground truth"]
    TRUTH --> GAUSS["Add Gaussian noise"]
    GAUSS --> SPIKES["Inject impulses"]
    SPIKES --> RAW["Raw measurements"]

    RAW --> E["EWMA"]
    RAW --> M["Median"]
    RAW --> K["Kalman"]

    TRUTH --> MET["Metrics"]
    E --> MET
    M --> MET
    K --> MET

    RAW --> TERM["Terminal frame"]
    E --> TERM
    M --> TERM
    K --> TERM
```

## CSV tuning pipeline

```mermaid
flowchart LR
    FILE["sensor.csv"] --> PARSE["CSV parse + validation"]
    PARSE --> REFQ{"Reference column?"}
    REFQ -- Yes --> REF["Provided reference"]
    REFQ -- No --> HAMPEL["Hampel-style despike"]
    HAMPEL --> ZP["Forward/backward EWMA"]
    ZP --> REF

    REF --> GRID["Q/R candidate grid"]
    PARSE --> GRID
    GRID --> RUN["Run scalar Kalman per candidate"]
    RUN --> RMSE["Compute RMSE"]
    RMSE --> BEST["Deterministic best Q/R"]
```

## Audio pipeline

```mermaid
flowchart LR
    CLEAN["Clean PCM WAV"] --> DECODE["Decode + normalize"]
    DECODE --> NOISE["Optional Gaussian + impulses"]
    NOISE --> NOISYWAV["noisy.wav"]

    NOISE --> E["EWMA per channel"]
    NOISE --> M["Median per channel"]
    NOISE --> K["Kalman per channel"]

    E --> EW["ewma.wav"]
    M --> MW["median.wav"]
    K --> KW["kalman.wav"]

    DECODE --> MET["Reference metrics"]
    NOISE --> MET
    E --> MET
    M --> MET
    K --> MET
```

## Startup and state

Every command should construct fresh filters. No filter state should leak between channels, files, traces, tuning candidates, or repeated commands.
