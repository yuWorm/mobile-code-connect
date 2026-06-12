#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[install-relayd] %s\n' "$*"
}

die() {
  log "$*"
  exit 1
}

control_url=""
bootstrap_id=""
bootstrap_token=""
relayd_bin=""
relayd_url=""
relayd_sha256=""
install_dir="/usr/local/bin"
install_dir_default=1
systemd_dir="/etc/systemd/system"
env_file="/etc/quic-tunnel/relayd.env"
env_file_default=1
service_name="quic-tunnel-relayd"
bind_addr="0.0.0.0:4443"
debug_admin_listen=""
dry_run=0
no_service=0

shell_quote() {
  local value="$1"
  printf "'%s'" "${value//\'/\'\\\'\'}"
}

is_root() {
  [[ "$(id -u 2>/dev/null || printf '1')" == "0" ]]
}

normalize_url() {
  local value="${1%/}"
  case "$value" in
    http://*|https://*) printf '%s' "$value" ;;
    *) printf 'http://%s' "$value" ;;
  esac
}

print_manual_relayd_command() {
  local quoted_env_file
  quoted_env_file="$(shell_quote "$env_file")"
  if [[ -n "$debug_admin_listen" ]]; then
    printf "env QUIC_TUNNEL_RELAY_ENV=%s sh -c 'set -a; . \"\$QUIC_TUNNEL_RELAY_ENV\"; set +a; exec \"\$QUIC_TUNNEL_RELAY_BIN\" --bind \"\$QUIC_TUNNEL_RELAY_BIND\" --debug-admin-listen \"\$QUIC_TUNNEL_RELAY_DEBUG_ADMIN_LISTEN\" --token-secret \"\$QUIC_TUNNEL_RELAY_TOKEN_SECRET\" --control-url \"\$QUIC_TUNNEL_RELAY_CONTROL_URL\" --control-token \"\$QUIC_TUNNEL_RELAY_CONTROL_TOKEN\" --relay-id \"\$QUIC_TUNNEL_RELAY_ID\" --advertise-addr \"\$QUIC_TUNNEL_RELAY_ADVERTISE_ADDR\" --capacity-streams \"\$QUIC_TUNNEL_RELAY_CAPACITY_STREAMS\" --heartbeat-interval-sec \"\$QUIC_TUNNEL_RELAY_HEARTBEAT_INTERVAL_SEC\"'\n" "$quoted_env_file"
  else
    printf "env QUIC_TUNNEL_RELAY_ENV=%s sh -c 'set -a; . \"\$QUIC_TUNNEL_RELAY_ENV\"; set +a; exec \"\$QUIC_TUNNEL_RELAY_BIN\" --bind \"\$QUIC_TUNNEL_RELAY_BIND\" --token-secret \"\$QUIC_TUNNEL_RELAY_TOKEN_SECRET\" --control-url \"\$QUIC_TUNNEL_RELAY_CONTROL_URL\" --control-token \"\$QUIC_TUNNEL_RELAY_CONTROL_TOKEN\" --relay-id \"\$QUIC_TUNNEL_RELAY_ID\" --advertise-addr \"\$QUIC_TUNNEL_RELAY_ADVERTISE_ADDR\" --capacity-streams \"\$QUIC_TUNNEL_RELAY_CAPACITY_STREAMS\" --heartbeat-interval-sec \"\$QUIC_TUNNEL_RELAY_HEARTBEAT_INTERVAL_SEC\"'\n" "$quoted_env_file"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --control-url)
      control_url="${2:-}"
      shift 2
      ;;
    --bootstrap-id)
      bootstrap_id="${2:-}"
      shift 2
      ;;
    --bootstrap-token)
      bootstrap_token="${2:-}"
      shift 2
      ;;
    --relayd-bin)
      relayd_bin="${2:-}"
      shift 2
      ;;
    --relayd-url)
      relayd_url="${2:-}"
      shift 2
      ;;
    --relayd-sha256)
      relayd_sha256="${2:-}"
      shift 2
      ;;
    --install-dir)
      install_dir="${2:-}"
      install_dir_default=0
      shift 2
      ;;
    --systemd-dir)
      systemd_dir="${2:-}"
      shift 2
      ;;
    --env-file)
      env_file="${2:-}"
      env_file_default=0
      shift 2
      ;;
    --service-name)
      service_name="${2:-}"
      shift 2
      ;;
    --bind)
      bind_addr="${2:-}"
      shift 2
      ;;
    --debug-admin-listen)
      debug_admin_listen="${2:-}"
      shift 2
      ;;
    --admin-listen)
      debug_admin_listen="${2:-}"
      shift 2
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --no-service)
      no_service=1
      shift
      ;;
    -h|--help)
      cat <<'USAGE'
Usage:
  install-relayd.sh --control-url URL --bootstrap-id ID --bootstrap-token TOKEN [options]

Options:
  --relayd-bin PATH       Copy this local relayd binary into --install-dir.
  --relayd-url URL        Download relayd from this URL when --relayd-bin is not provided.
  --relayd-sha256 HEX     Optional SHA-256 checksum for --relayd-url.
  --install-dir DIR       Binary install directory. Default: /usr/local/bin
  --systemd-dir DIR       systemd unit directory. Default: /etc/systemd/system
  --env-file PATH         Environment file path. Default: /etc/quic-tunnel/relayd.env
  --service-name NAME     systemd service name. Default: quic-tunnel-relayd
  --bind ADDR             Relay listen address. Default: 0.0.0.0:4443
  --debug-admin-listen ADDR
                          Optional local Relay admin debug listener. Disabled by default.
  --dry-run               Print planned actions without writing files.
  --no-service            Exchange bootstrap and print a manual relayd command instead of installing systemd service.
USAGE
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

[[ -n "$control_url" ]] || die "--control-url is required"
[[ -n "$bootstrap_id" ]] || die "--bootstrap-id is required"
[[ -n "$bootstrap_token" ]] || die "--bootstrap-token is required"

control_url="$(normalize_url "$control_url")"
if [[ -n "$relayd_url" ]]; then
  relayd_url="$(normalize_url "$relayd_url")"
fi

if [[ "$no_service" == "1" ]] && ! is_root; then
  if [[ "$install_dir_default" == "1" ]]; then
    if [[ -n "${HOME:-}" ]]; then
      install_dir="$HOME/.local/bin"
    else
      install_dir=".quic-tunnel/bin"
    fi
  fi
  if [[ "$env_file_default" == "1" ]]; then
    if [[ -n "${HOME:-}" ]]; then
      env_file="${XDG_CONFIG_HOME:-$HOME/.config}/quic-tunnel/relayd.env"
    else
      env_file=".quic-tunnel/relayd.env"
    fi
  fi
fi

relayd_path="$install_dir/relayd"
unit_file="$systemd_dir/$service_name.service"

if [[ "$dry_run" == "1" ]]; then
  log "dry-run: exchange POST ${control_url%/}/relay-bootstraps/$bootstrap_id/exchange"
  if [[ -n "$relayd_bin" ]]; then
    log "dry-run: install $relayd_bin to $relayd_path"
  elif [[ -n "$relayd_url" ]]; then
    log "dry-run: download relayd from $relayd_url to $relayd_path"
    if [[ -n "$relayd_sha256" ]]; then
      log "dry-run: verify relayd sha256 $relayd_sha256"
    fi
  else
    log "dry-run: use existing relayd at $relayd_path or PATH"
  fi
  log "dry-run: write resolved relay credentials to $env_file"
  if [[ -n "$debug_admin_listen" ]]; then
    log "dry-run: enable local relay debug admin listener $debug_admin_listen"
  fi
  if [[ "$no_service" == "1" ]]; then
    log "dry-run: no-service mode; skip systemd unit and systemctl"
    log "dry-run: manual relayd command would be:"
    print_manual_relayd_command
    exit 0
  fi
  log "dry-run: write systemd unit $unit_file"
  log "dry-run: systemctl daemon-reload"
  log "dry-run: systemctl enable --now $service_name"
  exit 0
fi

command -v python3 >/dev/null 2>&1 || die "python3 is required for bootstrap exchange"

download_relayd_binary() {
  install -d "$install_dir"
  local tmp_path="$relayd_path.download.$$"
  python3 - "$relayd_url" "$tmp_path" "$relayd_sha256" <<'PY'
import hashlib
import os
import sys
import urllib.error
import urllib.request

url, target, expected_sha256 = sys.argv[1:4]
request = urllib.request.Request(url, method="GET")
try:
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read()
except urllib.error.HTTPError as error:
    sys.stderr.write(error.read().decode("utf-8", errors="replace"))
    raise

if expected_sha256:
    actual_sha256 = hashlib.sha256(data).hexdigest()
    if actual_sha256.lower() != expected_sha256.lower():
        raise SystemExit(
            f"relayd sha256 mismatch: expected {expected_sha256}, got {actual_sha256}"
        )

with open(target, "wb") as file:
    file.write(data)
PY
  chmod 0755 "$tmp_path"
  mv "$tmp_path" "$relayd_path"
  chmod 0755 "$relayd_path"
}

if [[ -n "$relayd_bin" ]]; then
  [[ -f "$relayd_bin" ]] || die "relayd binary not found: $relayd_bin"
  install -d "$install_dir"
  install -m 0755 "$relayd_bin" "$relayd_path"
elif [[ -n "$relayd_url" ]]; then
  download_relayd_binary
elif command -v relayd >/dev/null 2>&1; then
  relayd_path="$(command -v relayd)"
elif [[ ! -x "$relayd_path" ]]; then
  die "relayd binary is required; pass --relayd-bin, pass --relayd-url, or install relayd in PATH"
fi

exchange_json="$(
  python3 - "$control_url" "$bootstrap_id" "$bootstrap_token" <<'PY'
import json
import sys
import urllib.error
import urllib.parse
import urllib.request

control_url, bootstrap_id, bootstrap_token = sys.argv[1:4]
url = control_url.rstrip("/") + "/relay-bootstraps/" + urllib.parse.quote(bootstrap_id) + "/exchange"
body = json.dumps({"bootstrap_token": bootstrap_token}).encode("utf-8")
request = urllib.request.Request(
    url,
    data=body,
    headers={"content-type": "application/json"},
    method="POST",
)
try:
    with urllib.request.urlopen(request, timeout=30) as response:
        sys.stdout.write(response.read().decode("utf-8"))
except urllib.error.HTTPError as error:
    sys.stderr.write(error.read().decode("utf-8", errors="replace"))
    raise
PY
)"

if [[ "$no_service" == "1" ]]; then
  install -d "$(dirname "$env_file")"
else
  install -d "$(dirname "$env_file")" "$systemd_dir"
fi
EXCHANGE_JSON="$exchange_json" RELAYD_PATH="$relayd_path" BIND_ADDR="$bind_addr" DEBUG_ADMIN_LISTEN="$debug_admin_listen" \
  python3 - "$env_file" <<'PY'
import json
import os
import shlex
import sys

target = sys.argv[1]
data = json.loads(os.environ["EXCHANGE_JSON"])

def normalize_control_url(value):
    value = value.rstrip("/")
    if "://" in value:
        return value
    return "http://" + value

values = {
    "QUIC_TUNNEL_RELAY_BIN": os.environ["RELAYD_PATH"],
    "QUIC_TUNNEL_RELAY_BIND": os.environ["BIND_ADDR"],
    "QUIC_TUNNEL_RELAY_TOKEN_SECRET": data["token_secret"],
    "QUIC_TUNNEL_RELAY_CONTROL_URL": normalize_control_url(data["control_url"]),
    "QUIC_TUNNEL_RELAY_CONTROL_TOKEN": data["control_token"],
    "QUIC_TUNNEL_RELAY_ID": data["relay_id"],
    "QUIC_TUNNEL_RELAY_ADVERTISE_ADDR": data["relay_addr"],
    "QUIC_TUNNEL_RELAY_CAPACITY_STREAMS": str(data["capacity_streams"]),
    "QUIC_TUNNEL_RELAY_HEARTBEAT_INTERVAL_SEC": str(data["heartbeat_interval_sec"]),
}
if os.environ["DEBUG_ADMIN_LISTEN"]:
    values["QUIC_TUNNEL_RELAY_DEBUG_ADMIN_LISTEN"] = os.environ["DEBUG_ADMIN_LISTEN"]
with open(target, "w", encoding="utf-8") as file:
    for key, value in values.items():
        file.write(f"{key}={shlex.quote(value)}\n")
PY
chmod 0600 "$env_file"

if [[ "$no_service" == "1" ]]; then
  log "relay credentials written to $env_file"
  log "no-service mode: systemd install skipped"
  log "manual relayd command (run with sudo if $env_file is root-owned):"
  print_manual_relayd_command
  exit 0
fi

cat >"$unit_file" <<UNIT
[Unit]
Description=QUIC Tunnel Relay
After=network-online.target
Wants=network-online.target

[Service]
EnvironmentFile=$env_file
ExecStart=/bin/sh -c 'set -- "\$QUIC_TUNNEL_RELAY_BIN" --bind "\$QUIC_TUNNEL_RELAY_BIND" --token-secret "\$QUIC_TUNNEL_RELAY_TOKEN_SECRET" --control-url "\$QUIC_TUNNEL_RELAY_CONTROL_URL" --control-token "\$QUIC_TUNNEL_RELAY_CONTROL_TOKEN" --relay-id "\$QUIC_TUNNEL_RELAY_ID" --advertise-addr "\$QUIC_TUNNEL_RELAY_ADVERTISE_ADDR" --capacity-streams "\$QUIC_TUNNEL_RELAY_CAPACITY_STREAMS" --heartbeat-interval-sec "\$QUIC_TUNNEL_RELAY_HEARTBEAT_INTERVAL_SEC"; if [ -n "\${QUIC_TUNNEL_RELAY_DEBUG_ADMIN_LISTEN:-}" ]; then set -- "\$@" --debug-admin-listen "\$QUIC_TUNNEL_RELAY_DEBUG_ADMIN_LISTEN"; fi; exec "\$@"'
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable --now "$service_name"
log "relay service started: $service_name"
