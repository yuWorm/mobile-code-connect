#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/target/debug"
STATE_DIR="${MOBILECODE_CONNECT_TEST_STATE_DIR:-${QUIC_TEST_STATE_DIR:-${TMPDIR:-/tmp}/mobilecode-connect-dev-stack}}"
LOG_DIR="$STATE_DIR/logs"

TOKEN_SECRET="${MOBILECODE_CONNECT_TEST_TOKEN_SECRET:-${QUIC_TEST_TOKEN_SECRET:-dev-secret}}"
ADMIN_SUBJECT="${MOBILECODE_CONNECT_TEST_ADMIN_SUBJECT:-${QUIC_TEST_ADMIN_SUBJECT:-admin@example.com}}"
CONTROL_ADMIN_EMAIL="${MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_EMAIL:-${QUIC_TEST_CONTROL_ADMIN_EMAIL:-}}"
CONTROL_ADMIN_PASSWORD="${MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_PASSWORD:-${QUIC_TEST_CONTROL_ADMIN_PASSWORD:-}}"
CONTROL_ADMIN_DISPLAY_NAME="${MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_DISPLAY_NAME:-${QUIC_TEST_CONTROL_ADMIN_DISPLAY_NAME:-Admin}}"
RELAY_CONTROL_REGISTER="${MOBILECODE_CONNECT_TEST_RELAY_CONTROL_REGISTER:-${QUIC_TEST_RELAY_CONTROL_REGISTER:-0}}"
RELAY_ID="${MOBILECODE_CONNECT_TEST_RELAY_ID:-${QUIC_TEST_RELAY_ID:-relay_dev_001}}"
RELAY_CAPACITY_STREAMS="${MOBILECODE_CONNECT_TEST_RELAY_CAPACITY_STREAMS:-${QUIC_TEST_RELAY_CAPACITY_STREAMS:-128}}"
RELAY_HEARTBEAT_INTERVAL_SEC="${MOBILECODE_CONNECT_TEST_RELAY_HEARTBEAT_INTERVAL_SEC:-${QUIC_TEST_RELAY_HEARTBEAT_INTERVAL_SEC:-30}}"
DEVICE_ID="${MOBILECODE_CONNECT_TEST_DEVICE_ID:-${QUIC_TEST_DEVICE_ID:-pc_001}}"
CLIENT_ID="${MOBILECODE_CONNECT_TEST_CLIENT_ID:-${QUIC_TEST_CLIENT_ID:-mobile_001}}"
SERVICE_ID="${MOBILECODE_CONNECT_TEST_SERVICE_ID:-${QUIC_TEST_SERVICE_ID:-svc_echo}}"
HOST="${MOBILECODE_CONNECT_TEST_HOST:-${QUIC_TEST_HOST:-127.0.0.1}}"

CONTROL_PORT="${MOBILECODE_CONNECT_TEST_CONTROL_PORT:-${QUIC_TEST_CONTROL_PORT:-4242}}"
RELAY_PORT="${MOBILECODE_CONNECT_TEST_RELAY_PORT:-${QUIC_TEST_RELAY_PORT:-4443}}"
RELAY_ADMIN_PORT="${MOBILECODE_CONNECT_TEST_RELAY_ADMIN_PORT:-${QUIC_TEST_RELAY_ADMIN_PORT:-9090}}"
PUNCH_PORT="${MOBILECODE_CONNECT_TEST_PUNCH_PORT:-${QUIC_TEST_PUNCH_PORT:-3478}}"
ECHO_PORT="${MOBILECODE_CONNECT_TEST_ECHO_PORT:-${QUIC_TEST_ECHO_PORT:-13000}}"
LOCAL_PORT="${MOBILECODE_CONNECT_TEST_LOCAL_PORT:-${QUIC_TEST_LOCAL_PORT:-18080}}"
WAIT_TIMEOUT="${MOBILECODE_CONNECT_TEST_WAIT_TIMEOUT:-${QUIC_TEST_WAIT_TIMEOUT:-20}}"
TUNNEL_PATH="${MOBILECODE_CONNECT_TEST_PATH:-${QUIC_TEST_PATH:-p2p}}"

CONTROL_URL="http://$HOST:$CONTROL_PORT"
RELAY_ADDR="$HOST:$RELAY_PORT"
RELAY_ADMIN_ADDR="$HOST:$RELAY_ADMIN_PORT"
PUNCH_ADDR="$HOST:$PUNCH_PORT"
RELAY_CERT="$STATE_DIR/relay.der"
CONTROL_STATE_DB="${MOBILECODE_CONNECT_TEST_CONTROL_STATE_DB:-${QUIC_TEST_CONTROL_STATE_DB:-$STATE_DIR/control-state.sqlite}}"

log() {
  printf '[dev-stack] %s\n' "$*"
}

die() {
  log "$*"
  exit 1
}

usage() {
  cat <<EOF
Usage: ./scripts/dev-stack.sh <command>

Manual order:
  ./scripts/dev-stack.sh build
  ./scripts/dev-stack.sh start-echo
  ./scripts/dev-stack.sh start-relay
  ./scripts/dev-stack.sh start-punch
  ./scripts/dev-stack.sh start-control
  ./scripts/dev-stack.sh start-agent
  ./scripts/dev-stack.sh start-mobile
  ./scripts/dev-stack.sh check
  ./scripts/dev-stack.sh admin-token
  ./scripts/dev-stack.sh relay-token

Admin helpers against the running local Control server:
  ./scripts/dev-stack.sh admin-users [--limit 20 --offset 0 --q email]
  ./scripts/dev-stack.sh admin-usage [--sort actual_total_bytes --limit 20]
  ./scripts/dev-stack.sh admin-devices [--limit 20]
  ./scripts/dev-stack.sh admin-relays [--healthy true]
  ./scripts/dev-stack.sh admin-audit [--limit 20]
  ./scripts/dev-stack.sh admin-device-access --device pc_001 [--limit 20]
  ./scripts/dev-stack.sh admin-create-user --email user@example.com --password user-password-123 --name "Test User"
  ./scripts/dev-stack.sh admin-grant-device-access --device pc_001 --user-id user_123
  ./scripts/dev-stack.sh admin-revoke-device-access --device pc_001 --user-id user_123

Convenience:
  ./scripts/dev-stack.sh start-all
  ./scripts/dev-stack.sh run-all
  ./scripts/dev-stack.sh status
  ./scripts/dev-stack.sh stop
  ./scripts/dev-stack.sh clean

Ports can be overridden with MOBILECODE_CONNECT_TEST_* env vars, for example:
  MOBILECODE_CONNECT_TEST_LOCAL_PORT=18081 ./scripts/dev-stack.sh start-mobile
Legacy QUIC_TEST_* env vars are accepted as fallback aliases.

To force a Relay session for the admin panel:
  MOBILECODE_CONNECT_TEST_PATH=fallback ./scripts/dev-stack.sh start-all

To print a Control Admin token for the current dev secret:
  MOBILECODE_CONNECT_TEST_ADMIN_SUBJECT=admin@example.com ./scripts/dev-stack.sh admin-token

Admin helpers call mobile-cli admin with the local Control URL and an admin token:
  ./scripts/dev-stack.sh admin-users --limit 20
  ./scripts/dev-stack.sh admin-create-user --email user@example.com --password user-password-123 --name "Test User"
  wraps: mobile-cli admin users, mobile-cli admin usage, mobile-cli admin device-access, mobile-cli admin create-user, mobile-cli admin grant-device-access

To print a Relay registration token for the current dev relay id:
  MOBILECODE_CONNECT_TEST_RELAY_ID=relay_dev_001 ./scripts/dev-stack.sh relay-token

To bootstrap a persistent Control Admin user on startup:
  MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_EMAIL=admin@example.com \\
  MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_PASSWORD=admin-password-123 \\
  ./scripts/dev-stack.sh start-control

To make relayd register itself into Control and report session usage:
  MOBILECODE_CONNECT_TEST_RELAY_CONTROL_REGISTER=1 ./scripts/dev-stack.sh start-all

Logs and pid files:
  $STATE_DIR
EOF
}

validate_path() {
  case "$TUNNEL_PATH" in
    p2p | fallback) ;;
    *) die "MOBILECODE_CONNECT_TEST_PATH must be p2p or fallback" ;;
  esac
}

relay_control_register_enabled() {
  [[ "$RELAY_CONTROL_REGISTER" == "1" || "$RELAY_CONTROL_REGISTER" == "true" ]]
}

mkdir_state() {
  mkdir -p "$STATE_DIR" "$LOG_DIR"
}

pid_path() {
  printf '%s/%s.pid\n' "$STATE_DIR" "$1"
}

log_path() {
  printf '%s/%s.log\n' "$LOG_DIR" "$1"
}

is_running() {
  local name="$1"
  local pid_file pid
  pid_file="$(pid_path "$name")"
  [[ -f "$pid_file" ]] || return 1
  pid="$(cat "$pid_file")"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

tail_log() {
  local name="$1"
  local log_file
  log_file="$(log_path "$name")"
  [[ -f "$log_file" ]] || return 0
  log "last log lines for $name:"
  tail -n 40 "$log_file" || true
}

ensure_binaries() {
  local missing=()
  for bin in relayd punch-server control-server agentd mobile-cli; do
    if [[ ! -x "$BIN_DIR/$bin" ]]; then
      missing+=("$bin")
    fi
  done

  if [[ "${#missing[@]}" -gt 0 ]]; then
    die "missing binaries (${missing[*]}). Run: ./scripts/dev-stack.sh build"
  fi
}

ensure_running() {
  local name="$1"
  if ! is_running "$name"; then
    tail_log "$name"
    die "$name is not running"
  fi
}

wait_for_tcp() {
  local host="$1"
  local port="$2"
  local label="$3"
  local deadline=$((SECONDS + WAIT_TIMEOUT))

  until python3 - "$host" "$port" >/dev/null 2>&1 <<'PY'
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.settimeout(0.2)
    try:
        sock.connect((host, port))
    except OSError:
        raise SystemExit(1)
PY
  do
    if (( SECONDS >= deadline )); then
      die "timed out waiting for $label on $host:$port"
    fi
    sleep 0.1
  done
}

wait_for_file() {
  local path="$1"
  local label="$2"
  local deadline=$((SECONDS + WAIT_TIMEOUT))

  until [[ -s "$path" ]]; do
    if (( SECONDS >= deadline )); then
      die "timed out waiting for $label at $path"
    fi
    sleep 0.1
  done
}

wait_for_log() {
  local name="$1"
  local pattern="$2"
  local label="$3"
  local path
  local deadline=$((SECONDS + WAIT_TIMEOUT))
  path="$(log_path "$name")"

  until grep -q "$pattern" "$path" 2>/dev/null; do
    ensure_running "$name"
    if (( SECONDS >= deadline )); then
      tail_log "$name"
      die "timed out waiting for $label"
    fi
    sleep 0.1
  done
}

start_bg() {
  local name="$1"
  shift
  mkdir_state

  if is_running "$name"; then
    log "$name already running (pid $(cat "$(pid_path "$name")"))"
    return 0
  fi

  rm -f "$(pid_path "$name")"
  local log_file
  log_file="$(log_path "$name")"
  log "starting $name, log: $log_file"
  nohup "$@" >"$log_file" 2>&1 &
  printf '%s\n' "$!" >"$(pid_path "$name")"
  sleep 0.1
  ensure_running "$name"
}

build() {
  mkdir_state
  validate_path
  log "building workspace binaries"
  cargo build --workspace --bins
}

start_echo() {
  mkdir_state
  if is_running echo; then
    log "echo already running (pid $(cat "$(pid_path echo)"))"
    return 0
  fi

  rm -f "$(pid_path echo)"
  local log_file
  log_file="$(log_path echo)"
  log "starting echo service on $HOST:$ECHO_PORT, log: $log_file"
  nohup python3 -u - "$HOST" "$ECHO_PORT" >"$log_file" 2>&1 <<'PY' &
import socketserver
import sys

host, port = sys.argv[1], int(sys.argv[2])

class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        data = self.request.recv(4096)
        if data == b"hello":
            self.request.sendall(b"world")
        elif data.startswith(b"GET "):
            body = b"mobilecode-connect-forward-ok\n"
            response = (
                b"HTTP/1.1 200 OK\r\n"
                b"Content-Type: text/plain; charset=utf-8\r\n"
                b"Content-Length: " + str(len(body)).encode("ascii") + b"\r\n"
                b"Connection: close\r\n"
                b"\r\n" + body
            )
            self.request.sendall(response)
        elif data:
            self.request.sendall(data)

class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True

with Server((host, port), Handler) as server:
    print(f"echo listening on {host}:{port}", flush=True)
    server.serve_forever()
PY
  printf '%s\n' "$!" >"$(pid_path echo)"
  sleep 0.1
  ensure_running echo
  wait_for_tcp "$HOST" "$ECHO_PORT" echo
}

start_relay() {
  ensure_binaries
  mkdir_state
  local relay_args=(
    "$BIN_DIR/relayd"
    --bind "$RELAY_ADDR"
    --token-secret "$TOKEN_SECRET"
    --debug-admin-listen "$RELAY_ADMIN_ADDR"
    --cert-out "$RELAY_CERT"
  )
  if relay_control_register_enabled; then
    local relay_token
    relay_token="$(print_relay_token)"
    relay_args+=(
      --control-url "$CONTROL_URL"
      --control-token "$relay_token"
      --relay-id "$RELAY_ID"
      --advertise-addr "$RELAY_ADDR"
      --capacity-streams "$RELAY_CAPACITY_STREAMS"
      --heartbeat-interval-sec "$RELAY_HEARTBEAT_INTERVAL_SEC"
    )
  fi
  start_bg relay "${relay_args[@]}"
  wait_for_tcp "$HOST" "$RELAY_ADMIN_PORT" relay-debug-admin
  wait_for_file "$RELAY_CERT" "relay certificate"
}

start_punch() {
  ensure_binaries
  start_bg punch "$BIN_DIR/punch-server" --bind "$PUNCH_ADDR"
  sleep 0.2
  ensure_running punch
}

start_control() {
  ensure_binaries
  if ! relay_control_register_enabled; then
    wait_for_tcp "$HOST" "$RELAY_ADMIN_PORT" relay-debug-admin
  fi
  local control_args=(
    "$BIN_DIR/control-server" \
      --listen "$HOST:$CONTROL_PORT" \
      --token-secret "$TOKEN_SECRET" \
      --relay-addr "$RELAY_ADDR" \
      --punch-addr "$PUNCH_ADDR" \
      --state-db "$CONTROL_STATE_DB"
  )
  if [[ -n "$CONTROL_ADMIN_EMAIL" || -n "$CONTROL_ADMIN_PASSWORD" ]]; then
    [[ -n "$CONTROL_ADMIN_EMAIL" && -n "$CONTROL_ADMIN_PASSWORD" ]] || \
      die "MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_EMAIL and MOBILECODE_CONNECT_TEST_CONTROL_ADMIN_PASSWORD must be set together"
    control_args+=(
      --bootstrap-admin-email "$CONTROL_ADMIN_EMAIL"
      --bootstrap-admin-password "$CONTROL_ADMIN_PASSWORD"
      --bootstrap-admin-display-name "$CONTROL_ADMIN_DISPLAY_NAME"
    )
  fi
  start_bg control "${control_args[@]}"
  wait_for_tcp "$HOST" "$CONTROL_PORT" control-server
}

print_admin_token() {
  ensure_binaries
  "$BIN_DIR/control-server" \
    --token-secret "$TOKEN_SECRET" \
    --relay-addr "$RELAY_ADDR" \
    --punch-addr "$PUNCH_ADDR" \
    --print-admin-token "$ADMIN_SUBJECT"
}

print_relay_token() {
  ensure_binaries
  "$BIN_DIR/control-server" \
    --token-secret "$TOKEN_SECRET" \
    --relay-addr "$RELAY_ADDR" \
    --punch-addr "$PUNCH_ADDR" \
    --print-relay-token "$RELAY_ID"
}

admin_token_value() {
  if [[ -n "${MOBILECODE_CONNECT_TEST_ADMIN_TOKEN:-${QUIC_TEST_ADMIN_TOKEN:-}}" ]]; then
    printf '%s\n' "${MOBILECODE_CONNECT_TEST_ADMIN_TOKEN:-${QUIC_TEST_ADMIN_TOKEN:-}}"
    return 0
  fi
  print_admin_token
}

admin_cli() {
  local admin_command="$1"
  shift

  ensure_binaries
  wait_for_tcp "$HOST" "$CONTROL_PORT" control-server
  "$BIN_DIR/mobile-cli" admin "$admin_command" \
    --control "$CONTROL_URL" \
    --token "$(admin_token_value)" \
    "$@"
}

start_agent() {
  ensure_binaries
  validate_path
  wait_for_tcp "$HOST" "$CONTROL_PORT" control-server
  wait_for_tcp "$HOST" "$ECHO_PORT" echo
  wait_for_file "$RELAY_CERT" "relay certificate"
  local agent_args=(
    "$BIN_DIR/agentd" \
      --relay-cert "$RELAY_CERT" \
      --service "$SERVICE_ID=$HOST:$ECHO_PORT" \
      --control "$CONTROL_URL" \
      --device "$DEVICE_ID" \
      --agent-token agent-token \
      --poll-ms 20
  )
  if [[ "$TUNNEL_PATH" == "p2p" ]]; then
    agent_args+=(
      --p2p-identity-dir "$STATE_DIR/agent-identity"
      --p2p-bind "$HOST:0"
      --p2p-candidate-timeout-ms 1000
      --p2p-probe-timeout-ms 1000
      --p2p-interval-ms 10
    )
  fi
  start_bg agent "${agent_args[@]}"
  wait_for_log agent "agentd polling control" agent
}

start_mobile() {
  ensure_binaries
  validate_path
  wait_for_tcp "$HOST" "$CONTROL_PORT" control-server
  wait_for_file "$RELAY_CERT" "relay certificate"
  start_bg mobile \
    "$BIN_DIR/mobile-cli" open-service \
      --control "$CONTROL_URL" \
      --token user-token \
      --client "$CLIENT_ID" \
      --device "$DEVICE_ID" \
      --service "$SERVICE_ID" \
      --local "$LOCAL_PORT" \
      --relay-cert "$RELAY_CERT" \
      --p2p-bind "$HOST:0" \
      --p2p-candidate-timeout-ms 1000 \
      --p2p-probe-timeout-ms 1000 \
      --p2p-interval-ms 10 \
      --relay-fallback-delay-ms 50
  wait_for_log mobile "mobile-cli forwarding" mobile-cli
  wait_for_tcp "$HOST" "$LOCAL_PORT" mobile-forward
}

check() {
  wait_for_tcp "$HOST" "$LOCAL_PORT" mobile-forward
  local response
  response="$(python3 - "$HOST" "$LOCAL_PORT" <<'PY'
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
with socket.create_connection((host, port), timeout=5) as sock:
    sock.sendall(b"hello")
    data = sock.recv(5)
print(data.decode("ascii"))
PY
)"

  if [[ "$response" != "world" ]]; then
    die "unexpected response from forwarded service: $response"
  fi

  log "forward check passed: $HOST:$LOCAL_PORT -> $SERVICE_ID -> world"
  assert_http_forward_response
  if [[ "$TUNNEL_PATH" == "fallback" ]]; then
    assert_admin_session_visible
  fi
}

assert_http_forward_response() {
  local response

  response="$(python3 - "$HOST" "$LOCAL_PORT" <<'PY'
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
with socket.create_connection((host, port), timeout=5) as sock:
    sock.sendall(
        b"GET / HTTP/1.1\r\n"
        b"Host: 127.0.0.1\r\n"
        b"Connection: close\r\n"
        b"\r\n"
    )
    chunks = []
    while True:
        data = sock.recv(4096)
        if not data:
            break
        chunks.append(data)
print(b"".join(chunks).decode("utf-8", errors="replace"))
PY
)"

  if [[ "$response" != *"HTTP/1.1 200 OK"* || "$response" != *"mobilecode-connect-forward-ok"* ]]; then
    die "unexpected HTTP response from forwarded service: $response"
  fi

  log "HTTP forward check passed: http://$HOST:$LOCAL_PORT -> mobilecode-connect-forward-ok"
}

assert_admin_session_visible() {
  python3 - "$HOST" "$RELAY_ADMIN_PORT" <<'PY'
import json
import sys
import time
import urllib.request

host, port = sys.argv[1], int(sys.argv[2])
deadline = time.monotonic() + 5
last_sessions = []

while time.monotonic() < deadline:
    with urllib.request.urlopen(f"http://{host}:{port}/admin/sessions", timeout=5) as response:
        last_sessions = json.load(response)

    bound = [
        session for session in last_sessions
        if session.get("mobile_bound") and session.get("agent_bound")
    ]
    if any(int(session.get("stats", {}).get("total_bytes", 0)) > 0 for session in bound):
        raise SystemExit(0)
    time.sleep(0.1)

if not last_sessions:
    raise SystemExit("expected at least one Relay admin session")
raise SystemExit(f"expected Relay admin traffic stats, got {last_sessions!r}")
PY

  log "Relay Debug Admin check passed: http://$RELAY_ADMIN_ADDR/admin/sessions"
}

start_all() {
  build
  start_echo
  if relay_control_register_enabled; then
    start_punch
    start_control
    start_relay
  else
    start_relay
    start_punch
    start_control
  fi
  start_agent
  start_mobile
  check
}

status() {
  mkdir_state
  for name in echo relay punch control agent mobile; do
    if is_running "$name"; then
      log "$name running (pid $(cat "$(pid_path "$name")"))"
    else
      log "$name stopped"
    fi
  done
  log "control: $CONTROL_URL"
  log "Control Admin: $CONTROL_URL/admin"
  log "relay: $RELAY_ADDR"
  log "Relay Debug Admin: http://$RELAY_ADMIN_ADDR/admin"
  log "punch: $PUNCH_ADDR"
  log "local forward: $HOST:$LOCAL_PORT"
  log "control state db: $CONTROL_STATE_DB"
  if [[ -n "$CONTROL_ADMIN_EMAIL" ]]; then
    log "control admin user: $CONTROL_ADMIN_EMAIL"
  fi
  log "relay control register: $RELAY_CONTROL_REGISTER"
  log "relay heartbeat interval: ${RELAY_HEARTBEAT_INTERVAL_SEC}s"
  log "path: $TUNNEL_PATH"
  log "logs: $LOG_DIR"
}

stop_one() {
  local name="$1"
  local pid_file pid
  pid_file="$(pid_path "$name")"
  [[ -f "$pid_file" ]] || return 0
  pid="$(cat "$pid_file")"
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
    log "stopping $name (pid $pid)"
    kill "$pid" 2>/dev/null || true
    for _ in {1..50}; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$pid_file"
}

stop_port_owner_if_expected() {
  local name="$1"
  local port="$2"
  local expected_prefix="$3"

  command -v lsof >/dev/null 2>&1 || return 0

  local proc_name pid
  while read -r proc_name pid; do
    [[ -n "${proc_name:-}" && -n "${pid:-}" ]] || continue
    [[ "$pid" =~ ^[0-9]+$ ]] || continue

    if [[ "$proc_name" != "$expected_prefix"* ]]; then
      log "leaving $name port owner alone on $HOST:$port (pid $pid, command $proc_name)"
      continue
    fi

    log "stopping stale $name on $HOST:$port (pid $pid, command $proc_name)"
    kill "$pid" 2>/dev/null || true
    for _ in {1..50}; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  done < <(lsof -nP -iTCP@"$HOST":"$port" -sTCP:LISTEN 2>/dev/null | awk 'NR > 1 {print $1, $2}')
}

stop() {
  mkdir_state
  for name in mobile agent control punch relay echo; do
    stop_one "$name"
  done
  stop_port_owner_if_expected mobile "$LOCAL_PORT" mobile-cl
  stop_port_owner_if_expected control "$CONTROL_PORT" control-
  stop_port_owner_if_expected relay-debug-admin "$RELAY_ADMIN_PORT" relayd
}

run_all() {
  trap 'trap - EXIT; stop; exit 0' INT TERM
  trap stop EXIT

  start_all
  log "stack ready"
  log "Control Admin: $CONTROL_URL/admin"
  log "Relay Debug Admin: http://$RELAY_ADMIN_ADDR/admin"
  log "forwarded HTTP service: http://$HOST:$LOCAL_PORT"
  log "admin token: ./scripts/dev-stack.sh admin-token"
  log "admin users: ./scripts/dev-stack.sh admin-users --limit 20"
  log "admin usage: ./scripts/dev-stack.sh admin-usage --sort actual_total_bytes --limit 20"
  log "relay token: ./scripts/dev-stack.sh relay-token"
  log "press Ctrl-C to stop"

  while true; do
    sleep 3600
  done
}

clean() {
  stop
  rm -rf "$STATE_DIR"
  log "removed $STATE_DIR"
}

case "${1:-}" in
  build) build ;;
  start-echo) start_echo ;;
  start-relay) start_relay ;;
  start-punch) start_punch ;;
  start-control) start_control ;;
  start-agent) start_agent ;;
  start-mobile) start_mobile ;;
  admin-token) print_admin_token ;;
  relay-token) print_relay_token ;;
  admin-users) shift; admin_cli users "$@" ;;
  admin-usage) shift; admin_cli usage "$@" ;;
  admin-devices) shift; admin_cli devices "$@" ;;
  admin-relays) shift; admin_cli relays "$@" ;;
  admin-audit) shift; admin_cli audit "$@" ;;
  admin-device-access) shift; admin_cli device-access "$@" ;;
  admin-create-user) shift; admin_cli create-user "$@" ;;
  admin-grant-device-access) shift; admin_cli grant-device-access "$@" ;;
  admin-revoke-device-access) shift; admin_cli revoke-device-access "$@" ;;
  start-all) start_all ;;
  run-all) run_all ;;
  check) check ;;
  status) status ;;
  stop) stop ;;
  clean) clean ;;
  "" | help | -h | --help) usage ;;
  *) usage; exit 2 ;;
esac
