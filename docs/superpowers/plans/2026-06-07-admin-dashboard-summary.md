# Admin Dashboard Summary

## Goal

Add a simple admin-only dashboard snapshot for the Control plane and surface it
in the zero-build admin console.

## Scope

- [x] Add shared dashboard DTOs to `quic_tunnel_control_client`.
- [x] Add `GET /dashboard` as an admin-only Control route.
- [x] Aggregate users, devices, controller devices, sessions, Relays, actual
      Relay usage totals, and recent audit logs from Control state.
- [x] Add a Dashboard panel to `docs/control-admin.html`.
- [x] Document the endpoint in `README.md`.

## Verification

- [x] `cargo test -p quic_tunnel_control --test control_plane admin_dashboard_summary_reports_control_plane_totals`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script control_admin_page_targets_current_control_api`

