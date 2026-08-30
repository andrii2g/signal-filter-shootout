#!/usr/bin/env python3
"""Generate deterministic noisy PCM16 WAV fixtures using only Python stdlib."""

from __future__ import annotations

import argparse
import csv
import math
import random
import struct
import wave
from pathlib import Path


def read_pcm16_mono(path: Path):
    with wave.open(str(path), "rb") as w:
        if w.getnchannels() != 1 or w.getsampwidth() != 2:
            raise ValueError(f"expected mono PCM16: {path}")
        rate = w.getframerate()
        frames = w.readframes(w.getnframes())
    samples = [v[0] / 32768.0 for v in struct.iter_unpack("<h", frames)]
    return rate, samples


def write_pcm16_mono(path: Path, rate: int, samples):
    path.parent.mkdir(parents=True, exist_ok=True)
    pcm = bytearray()
    for x in samples:
        x = max(-1.0, min(1.0, x))
        q = int(round(x * 32767.0)) if x >= 0 else int(round(x * 32768.0))
        q = max(-32768, min(32767, q))
        pcm += struct.pack("<h", q)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(pcm))


def inject(samples, seed, gaussian_sigma, spike_probability, spike_amplitude):
    rng = random.Random(seed)
    out = []
    for x in samples:
        y = x
        if gaussian_sigma > 0:
            y += rng.gauss(0.0, gaussian_sigma)
        if spike_probability > 0 and rng.random() < spike_probability:
            magnitude = rng.uniform(0.5 * spike_amplitude, spike_amplitude)
            y += magnitude if rng.random() < 0.5 else -magnitude
        out.append(y)
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = p.parse_args()

    root = args.root
    manifest = root / "samples/audio/manifest.csv"
    with manifest.open(newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))

    for row in rows:
        clean = root / row["clean_wav"]
        noisy_rel = row["noisy_wav"]
        if not noisy_rel:
            continue
        noisy = root / noisy_rel
        rate, samples = read_pcm16_mono(clean)
        transformed = inject(
            samples,
            int(row["noise_seed"]),
            float(row["gaussian_sigma"]),
            float(row["spike_probability"]),
            float(row["spike_amplitude"]),
        )
        write_pcm16_mono(noisy, rate, transformed)
        print(f"generated {noisy}")


if __name__ == "__main__":
    main()
