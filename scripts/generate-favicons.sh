#!/usr/bin/env bash
#
# generate-favicons.sh — Process a logo and generate comprehensive favicon suite
#
# Usage: ./scripts/generate-favicons.sh <input-image> [--install]
#
# Steps:
#   1. Trim transparent/near-transparent borders
#   2. Make square (center content with transparent padding)
#   3. Generate all favicon sizes
#   4. Optionally copy to client/public/
#
# Requires: ImageMagick 7+ (magick command)

set -euo pipefail

# ─── Config ─────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="${PROJECT_ROOT}/.dev/favicons"
CLIENT_PUBLIC="${PROJECT_ROOT}/client/public"

# Alpha threshold for trimming (pixels below this % alpha are considered transparent)
ALPHA_THRESHOLD="15%"

# ─── Usage ──────────────────────────────────────────────────────────
usage() {
  cat <<EOF
Usage: $(basename "$0") <input-image> [--install]

Arguments:
  input-image    Path to source logo image (PNG recommended)
  --install      Copy generated favicons to client/public/

Examples:
  $(basename "$0") ~/Downloads/logo.png
  $(basename "$0") ~/Downloads/logo.png --install

Output: Generated favicons in ${OUTPUT_DIR}/
EOF
  exit 1
}

# ─── Check dependencies ─────────────────────────────────────────────
check_deps() {
  if ! command -v magick &>/dev/null; then
    echo "Error: ImageMagick 7+ required (magick command not found)"
    echo "Install: brew install imagemagick (macOS) or pacman -S imagemagick (Arch)"
    exit 1
  fi
}

# ─── Main ───────────────────────────────────────────────────────────
main() {
  local input=""
  local install=false

  # Parse args
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --install) install=true; shift ;;
      -h|--help) usage ;;
      -*) echo "Unknown option: $1"; usage ;;
      *) input="$1"; shift ;;
    esac
  done

  [[ -z "$input" ]] && usage
  [[ ! -f "$input" ]] && { echo "Error: File not found: $input"; exit 1; }

  check_deps

  # Setup
  mkdir -p "$OUTPUT_DIR"
  local tmp_dir
  tmp_dir=$(mktemp -d)
  trap "rm -rf $tmp_dir" EXIT

  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "  Favicon Generator"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo ""
  echo "Input:  $input"
  echo "Output: $OUTPUT_DIR"
  echo ""

  # ─── Step 1: Analyze input ────────────────────────────────────────
  echo "→ Analyzing input image..."
  local orig_size
  orig_size=$(magick identify -format "%wx%h" "$input")
  echo "  Original size: $orig_size"

  # ─── Step 2: Trim transparent borders ─────────────────────────────
  echo "→ Trimming transparent borders (threshold: $ALPHA_THRESHOLD)..."
  magick "$input" \
    -channel A -threshold "$ALPHA_THRESHOLD" +channel \
    -trim +repage \
    "$tmp_dir/trimmed.png"

  local trimmed_size
  trimmed_size=$(magick identify -format "%wx%h" "$tmp_dir/trimmed.png")
  echo "  Trimmed size: $trimmed_size"

  # ─── Step 3: Make square ──────────────────────────────────────────
  echo "→ Making square..."
  local width height max_dim
  width=$(magick identify -format "%w" "$tmp_dir/trimmed.png")
  height=$(magick identify -format "%h" "$tmp_dir/trimmed.png")
  max_dim=$((width > height ? width : height))

  magick "$tmp_dir/trimmed.png" \
    -gravity center \
    -background none \
    -extent "${max_dim}x${max_dim}" \
    "$tmp_dir/square.png"

  echo "  Square size: ${max_dim}x${max_dim}"

  # ─── Step 4: Generate all sizes ───────────────────────────────────
  echo "→ Generating favicon suite..."

  # Standard favicons
  magick "$tmp_dir/square.png" -resize 16x16   "$OUTPUT_DIR/favicon-16x16.png"
  magick "$tmp_dir/square.png" -resize 32x32   "$OUTPUT_DIR/favicon-32x32.png"
  magick "$tmp_dir/square.png" -resize 32x32   "$OUTPUT_DIR/favicon.png"
  echo "  ✓ favicon-16x16.png, favicon-32x32.png, favicon.png"

  # Apple touch icon (180x180 is standard)
  magick "$tmp_dir/square.png" -resize 180x180 "$OUTPUT_DIR/apple-touch-icon.png"
  echo "  ✓ apple-touch-icon.png (180px)"

  # PWA icons
  magick "$tmp_dir/square.png" -resize 192x192 "$OUTPUT_DIR/logo192.png"
  magick "$tmp_dir/square.png" -resize 512x512 "$OUTPUT_DIR/logo512.png"
  echo "  ✓ logo192.png, logo512.png (PWA)"

  # Android Chrome (maskable)
  magick "$tmp_dir/square.png" -resize 192x192 "$OUTPUT_DIR/android-chrome-192x192.png"
  magick "$tmp_dir/square.png" -resize 512x512 "$OUTPUT_DIR/android-chrome-512x512.png"
  echo "  ✓ android-chrome-192x192.png, android-chrome-512x512.png"

  # Windows tile
  magick "$tmp_dir/square.png" -resize 150x150 "$OUTPUT_DIR/mstile-150x150.png"
  echo "  ✓ mstile-150x150.png (Windows)"

  # Multi-resolution ICO (16, 32, 48)
  magick "$tmp_dir/square.png" -resize 16x16 "$tmp_dir/ico-16.png"
  magick "$tmp_dir/square.png" -resize 32x32 "$tmp_dir/ico-32.png"
  magick "$tmp_dir/square.png" -resize 48x48 "$tmp_dir/ico-48.png"
  magick "$tmp_dir/ico-16.png" "$tmp_dir/ico-32.png" "$tmp_dir/ico-48.png" "$OUTPUT_DIR/favicon.ico"
  echo "  ✓ favicon.ico (16/32/48px multi-resolution)"

  # Full resolution logo
  cp "$tmp_dir/square.png" "$OUTPUT_DIR/logo.png"
  local logo_size
  logo_size=$(du -h "$OUTPUT_DIR/logo.png" | cut -f1)
  echo "  ✓ logo.png (${max_dim}px, ${logo_size})"

  # ─── Step 5: Install (optional) ───────────────────────────────────
  if [[ "$install" == true ]]; then
    echo ""
    echo "→ Installing to $CLIENT_PUBLIC..."
    cp "$OUTPUT_DIR"/*.png "$OUTPUT_DIR"/*.ico "$CLIENT_PUBLIC/"
    echo "  ✓ Copied all favicons"
    echo ""
    echo "  Don't forget to rebuild: cd client && yarn build"
  fi

  # ─── Summary ──────────────────────────────────────────────────────
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "  Done! Generated files:"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  ls -la "$OUTPUT_DIR"
  echo ""
  echo "To install: $(basename "$0") $input --install"
}

main "$@"
