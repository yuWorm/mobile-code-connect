# Relay Health Reporting Design

## Goal

Make relay status production-usable by reporting relayd self-check results and runtime metrics to Control, while keeping the existing relay registration/update API backward compatible.

## Scope

This round implements heartbeat-backed health reporting, not a full external monitoring system. Control continues to use heartbeat freshness for liveness, and relayd adds local self-check data so Control can distinguish stale relays, unhealthy relays, and healthy relays with useful metrics.

## Data Model

- Add a `RelayHealthStatus` enum: `healthy`, `degraded`, `unhealthy`.
- Add a `RelayHealthReport` payload sent by relayd during heartbeat:
  - status, reason
  - version, uptime
  - active sessions, active streams
  - total uplink/downlink/bytes
  - data-plane/admin listener check flags
- Add a `ReportRelayHealthRequest` payload for relayd health heartbeats.
- Extend `RelayNode` with the latest health report fields. Keep the existing `healthy` and `last_seen_epoch_sec` fields for old clients and filters.

## Control Behavior

- Relay registration initializes a healthy snapshot.
- Relay update remains backward compatible for old callers that only send `healthy`.
- Relay health reports are submitted to `/relays/{relay_id}/health`. Control stores the report, updates `last_seen_epoch_sec`, and derives `healthy` from the reported status.
- On reads and relay selection, Control applies the existing heartbeat timeout. Stale relays are returned as `unhealthy` with a stale reason.
- Relay selection uses the effective health state so stale/unhealthy relays are not selected.

## Relayd Behavior

- Each heartbeat builds a health report from local runtime state:
  - QUIC data-plane listener was bound.
  - Admin listener is bound if configured.
  - Session store metrics summarize active sessions, active streams, and bytes.
  - Version and uptime are included for operations.
- The report is sent with the existing relay update call.
- Existing session usage reporting remains unchanged.

## Admin/Operations Surface

- Relay admin routes expose a lightweight `/admin/health` endpoint for future active probes and operator checks.
- Control/Admin clients can list relays and see the latest status, reason, uptime, version, and load metrics.

## Testing

- Control tests cover health report persistence, stale relay health downgrade, and relay selection ignoring stale/unhealthy relays.
- Relayd tests cover health report construction from sessions.
- Relay admin route tests cover the new `/admin/health` endpoint.
- Existing relay bootstrap, relay update, SDK, and CLI tests must stay green.
