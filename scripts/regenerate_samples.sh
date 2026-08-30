#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$ROOT/scripts/generate_tts_samples.sh"
python3 "$ROOT/scripts/inject_fixture_noise.py" --root "$ROOT"
