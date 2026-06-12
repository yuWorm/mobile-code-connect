# Mobile CLI Admin List Queries

## Goal

Expose the Rust admin list query methods through `mobile-cli` so operators can
inspect Control state from scripts without opening the browser admin console.

## Scope

- [x] Add CLI parsing tests for admin users query arguments.
- [x] Add CLI parsing tests for device access query arguments.
- [x] Add `mobile-cli admin` subcommands for paginated admin list endpoints:
      users, audit, usage, sessions, controllers, devices, plan catalog, relay
      credentials, relays, and device access.
- [x] Reuse `HttpControlClient::*_with_query(AdminListQuery)` methods.
- [x] Print query responses as pretty JSON.
- [x] Document examples and supported subcommands in `README.md`.

## Verification

- [x] `cargo test -p mobile-cli admin_`

