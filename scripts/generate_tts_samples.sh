#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLEAN="$ROOT/samples/audio/clean"
TRANS="$ROOT/samples/audio/transcripts"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v espeak >/dev/null || { echo "espeak is required" >&2; exit 1; }
command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }

mkdir -p "$CLEAN"

for txt in "$TRANS"/*.txt; do
  base="$(basename "$txt" .txt)"
  raw="$TMP/$base.raw.wav"
  out="$CLEAN/$base.wav"

  espeak -v en-us -s 150 -p 48 -a 160 -f "$txt" -w "$raw"
  ffmpeg -hide_banner -loglevel error -y -i "$raw" \
    -ac 1 -ar 16000 -c:a pcm_s16le "$out"
  echo "generated $out"
done
