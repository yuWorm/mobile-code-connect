# Admin Console User And Plan UX

## Goal

Make the zero-build Control Admin console easier to use for simple user and
plan management without changing backend API contracts.

## Scope

- [x] Add smoke assertions for user/detail and plan-selection UI hooks.
- [x] Add a User Detail table to the Users panel.
- [x] Add one-click user selection from the Users table.
- [x] Link selected user ids into user, plan, usage-reset, and device-access
      inputs.
- [x] Render selected user controllers and controlled devices from
      `GET /users/{user_id}`.
- [x] Add one-click Plan Catalog template selection into both plan editors.
- [x] Document the console workflow in `README.md`.

## Verification

- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script control_admin_page_targets_current_control_api`

