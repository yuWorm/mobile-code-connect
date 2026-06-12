#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IOS_DIR="$ROOT_DIR/mobile/ios"
STAGING_DIR="$ROOT_DIR/target/mobile-package/ios"
IOS_TARGETS_CSV="aarch64-apple-ios,aarch64-apple-ios-sim"
IOS_MIN_VERSION="${IOS_MIN_VERSION:-17.0}"
XCFRAMEWORK_OUTPUT=""
SKIP_BUILD=0
SKIP_XCODEBUILD=0
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/package-mobile-ios.sh [options]

Build the iOS SwiftPM package inputs for MobileCodeConnectMobileSdk.

Options:
  --targets CSV        Rust iOS targets to build
                       (default: aarch64-apple-ios,aarch64-apple-ios-sim)
  --staging-dir DIR    Temporary output directory (default: target/mobile-package/ios)
  --ios-min-version V  iOS deployment target for Rust builds (default: 17.0)
  --xcframework-output PATH
                       XCFramework output path
                       (default: mobile/ios/Artifacts/mobilecode_connect_mobile_coreFFI.xcframework)
  --skip-build         Do not run cargo build; require existing target artifacts
  --skip-xcodebuild    Stage bindings/headers but do not create the XCFramework
  --dry-run            Print the packaging plan without requiring toolchains
  -h, --help           Show this help
EOF
}

log() {
  printf '[package-mobile-ios] %s\n' "$*"
}

die() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required"
}

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    die "shasum or sha256sum is required to write the package manifest"
  fi
}

file_size() {
  wc -c <"$1" | tr -d '[:space:]'
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

is_ios_simulator_target() {
  case "$1" in
    aarch64-apple-ios-sim|x86_64-apple-ios) return 0 ;;
    *) return 1 ;;
  esac
}

validate_ios_target() {
  case "$1" in
    aarch64-apple-ios|aarch64-apple-ios-sim|x86_64-apple-ios) ;;
    *) die "unsupported iOS target: $1" ;;
  esac
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --targets)
      IOS_TARGETS_CSV="${2:-}"
      shift 2
      ;;
    --staging-dir)
      STAGING_DIR="${2:-}"
      shift 2
      ;;
    --ios-min-version)
      IOS_MIN_VERSION="${2:-}"
      shift 2
      ;;
    --xcframework-output)
      XCFRAMEWORK_OUTPUT="${2:-}"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --skip-xcodebuild)
      SKIP_XCODEBUILD=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
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

IFS=',' read -r -a IOS_TARGETS <<<"$IOS_TARGETS_CSV"
[[ "${#IOS_TARGETS[@]}" -gt 0 && -n "${IOS_TARGETS[0]}" ]] || die "--targets must not be empty"
[[ "$IOS_MIN_VERSION" =~ ^[0-9]+([.][0-9]+){0,2}$ ]] || die "--ios-min-version must be a version like 17.0"
for target in "${IOS_TARGETS[@]}"; do
  validate_ios_target "$target"
done

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: planning iOS package without checking toolchains or artifacts"
else
  require_cmd cargo
  if [[ "$SKIP_XCODEBUILD" -eq 0 ]]; then
      require_cmd xcodebuild
  fi
fi
log "IPHONEOS_DEPLOYMENT_TARGET=$IOS_MIN_VERSION"

ensure_rust_target() {
  local target="$1"
  if command -v rustup >/dev/null 2>&1 && ! rustup target list --installed | grep -qx "$target"; then
    die "Rust target $target is not installed. Run: rustup target add $target"
  fi
}

build_target() {
  local target="$1"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: IPHONEOS_DEPLOYMENT_TARGET=$IOS_MIN_VERSION cargo build -p mobilecode_connect_mobile_core --release --target $target"
    return 0
  fi

  ensure_rust_target "$target"

  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    log "IPHONEOS_DEPLOYMENT_TARGET=$IOS_MIN_VERSION cargo build -p mobilecode_connect_mobile_core --release --target $target"
    IPHONEOS_DEPLOYMENT_TARGET="$IOS_MIN_VERSION" cargo build -p mobilecode_connect_mobile_core --release --target "$target"
  fi
}

static_library_for_target() {
  local target="$1"
  printf '%s/target/%s/release/libmobilecode_connect_mobile_core.a' "$ROOT_DIR" "$target"
}

for target in "${IOS_TARGETS[@]}"; do
  build_target "$target"
done

METADATA_LIBRARY="$(static_library_for_target "${IOS_TARGETS[0]}")"
if [[ "$DRY_RUN" -eq 0 ]]; then
  [[ -f "$METADATA_LIBRARY" ]] || die "mobile-core static library not found: $METADATA_LIBRARY"
fi

log "scripts/gen-mobile-bindings.sh --language swift"
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: scripts/gen-mobile-bindings.sh --language swift --library $METADATA_LIBRARY --out-dir $STAGING_DIR/uniffi --no-build"
else
  "$ROOT_DIR/scripts/gen-mobile-bindings.sh" \
    --language swift \
    --library "$METADATA_LIBRARY" \
    --out-dir "$STAGING_DIR/uniffi" \
    --no-build
fi

SWIFT_BINDINGS_DIR="$STAGING_DIR/uniffi/swift"
GENERATED_SWIFT_DIR="$IOS_DIR/Sources/MobileCodeConnectMobileSdk/Generated"
HEADERS_DIR="$STAGING_DIR/headers"
ARTIFACTS_DIR="$IOS_DIR/Artifacts"
SIMULATOR_LIBRARY="$STAGING_DIR/libmobilecode_connect_mobile_core_simulator.a"
XCFRAMEWORK_RELATIVE_PATH="Artifacts/mobilecode_connect_mobile_coreFFI.xcframework"
XCFRAMEWORK_PATH="${XCFRAMEWORK_OUTPUT:-$IOS_DIR/$XCFRAMEWORK_RELATIVE_PATH}"
XCFRAMEWORK_DIR="$(dirname "$XCFRAMEWORK_PATH")"
MANIFEST_PATH="$STAGING_DIR/mobile-package-manifest.json"
log "XCFRAMEWORK_OUTPUT=$XCFRAMEWORK_PATH"

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: mkdir -p $GENERATED_SWIFT_DIR $HEADERS_DIR $XCFRAMEWORK_DIR"
  log "dry-run: cp $SWIFT_BINDINGS_DIR/mobilecode_connect_mobile_core.swift $GENERATED_SWIFT_DIR/mobilecode_connect_mobile_core.swift"
  log "dry-run: cp $SWIFT_BINDINGS_DIR/mobilecode_connect_mobile_coreFFI.h $HEADERS_DIR/mobilecode_connect_mobile_coreFFI.h"
  log "dry-run: cp $SWIFT_BINDINGS_DIR/mobilecode_connect_mobile_coreFFI.modulemap $HEADERS_DIR/module.modulemap"
else
  mkdir -p "$GENERATED_SWIFT_DIR" "$HEADERS_DIR" "$XCFRAMEWORK_DIR"
  cp "$SWIFT_BINDINGS_DIR/mobilecode_connect_mobile_core.swift" \
    "$GENERATED_SWIFT_DIR/mobilecode_connect_mobile_core.swift"
  cp "$SWIFT_BINDINGS_DIR/mobilecode_connect_mobile_coreFFI.h" \
    "$HEADERS_DIR/mobilecode_connect_mobile_coreFFI.h"
  cp "$SWIFT_BINDINGS_DIR/mobilecode_connect_mobile_coreFFI.modulemap" \
    "$HEADERS_DIR/module.modulemap"
fi

write_manifest_artifact() {
  local file="$1"
  [[ -f "$file" ]] || return 0

  if [[ "$MANIFEST_FIRST" -eq 0 ]]; then
    printf ',\n'
  fi
  MANIFEST_FIRST=0

  local escaped_path
  escaped_path="$(json_escape "$file")"
  printf '    {"path":"%s","sha256":"%s","bytes":%s}' \
    "$escaped_path" \
    "$(sha256_file "$file")" \
    "$(file_size "$file")"
}

write_ios_manifest() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: write $MANIFEST_PATH with sha256 entries"
    return 0
  fi

  mkdir -p "$(dirname "$MANIFEST_PATH")"
  {
    printf '{\n'
    printf '  "package":"ios",\n'
    printf '  "ios_min_version":"%s",\n' "$(json_escape "$IOS_MIN_VERSION")"
    printf '  "xcframework_output":"%s",\n' "$(json_escape "$XCFRAMEWORK_PATH")"
    printf '  "artifacts":[\n'
    MANIFEST_FIRST=1
    for target in "${IOS_TARGETS[@]}"; do
      write_manifest_artifact "$(static_library_for_target "$target")"
    done
    write_manifest_artifact "$SIMULATOR_LIBRARY"
    write_manifest_artifact "$GENERATED_SWIFT_DIR/mobilecode_connect_mobile_core.swift"
    write_manifest_artifact "$HEADERS_DIR/mobilecode_connect_mobile_coreFFI.h"
    write_manifest_artifact "$HEADERS_DIR/module.modulemap"
    if [[ -d "$XCFRAMEWORK_PATH" ]]; then
      while IFS= read -r file; do
        write_manifest_artifact "$file"
      done < <(find "$XCFRAMEWORK_PATH" -type f | sort)
    else
      write_manifest_artifact "$XCFRAMEWORK_PATH"
    fi
    printf '\n  ]\n'
    printf '}\n'
  } >"$MANIFEST_PATH"
  log "wrote $MANIFEST_PATH"
}

if [[ "$SKIP_XCODEBUILD" -eq 1 ]]; then
  log "skipping xcodebuild; staged Swift bindings and FFI headers"
  write_ios_manifest
  exit 0
fi

DEVICE_LIBRARIES=()
SIMULATOR_LIBRARIES=()
for target in "${IOS_TARGETS[@]}"; do
  library="$(static_library_for_target "$target")"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    [[ -f "$library" ]] || die "mobile-core static library not found: $library"
  fi

  if is_ios_simulator_target "$target"; then
    SIMULATOR_LIBRARIES+=("$library")
  else
    DEVICE_LIBRARIES+=("$library")
  fi
done

SIMULATOR_XCFRAMEWORK_LIBRARY=""
prepare_simulator_library() {
  local simulator_count="${#SIMULATOR_LIBRARIES[@]}"
  if [[ "$simulator_count" -eq 0 ]]; then
    return 0
  fi

  if [[ "$simulator_count" -eq 1 ]]; then
    SIMULATOR_XCFRAMEWORK_LIBRARY="${SIMULATOR_LIBRARIES[0]}"
    return 0
  fi

  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: lipo -create ${SIMULATOR_LIBRARIES[*]} -output $SIMULATOR_LIBRARY"
  else
    require_cmd lipo
    mkdir -p "$(dirname "$SIMULATOR_LIBRARY")"
    rm -f "$SIMULATOR_LIBRARY"
    lipo -create "${SIMULATOR_LIBRARIES[@]}" -output "$SIMULATOR_LIBRARY"
  fi

  SIMULATOR_XCFRAMEWORK_LIBRARY="$SIMULATOR_LIBRARY"
}

XCODEBUILD_ARGS=(-create-xcframework)
for library in "${DEVICE_LIBRARIES[@]}"; do
  XCODEBUILD_ARGS+=(-library "$library" -headers "$HEADERS_DIR")
done
prepare_simulator_library
if [[ -n "$SIMULATOR_XCFRAMEWORK_LIBRARY" ]]; then
  XCODEBUILD_ARGS+=(-library "$SIMULATOR_XCFRAMEWORK_LIBRARY" -headers "$HEADERS_DIR")
fi
XCODEBUILD_ARGS+=(-output "$XCFRAMEWORK_PATH")

log "xcodebuild -create-xcframework -> $XCFRAMEWORK_PATH"
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: xcodebuild ${XCODEBUILD_ARGS[*]}"
else
  rm -rf "$XCFRAMEWORK_PATH"
  xcodebuild "${XCODEBUILD_ARGS[@]}"
fi
write_ios_manifest
log "iOS package staged at $IOS_DIR"
