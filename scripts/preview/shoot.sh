#!/usr/bin/env bash
# Renders the UI against a fake backend and saves screenshots to docs/assets/.
# Usage: scripts/preview/shoot.sh [name=query ...]   e.g. void=theme=void rs3='theme=ember&game=rs3'
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/docs/assets"
work="$(mktemp -d)"
trap 'kill "${server:-0}" 2>/dev/null || true; rm -rf "$work"' EXIT

npm --prefix "$root" run --silent build >/dev/null
cp -r "$root/dist" "$work/site"
cp "$root/scripts/preview/mock.js" "$work/site/mock.js"
sed -i 's|<head>|<head><script src="/mock.js"></script>|' "$work/site/index.html"

port=47811
python3 -I -m http.server "$port" --bind 127.0.0.1 --directory "$work/site" >/dev/null 2>&1 &
server=$!
sleep 1

browser="$(command -v chromium || command -v google-chrome-stable || command -v google-chrome)"
mkdir -p "$out"
for shot in "${@:-void=theme=void}"; do
  name="${shot%%=*}"; query="${shot#*=}"
  "$browser" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --user-data-dir="$work/profile" --window-size=1280,800 --virtual-time-budget=3000 \
    --screenshot="$out/screenshot-$name.png" "http://127.0.0.1:$port/?$query" 2>/dev/null
  echo "$out/screenshot-$name.png"
done
