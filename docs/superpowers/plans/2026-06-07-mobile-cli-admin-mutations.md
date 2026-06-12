# Mobile CLI Admin Mutations

## Goal

Expose common Control admin mutations through `mobile-cli` so operators can
manage users, plans, Relays, and device access from scripts without opening the
browser admin console.

## Scope

- [x] Add CLI parsing tests for creating users.
- [x] Add CLI parsing tests for user status changes and plan assignment.
- [x] Add CLI parsing tests for Relay registration/update.
- [x] Add CLI parsing tests for granting/revoking device access.
- [x] Add `mobile-cli admin` mutation subcommands:
      `create-user`, `set-user-status`, `assign-plan`, `register-relay`,
      `update-relay`, `grant-device-access`, and `revoke-device-access`.
- [x] Reuse existing `HttpControlClient` mutation methods.
- [x] Print resource responses as pretty JSON and revoke success as
      `{"ok":true}`.
- [x] Document usage examples in `README.md`.

## Verification

- [x] `cargo test -p mobile-cli`

