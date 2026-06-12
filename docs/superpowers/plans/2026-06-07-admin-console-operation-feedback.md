# Admin Console Operation Feedback

## Goal

Improve the zero-build Control Admin console feedback loop for management
actions without changing backend API contracts.

## Scope

- [x] Add smoke assertions for operation feedback hooks.
- [x] Add an Operations panel with a recent operation log.
- [x] Record successful and failed UI actions with area, operation name, and
      message.
- [x] Preserve failure context in the central operation log and existing status
      fields.
- [x] Refresh Dashboard and Audit Logs after admin management mutations when an
      admin token is available.
- [x] Skip overview refresh gracefully when a user-scoped action runs without an
      admin token.
- [x] Document the workflow in `README.md`.

## Verification

- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script control_admin_page_targets_current_control_api`

