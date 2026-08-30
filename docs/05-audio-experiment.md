# 05 — Audio Experiment

## Why audio is included

The core project is about scalar measurements. Speech deliberately stresses the assumptions:

- EWMA behaves as a simple sample-domain low-pass smoother, reducing high-frequency noise but also dulling speech.
- Median filtering can reject isolated sample impulses/clicks surprisingly well.
- A scalar random-walk Kalman filter can suppress variation but does not model speech harmonics, pitch, formants, or transients.

The expected lesson is model selection, not “Kalman is best”.

## Bundled speech material

All clean samples are generated from project-authored English test sentences using a local text-to-speech engine, then converted to:

```text
sample rate: 16000 Hz
channels:    1
format:      signed PCM
bit depth:   16
```

The repository includes clean references and deterministic noisy variants.

### Sample 01 — general phonetic coverage

Text:

> The quick brown fox jumps over the lazy dog. A clean signal should stay clear while noise is removed.

Purpose:
- broad phoneme coverage;
- easy listening comparison;
- moderate plosives and fricatives.

### Sample 02 — IoT/numeric speech

Text:

> Temperature is twenty one point seven degrees Celsius. Pressure is one thousand thirteen hectopascals. A sudden sensor spike should not become the final estimate.

Purpose:
- numbers;
- longer vowels;
- project-domain vocabulary.

### Sample 03 — filter terminology

Text:

> Kalman follows gradual change. Median rejects isolated clicks. Exponential smoothing is fast and inexpensive.

Purpose:
- direct explanation while listening;
- clear stop consonants in “clicks” and “fast”.

### Sample 04 — transient/sibilant stress

Text:

> Peter packed six crisp packets, bright blue buttons, sharp clicks, soft whispers, and sizzling sensors.

Purpose:
- strong plosives;
- sibilants;
- short high-frequency consonant content likely to be damaged by excessive smoothing.

## Included noisy variants

Each `*_mixed.wav` is generated with:

```text
Gaussian sigma:       0.035
Impulse probability:  0.0008 per sample
Impulse amplitude:    0.85 normalized units
Seed:                 listed in samples/audio/manifest.csv
```

These values are intentionally audible but not catastrophic.

Two diagnostic variants from Sample 01 are also included:
- `01_general_gaussian.wav`: Gaussian only;
- `01_general_impulse.wav`: impulses only.

This makes the expected filter specialization easier to hear.

## Listening protocol

Run the listening comparison with:

```bash
cargo run --release -- audio compare \
  samples/audio/noisy/01_general_mixed.wav \
  --reference samples/audio/clean/01_general.wav \
  --output-dir out/01-general
```

Listen in this order:
1. clean reference;
2. noisy input;
3. median output;
4. EWMA output;
5. Kalman output.

Questions to note:
- Which output removes sparse clicks best?
- Which output sounds muffled?
- Which filter rounds off plosives and sibilants?
- Does lower waveform RMSE correspond to the version you prefer listening to?
- How sensitive is Kalman output to `Q/R` ratio?

## Parameter sweeps worth trying

EWMA:

```text
alpha = 0.05, 0.10, 0.20, 0.40, 0.80
```

Median:

```text
window = 3, 5, 7, 9
```

Kalman:

```text
Q = 1e-5, 1e-4, 1e-3, 1e-2
R = 1e-3, 1e-2, 4e-2, 1e-1
```

Do not automatically tune audio against RMSE in v0.1; manual listening is part of the demonstration.

## WAV validation

Reject:
- compressed WAV codecs;
- unsupported bit depths;
- empty files;
- non-finite decoded values;
- reference files with mismatched metadata/sample count.

For stereo input, instantiate independent filter state per channel. Never interleave both channels through one filter state.
