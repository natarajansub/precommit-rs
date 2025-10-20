#!/usr/bin/env bash
set -euo pipefail

# Generates small valid PNG placeholders for packaging.
# Run this script from the repository root to replace the placeholder text files with real PNGs.

write_base64_file() {
  local b64="$1"
  local out="$2"
  echo "$b64" | base64 --decode > "$out"
  echo "Wrote $out"
}

# 1x1 transparent PNG (smallest valid PNG)
TRANSPARENT_PNG_B64="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAAWgmWQ0AAAAASUVORK5CYII="

mkdir -p assets
write_base64_file "$TRANSPARENT_PNG_B64" assets/icon.png
write_base64_file "$TRANSPARENT_PNG_B64" assets/icon-256.png
write_base64_file "$TRANSPARENT_PNG_B64" assets/icon-512.png

echo "Placeholder PNGs written to assets/ (replace with real icons before release)."
