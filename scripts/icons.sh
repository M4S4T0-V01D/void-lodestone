#!/usr/bin/env bash
# Render the logo to the PNG sizes Tauri, the Flatpak and the website use. Needs rsvg-convert (librsvg).
set -euo pipefail
cd "$(dirname "$0")/.."
python3 -I art/make_logo.py
svg=art/logo.svg
out=src-tauri/icons
mkdir -p "$out" flatpak/icons
for s in 32 128 256 512; do rsvg-convert -w "$s" -h "$s" "$svg" -o "flatpak/icons/${s}x${s}.png"; done
rsvg-convert -w 32 -h 32 "$svg" -o "$out/32x32.png"
rsvg-convert -w 128 -h 128 "$svg" -o "$out/128x128.png"
rsvg-convert -w 256 -h 256 "$svg" -o "$out/128x128@2x.png"
rsvg-convert -w 512 -h 512 "$svg" -o "$out/icon.png"
cp "$svg" src/assets/logo.svg
cp "$svg" public/favicon.svg
cp "$svg" flatpak/icons/io.github.M4S4T0_V01D.void-lodestone.svg
mkdir -p docs/assets && cp "$svg" docs/assets/logo.svg && rsvg-convert -w 512 -h 512 "$svg" -o docs/assets/icon-512.png
echo "icons written"
