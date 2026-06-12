# Dev Stack Admin CLI Helpers

## Goal

Expose the first admin control-plane workflows through `scripts/dev-stack.sh` so
local testing does not require copying an admin token into every
`mobile-cli admin` command.

## Tasks

- [x] Add a smoke contract for admin helper commands.
- [x] Add dev-stack helpers for user, usage, device, relay, audit, and device
  access admin queries.
- [x] Add dev-stack helpers for creating users and granting/revoking controlled
  device access.
- [x] Document the local testing order in `README.md`.

## Verification

- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script dev_stack_script_documents_manual_start_order`
- [x] `bash -n scripts/dev-stack.sh`
- [x] `cargo fmt --check`
- [x] `cargo test -p quic_tunnel_mobile_core --test smoke_script`
- [x] `cargo test -p mobile-cli`
- [x] `cargo test --workspace --no-run`
