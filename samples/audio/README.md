# Bundled speech fixtures

These WAV files are test/demo fixtures for the audio commands in `signal-filter-shootout`.

Clean files were generated locally from original project-authored sentences using command-line eSpeak and converted with FFmpeg to 16 kHz mono signed 16-bit PCM WAV. They are synthetic voices, not recordings of a person.

Noisy variants are deterministic transformations of those clean files. Exact parameters and seeds are listed in `manifest.csv`.

Regenerate on a Linux workstation with eSpeak, FFmpeg, and Python 3:

```bash
./scripts/regenerate_samples.sh
```

The Rust application itself must not depend on eSpeak, FFmpeg, or Python. They are fixture-generation tools only.
