# Admin Console Device Access UX

## Goal

Make the zero-build Control Admin console easier to operate for controlled
devices, services, and device access grants without changing backend API
contracts.

## Scope

- [x] Add smoke assertions for controlled device and access grant selection
      hooks.
- [x] Add an Action column to the controlled devices table.
- [x] Add a Device Detail table for selected controlled devices.
- [x] Add one-click controlled device selection that fills `deviceId`.
- [x] Load services for a selected device and load access grants when an admin
      token is available.
- [x] Add an Action column to the device access grants table.
- [x] Add one-click access grant selection that fills the device/user pair for
      revoke.
- [x] Document the console workflow in `README.md`.

## Verification

- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script control_admin_page_targets_current_control_api`

