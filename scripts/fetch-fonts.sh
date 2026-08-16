#!/usr/bin/env bash
# Downloads the two font families into resources/fonts, as static instances.
#
# The design ships woff2, which no native text stack reads, and Google Fonts
# ships these two families as variable TTFs only. Both facts matter:
#
#   - Registering the variable file and asking for a weight leaves the result up
#     to how completely the shaper under iced supports the `wght` axis. When
#     that support is missing the weights collapse together and nothing errors,
#     so the failure is invisible until someone compares a screenshot.
#   - Cutting each weight into its own static TTF makes weight selection a plain
#     font-database lookup, which every shaper does the same way.
#
# fontTools is a build-time dependency only. Without it the script stops rather
# than silently leaving the variable files in place, because that failure is
# invisible until someone looks closely at a screenshot.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p resources/fonts build/fonts

python3 -c "import fontTools" 2>/dev/null || {
  echo "fontTools is missing. Install it with:" >&2
  echo "    python3 -m pip install --user fonttools" >&2
  exit 1
}

base="https://raw.githubusercontent.com/google/fonts/main/ofl"

variable() {
  local url="$1" out="build/fonts/$2"
  [ -s "$out" ] || { echo "fetching $2"; curl -fsSL "$url" -o "$out"; }
}

instance() {
  local source="build/fonts/$1" weight="$2" out="resources/fonts/$3"
  if [ -s "$out" ]; then echo "have $3"; return; fi
  echo "instancing $3 at wght=$weight"
  python3 -m fontTools.varLib.instancer "$source" "wght=$weight" -o "$out" >/dev/null
}

variable "$base/onest/Onest%5Bwght%5D.ttf"         "Onest[wght].ttf"
variable "$base/unbounded/Unbounded%5Bwght%5D.ttf" "Unbounded[wght].ttf"

# 500 is the lightest body weight, 700 a row title, 800 anything emphatic.
instance "Onest[wght].ttf"     500 Onest-Medium.ttf
instance "Onest[wght].ttf"     700 Onest-Bold.ttf
instance "Onest[wght].ttf"     800 Onest-ExtraBold.ttf
# Unbounded is display only, and the design sets it at 800 everywhere.
instance "Unbounded[wght].ttf" 800 Unbounded-ExtraBold.ttf

ls -la resources/fonts
