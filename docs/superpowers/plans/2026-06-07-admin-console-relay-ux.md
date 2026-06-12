# Admin Console Relay UX

## Goal

Make the zero-build Control Admin console easier to operate for Relay pool and
Relay credential management without changing backend API contracts.

## Scope

- [x] Add smoke assertions for Relay and Relay credential selection hooks.
- [x] Add an Action column to the Relay Pool table.
- [x] Add one-click Relay selection that fills update/delete fields and the
      credential Relay id.
- [x] Add an Action column to the Relay Credential table.
- [x] Add one-click credential selection that fills credential status/rotation
      fields and the Relay id.
- [x] Refill Relay/Credential edit fields after create/update/load/rotate
      operations.
- [x] Document the console workflow in `README.md`.

## Verification

- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script control_admin_page_targets_current_control_api`

