#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT_DIR/target/debug"
TMP_ROOT="${TMPDIR:-/tmp}/quic-tunnel-smoke-$$"
TOKEN_SECRET="dev-secret"
DEVICE_ID="pc_001"
CLIENT_ID="mobile_001"
SERVICE_ID="svc_web_3000"

PIDS=()

log() {
  printf '[smoke] %s\n' "$*"
}

free_port() {
  python3 - "$@" <<'PY'
import socket
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
}

wait_for_tcp() {
  local host="$1"
  local port="$2"
  local label="$3"
  local deadline=$((SECONDS + 20))

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
      log "timed out waiting for $label on $host:$port"
      return 1
    fi
    sleep 0.1
  done
}

wait_for_file() {
  local path="$1"
  local label="$2"
  local deadline=$((SECONDS + 20))

  until [[ -s "$path" ]]; do
    if (( SECONDS >= deadline )); then
      log "timed out waiting for $label at $path"
      return 1
    fi
    sleep 0.1
  done
}

wait_for_log() {
  local path="$1"
  local pattern="$2"
  local label="$3"
  local deadline=$((SECONDS + 20))

  until grep -q "$pattern" "$path" 2>/dev/null; do
    if (( SECONDS >= deadline )); then
      log "timed out waiting for $label"
      return 1
    fi
    sleep 0.1
  done
}

start_bg() {
  local log_file="$1"
  shift

  "$@" >"$log_file" 2>&1 &
  local pid=$!
  PIDS+=("$pid")
  printf '%s\n' "$pid"
}

cleanup() {
  for pid in "${PIDS[@]:-}"; do
    [[ -n "$pid" ]] || continue
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
  if [[ "${#PIDS[@]}" -gt 0 ]]; then
    wait "${PIDS[@]}" 2>/dev/null || true
  fi
  PIDS=()
}

start_echo_server() {
  local port="$1"
  local log_file="$2"

  python3 -u - "$port" >"$log_file" 2>&1 <<'PY' &
import socketserver
import sys

port = int(sys.argv[1])

class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        data = self.request.recv(4096)
        if data == b"hello":
            self.request.sendall(b"world")
        elif data.startswith(b"GET "):
            body = b"quic-test-forward-ok\n"
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

with Server(("127.0.0.1", port), Handler) as server:
    print(f"echo listening on 127.0.0.1:{port}", flush=True)
    server.serve_forever()
PY
  local pid=$!
  PIDS+=("$pid")
  printf '%s\n' "$pid"
}

assert_forward_response() {
  local port="$1"
  local response

  response="$(python3 - "$port" <<'PY'
import socket
import sys

port = int(sys.argv[1])
with socket.create_connection(("127.0.0.1", port), timeout=5) as sock:
    sock.sendall(b"hello")
    data = sock.recv(5)
print(data.decode("ascii"))
PY
)"

  if [[ "$response" != "world" ]]; then
    log "unexpected response from forwarded service: $response"
    return 1
  fi
}

assert_http_forward_response() {
  local port="$1"
  local response

  response="$(python3 - "$port" <<'PY'
import socket
import sys

port = int(sys.argv[1])
with socket.create_connection(("127.0.0.1", port), timeout=5) as sock:
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

  if [[ "$response" != *"HTTP/1.1 200 OK"* || "$response" != *"quic-test-forward-ok"* ]]; then
    log "unexpected HTTP response from forwarded service: $response"
    return 1
  fi
}

assert_admin_session_visible() {
  local relay_admin_port="$1"

  python3 - "$relay_admin_port" <<'PY'
import json
import sys
import time
import urllib.request

port = int(sys.argv[1])
deadline = time.monotonic() + 5
last_sessions = []

while time.monotonic() < deadline:
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/admin/sessions", timeout=5) as response:
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
}

run_case() {
  local mode="$1"
  local work_dir="$TMP_ROOT/$mode"
  mkdir -p "$work_dir"
  PIDS=()

  local control_port relay_port relay_admin_port punch_port echo_port local_port
  control_port="$(free_port)"
  relay_port="$(free_port)"
  relay_admin_port="$(free_port)"
  punch_port="$(free_port)"
  echo_port="$(free_port)"
  local_port="$(free_port)"

  local relay_cert="$work_dir/relay.der"
  local control_url="http://127.0.0.1:$control_port"
  local relay_addr="127.0.0.1:$relay_port"
  local punch_addr="127.0.0.1:$punch_port"
  local control_state_db="$work_dir/control-state.sqlite"

  log "running $mode case"

  start_bg "$work_dir/relayd.log" \
    "$BIN_DIR/relayd" \
      --bind "$relay_addr" \
      --token-secret "$TOKEN_SECRET" \
      --debug-admin-listen "127.0.0.1:$relay_admin_port" \
      --cert-out "$relay_cert" >/dev/null
  wait_for_tcp 127.0.0.1 "$relay_admin_port" relayd
  wait_for_file "$relay_cert" "relay certificate"

  start_bg "$work_dir/punch-server.log" \
    "$BIN_DIR/punch-server" \
      --bind "$punch_addr" >/dev/null
  sleep 0.2

  start_bg "$work_dir/control-server.log" \
    "$BIN_DIR/control-server" \
      --listen "127.0.0.1:$control_port" \
      --token-secret "$TOKEN_SECRET" \
      --relay-addr "$relay_addr" \
      --punch-addr "$punch_addr" \
      --state-db "$control_state_db" >/dev/null
  wait_for_tcp 127.0.0.1 "$control_port" control-server

  start_echo_server "$echo_port" "$work_dir/echo.log" >/dev/null
  wait_for_log "$work_dir/echo.log" "echo listening" "echo server"

  local agent_args=(
    "$BIN_DIR/agentd"
    --relay-cert "$relay_cert"
    --service "$SERVICE_ID=127.0.0.1:$echo_port"
    --control "$control_url"
    --device "$DEVICE_ID"
    --agent-token agent-token
    --poll-ms 20
  )
  if [[ "$mode" == "p2p" ]]; then
    agent_args+=(
      --p2p-identity-dir "$work_dir/agent-identity"
      --p2p-bind "127.0.0.1:0"
      --p2p-candidate-timeout-ms 1000
      --p2p-probe-timeout-ms 1000
      --p2p-interval-ms 10
    )
  fi
  start_bg "$work_dir/agentd.log" "${agent_args[@]}" >/dev/null
  wait_for_log "$work_dir/agentd.log" "agentd polling control" agentd

  start_bg "$work_dir/mobile-cli.log" \
    "$BIN_DIR/mobile-cli" open-service \
      --control "$control_url" \
      --token user-token \
      --client "$CLIENT_ID" \
      --device "$DEVICE_ID" \
      --service "$SERVICE_ID" \
      --local "$local_port" \
      --relay-cert "$relay_cert" \
      --p2p-bind "127.0.0.1:0" \
      --p2p-candidate-timeout-ms 1000 \
      --p2p-probe-timeout-ms 1000 \
      --p2p-interval-ms 10 \
      --relay-fallback-delay-ms 50 >/dev/null
  wait_for_log "$work_dir/mobile-cli.log" "mobile-cli forwarding" mobile-cli

  assert_forward_response "$local_port"
  assert_http_forward_response "$local_port"
  if [[ "$mode" == "fallback" ]]; then
    assert_admin_session_visible "$relay_admin_port"
  fi
  log "$mode case passed"
  cleanup
}

main() {
  mkdir -p "$TMP_ROOT"
  trap cleanup EXIT

  log "building workspace binaries"
  cargo build --workspace --bins

  run_case p2p
  run_case fallback

  log "all smoke cases passed"
}

main "$@"
