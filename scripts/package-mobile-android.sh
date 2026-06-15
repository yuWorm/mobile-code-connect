#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$ROOT_DIR/mobile/android"
STAGING_DIR="$ROOT_DIR/target/mobile-package/android"
ANDROID_TARGETS_CSV="aarch64-linux-android,armv7-linux-androideabi,x86_64-linux-android,i686-linux-android"
ANDROID_API=23
NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
NDK_HOST_TAG="${ANDROID_NDK_HOST_TAG:-}"
GRADLE_TASK="${GRADLE_TASK:-assembleRelease}"
AAR_OUTPUT_DIR=""
STRIP_NATIVE_LIBS="${STRIP_NATIVE_LIBS:-1}"
SKIP_BUILD=0
SKIP_GRADLE=0
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/package-mobile-android.sh [options]

Build the Android AAR inputs for MobileCodeConnectMobileSdk.

Options:
  --targets CSV      Rust Android targets to build
                     (default: aarch64-linux-android,armv7-linux-androideabi,x86_64-linux-android,i686-linux-android)
  --staging-dir DIR  Temporary output directory (default: target/mobile-package/android)
  --ndk-home DIR     Android NDK directory (default: ANDROID_NDK_HOME, ANDROID_NDK_ROOT, or ANDROID_HOME/ndk latest)
  --ndk-host-tag TAG NDK host tag (default: linux-x86_64 on Linux, darwin-x86_64 on macOS)
  --android-api API  Android API level for NDK clang wrappers (default: 23)
  --gradle-task TASK Gradle task to run after staging (default: assembleRelease)
  --aar-output-dir DIR
                     Directory where built AAR files are copied
                     (default: target/mobile-package/android/aar)
  --strip            Strip staged native libraries with NDK llvm-strip (default)
  --no-strip         Do not strip staged native libraries
  --skip-build       Do not run cargo build; require existing target artifacts
  --skip-gradle      Stage bindings/native libraries but do not run the Gradle task
  --dry-run          Print the packaging plan without requiring toolchains
  -h, --help         Show this help
EOF
}

log() {
  printf '[package-mobile-android] %s\n' "$*"
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

abi_for_target() {
  case "$1" in
    aarch64-linux-android) echo "arm64-v8a" ;;
    armv7-linux-androideabi) echo "armeabi-v7a" ;;
    x86_64-linux-android) echo "x86_64" ;;
    i686-linux-android) echo "x86" ;;
    *) die "unsupported Android target: $1" ;;
  esac
}

clang_name_for_target() {
  case "$1" in
    aarch64-linux-android) echo "aarch64-linux-android${ANDROID_API}-clang" ;;
    armv7-linux-androideabi) echo "armv7a-linux-androideabi${ANDROID_API}-clang" ;;
    x86_64-linux-android) echo "x86_64-linux-android${ANDROID_API}-clang" ;;
    i686-linux-android) echo "i686-linux-android${ANDROID_API}-clang" ;;
    *) die "unsupported Android target: $1" ;;
  esac
}

cargo_linker_env_for_target() {
  case "$1" in
    aarch64-linux-android) echo "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER" ;;
    armv7-linux-androideabi) echo "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER" ;;
    x86_64-linux-android) echo "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER" ;;
    i686-linux-android) echo "CARGO_TARGET_I686_LINUX_ANDROID_LINKER" ;;
    *) die "unsupported Android target: $1" ;;
  esac
}

detect_ndk_host_tag() {
  if [[ -n "$NDK_HOST_TAG" ]]; then
    echo "$NDK_HOST_TAG"
    return 0
  fi

  case "$(uname -s)" in
    Darwin) echo "darwin-x86_64" ;;
    Linux) echo "linux-x86_64" ;;
    *) die "unsupported host OS; pass --ndk-host-tag explicitly" ;;
  esac
}

resolve_ndk_home() {
  if [[ -n "$NDK_HOME" ]]; then
    echo "$NDK_HOME"
    return 0
  fi

  local candidate_root=""
  for candidate_root in \
    "${ANDROID_HOME:-}/ndk" \
    "${ANDROID_SDK_ROOT:-}/ndk" \
    "$HOME/Library/Android/sdk/ndk" \
    "$HOME/Android/Sdk/ndk"; do
    [[ -d "$candidate_root" ]] || continue
    local latest
    latest="$(find "$candidate_root" -mindepth 1 -maxdepth 1 -type d | sort | tail -n 1)"
    if [[ -n "$latest" ]]; then
      echo "$latest"
      return 0
    fi
  done

  die "Android NDK not found. Set ANDROID_NDK_HOME or pass --ndk-home"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --targets)
      ANDROID_TARGETS_CSV="${2:-}"
      shift 2
      ;;
    --staging-dir)
      STAGING_DIR="${2:-}"
      shift 2
      ;;
    --ndk-home)
      NDK_HOME="${2:-}"
      shift 2
      ;;
    --ndk-host-tag)
      NDK_HOST_TAG="${2:-}"
      shift 2
      ;;
    --android-api)
      ANDROID_API="${2:-}"
      shift 2
      ;;
    --gradle-task)
      GRADLE_TASK="${2:-}"
      shift 2
      ;;
    --aar-output-dir)
      AAR_OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --strip)
      STRIP_NATIVE_LIBS=1
      shift
      ;;
    --no-strip)
      STRIP_NATIVE_LIBS=0
      shift
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --skip-gradle)
      SKIP_GRADLE=1
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

IFS=',' read -r -a ANDROID_TARGETS <<<"$ANDROID_TARGETS_CSV"
[[ "${#ANDROID_TARGETS[@]}" -gt 0 && -n "${ANDROID_TARGETS[0]}" ]] || die "--targets must not be empty"
[[ "$ANDROID_API" =~ ^[0-9]+$ ]] || die "--android-api must be a number"
[[ -n "$GRADLE_TASK" ]] || die "--gradle-task must not be empty"
[[ "$STRIP_NATIVE_LIBS" == "0" || "$STRIP_NATIVE_LIBS" == "1" ]] || die "STRIP_NATIVE_LIBS must be 0 or 1"
AAR_OUTPUT_DIR="${AAR_OUTPUT_DIR:-$STAGING_DIR/aar}"

if [[ "$DRY_RUN" -eq 1 ]]; then
  NDK_HOME="${NDK_HOME:-/opt/android-ndk}"
  NDK_HOST_TAG="$(detect_ndk_host_tag)"
  log "dry-run: planning Android package without checking toolchains or artifacts"
else
  require_cmd cargo
  NDK_HOME="$(resolve_ndk_home)"
  NDK_HOST_TAG="$(detect_ndk_host_tag)"
fi

NDK_TOOLCHAIN_BIN="$NDK_HOME/toolchains/llvm/prebuilt/$NDK_HOST_TAG/bin"
STRIP_BIN="$NDK_TOOLCHAIN_BIN/llvm-strip"
MANIFEST_PATH="$STAGING_DIR/mobile-package-manifest.json"
log "ANDROID_NDK_HOME=$NDK_HOME"
log "ANDROID_NDK_HOST_TAG=$NDK_HOST_TAG"
log "ANDROID_API=$ANDROID_API"
log "GRADLE_TASK=$GRADLE_TASK"
log "AAR_OUTPUT_DIR=$AAR_OUTPUT_DIR"
log "STRIP_NATIVE_LIBS=$STRIP_NATIVE_LIBS"

if [[ "$DRY_RUN" -eq 0 && "$STRIP_NATIVE_LIBS" -eq 1 ]]; then
  [[ -x "$STRIP_BIN" ]] || die "NDK llvm-strip not found or not executable: $STRIP_BIN"
fi

ensure_rust_target() {
  local target="$1"
  if command -v rustup >/dev/null 2>&1 && ! rustup target list --installed | grep -qx "$target"; then
    die "Rust target $target is not installed. Run: rustup target add $target"
  fi
}

build_target() {
  local target="$1"
  configure_ndk_for_target "$target"

  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: cargo build -p mobilecode_connect_mobile_core --release --target $target"
    return 0
  fi

  ensure_rust_target "$target"

  if [[ "$SKIP_BUILD" -eq 0 ]]; then
    log "cargo build -p mobilecode_connect_mobile_core --release --target $target"
    cargo build -p mobilecode_connect_mobile_core --release --target "$target"
  fi
}

configure_ndk_for_target() {
  local target="$1"
  local clang="$NDK_TOOLCHAIN_BIN/$(clang_name_for_target "$target")"
  local ar="$NDK_TOOLCHAIN_BIN/llvm-ar"
  local ranlib="$NDK_TOOLCHAIN_BIN/llvm-ranlib"
  local cargo_linker_env
  cargo_linker_env="$(cargo_linker_env_for_target "$target")"
  local target_env="${target//-/_}"
  local cc_env="CC_${target_env}"
  local ar_env="AR_${target_env}"
  local ranlib_env="RANLIB_${target_env}"

  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: $cargo_linker_env=$clang"
    log "dry-run: $cc_env=$clang"
    log "dry-run: $ar_env=$ar"
    log "dry-run: $ranlib_env=$ranlib"
    return 0
  fi

  [[ -x "$clang" ]] || die "NDK clang not found or not executable: $clang"
  [[ -x "$ar" ]] || die "NDK llvm-ar not found or not executable: $ar"
  [[ -x "$ranlib" ]] || die "NDK llvm-ranlib not found or not executable: $ranlib"

  export "${cargo_linker_env}=${clang}"
  export "${cc_env}=${clang}"
  export "${ar_env}=${ar}"
  export "${ranlib_env}=${ranlib}"
}

shared_library_for_target() {
  local target="$1"
  printf '%s/target/%s/release/libmobilecode_connect_mobile_core.so' "$ROOT_DIR" "$target"
}

for target in "${ANDROID_TARGETS[@]}"; do
  build_target "$target"
done

METADATA_LIBRARY="$(shared_library_for_target "${ANDROID_TARGETS[0]}")"
if [[ "$DRY_RUN" -eq 0 ]]; then
  [[ -f "$METADATA_LIBRARY" ]] || die "mobile-core shared library not found: $METADATA_LIBRARY"
fi

log "scripts/gen-mobile-bindings.sh --language kotlin"
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: scripts/gen-mobile-bindings.sh --language kotlin --library $METADATA_LIBRARY --out-dir $STAGING_DIR/uniffi --no-build"
else
  "$ROOT_DIR/scripts/gen-mobile-bindings.sh" \
    --language kotlin \
    --library "$METADATA_LIBRARY" \
    --out-dir "$STAGING_DIR/uniffi" \
    --no-build
fi

KOTLIN_DEST="$ANDROID_DIR/src/main/java"
JNILIBS_DEST="$ANDROID_DIR/src/main/jniLibs"
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: mkdir -p $KOTLIN_DEST $JNILIBS_DEST"
  log "dry-run: rm -rf $KOTLIN_DEST/uniffi"
  log "dry-run: rm -rf $JNILIBS_DEST"
  log "dry-run: cp -R $STAGING_DIR/uniffi/kotlin/. $KOTLIN_DEST/"
else
  mkdir -p "$KOTLIN_DEST"
  rm -rf "$KOTLIN_DEST/uniffi"
  rm -rf "$JNILIBS_DEST"
  mkdir -p "$JNILIBS_DEST"
  cp -R "$STAGING_DIR/uniffi/kotlin/." "$KOTLIN_DEST/"
  [[ -d "$KOTLIN_DEST/uniffi" ]] || die "generated UniFFI Kotlin bindings not found under $KOTLIN_DEST/uniffi"
fi

for target in "${ANDROID_TARGETS[@]}"; do
  abi="$(abi_for_target "$target")"
  library="$(shared_library_for_target "$target")"
  staged_library="$JNILIBS_DEST/$abi/libmobilecode_connect_mobile_core.so"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: cp $library $staged_library"
    if [[ "$STRIP_NATIVE_LIBS" -eq 1 ]]; then
      log "dry-run: $STRIP_BIN --strip-unneeded $staged_library"
    fi
  else
    [[ -f "$library" ]] || die "mobile-core shared library not found: $library"
    mkdir -p "$JNILIBS_DEST/$abi"
    cp "$library" "$staged_library"
    if [[ "$STRIP_NATIVE_LIBS" -eq 1 ]]; then
      "$STRIP_BIN" --strip-unneeded "$staged_library"
    fi
  fi
done

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

write_android_manifest() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: write $MANIFEST_PATH with sha256 entries"
    return 0
  fi

  mkdir -p "$(dirname "$MANIFEST_PATH")"
  {
    printf '{\n'
    printf '  "package":"android",\n'
    printf '  "android_api":%s,\n' "$ANDROID_API"
    printf '  "gradle_task":"%s",\n' "$(json_escape "$GRADLE_TASK")"
    printf '  "strip_native_libs":%s,\n' "$STRIP_NATIVE_LIBS"
    printf '  "aar_output_dir":"%s",\n' "$(json_escape "$AAR_OUTPUT_DIR")"
    printf '  "artifacts":[\n'
    MANIFEST_FIRST=1
    if [[ -d "$KOTLIN_DEST" ]]; then
      while IFS= read -r file; do
        write_manifest_artifact "$file"
      done < <(find "$KOTLIN_DEST" -type f | sort)
    fi
    if [[ -d "$JNILIBS_DEST" ]]; then
      while IFS= read -r file; do
        write_manifest_artifact "$file"
      done < <(find "$JNILIBS_DEST" -type f | sort)
    fi
    if [[ -d "$AAR_OUTPUT_DIR" ]]; then
      while IFS= read -r file; do
        write_manifest_artifact "$file"
      done < <(find "$AAR_OUTPUT_DIR" -type f -name '*.aar' | sort)
    fi
    printf '\n  ]\n'
    printf '}\n'
  } >"$MANIFEST_PATH"
  log "wrote $MANIFEST_PATH"
}

copy_android_aars() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    log "dry-run: mkdir -p $AAR_OUTPUT_DIR"
    log "dry-run: cp $ANDROID_DIR/build/outputs/aar/*.aar $AAR_OUTPUT_DIR/"
    return 0
  fi

  mkdir -p "$AAR_OUTPUT_DIR"
  shopt -s nullglob
  local aar_files=("$ANDROID_DIR"/build/outputs/aar/*.aar)
  shopt -u nullglob
  if [[ "${#aar_files[@]}" -eq 0 ]]; then
    log "no AAR files found under $ANDROID_DIR/build/outputs/aar; skipping AAR archive copy"
    return 0
  fi

  local aar
  for aar in "${aar_files[@]}"; do
    cp "$aar" "$AAR_OUTPUT_DIR/"
  done
}

if [[ "$SKIP_GRADLE" -eq 1 ]]; then
  log "skipping Gradle; staged Kotlin bindings and native libraries"
  write_android_manifest
  exit 0
fi

log "$GRADLE_TASK"
if [[ "$DRY_RUN" -eq 1 ]]; then
  log "dry-run: $GRADLE_TASK from $ANDROID_DIR"
elif [[ -x "$ANDROID_DIR/gradlew" ]]; then
  (cd "$ANDROID_DIR" && ./gradlew "$GRADLE_TASK")
elif command -v gradle >/dev/null 2>&1; then
  (cd "$ANDROID_DIR" && gradle "$GRADLE_TASK")
else
  log "Gradle is not installed and mobile/android/gradlew is missing; staged AAR inputs only"
fi

copy_android_aars
write_android_manifest
log "Android package staged at $ANDROID_DIR"
