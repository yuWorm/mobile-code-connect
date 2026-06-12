#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LANGUAGE="all"
OUT_DIR="$ROOT_DIR/target/uniffi"
LIBRARY=""
BUILD_RELEASE=1

usage() {
  cat <<'EOF'
Usage: scripts/gen-mobile-bindings.sh [options]

Build mobilecode_connect_mobile_core and generate UniFFI Swift/Kotlin bindings.

Options:
  --language swift|kotlin|all  Language to generate (default: all)
  --out-dir DIR                Output root directory (default: target/uniffi)
  --library PATH               Existing compiled mobile-core library to use
  --no-build                   Skip cargo build --release
  -h, --help                   Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --language)
      LANGUAGE="${2:-}"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="${2:-}"
      shift 2
      ;;
    --library)
      LIBRARY="${2:-}"
      BUILD_RELEASE=0
      shift 2
      ;;
    --no-build)
      BUILD_RELEASE=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$LANGUAGE" in
  swift|kotlin|all) ;;
  *)
    echo "--language must be swift, kotlin, or all" >&2
    exit 2
    ;;
esac

if ! command -v uniffi-bindgen >/dev/null 2>&1; then
  cat >&2 <<'EOF'
uniffi-bindgen is required.
Install it with:
  cargo install uniffi --version 0.31.1 --features cli --locked
EOF
  exit 127
fi

if [[ "$BUILD_RELEASE" -eq 1 ]]; then
  cargo build -p mobilecode_connect_mobile_core --release
fi

if [[ -z "$LIBRARY" ]]; then
  case "$(uname -s)" in
    Darwin) LIBRARY="$ROOT_DIR/target/release/libmobilecode_connect_mobile_core.dylib" ;;
    Linux) LIBRARY="$ROOT_DIR/target/release/libmobilecode_connect_mobile_core.so" ;;
    *)
      echo "unsupported host OS; pass --library explicitly" >&2
      exit 2
      ;;
  esac
elif [[ "$LIBRARY" != /* ]]; then
  LIBRARY="$ROOT_DIR/$LIBRARY"
fi

if [[ ! -f "$LIBRARY" ]]; then
  echo "mobile-core library not found: $LIBRARY" >&2
  exit 1
fi

generate() {
  local language="$1"
  local language_out_dir="$OUT_DIR/$language"
  mkdir -p "$language_out_dir"
  uniffi-bindgen generate "$LIBRARY" \
    --library \
    --metadata-no-deps \
    --crate mobilecode_connect_mobile_core \
    --language "$language" \
    --out-dir "$language_out_dir"
}

case "$LANGUAGE" in
  swift)
    generate swift
    ;;
  kotlin)
    generate kotlin
    ;;
  all)
    generate swift
    generate kotlin
    ;;
esac
