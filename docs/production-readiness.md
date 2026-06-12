# Production Readiness

This checklist is the release gate for moving the current MVP toward a
production deployment. It does not claim the system is production-ready by
itself; it makes the remaining security, operations, billing, and rollback work
explicit and repeatable.

## Release Gate

Run the default validation before every release candidate:

```bash
./scripts/production-check.sh
```

The default gate runs shell syntax checks, formatting checks, smoke contract
tests, the SDK live workflow against in-process Control routes, `mobile-cli`
admin parsing tests, and workspace compile checks. It avoids socket-binding E2E
tests by default so it can run in restricted development sandboxes.

Use heavier gates before deployment:

```bash
MOBILECODE_CONNECT_PROD_CHECK_FULL=1 ./scripts/production-check.sh
MOBILECODE_CONNECT_PROD_CHECK_E2E=1 ./scripts/production-check.sh
MOBILECODE_CONNECT_PROD_CHECK_MOBILE_PACKAGE=1 ./scripts/production-check.sh
MOBILECODE_CONNECT_PROD_CHECK_DEVICE_SIGNOFF=1 ./scripts/production-check.sh
MOBILECODE_CONNECT_PROD_CHECK_REQUIRE_RUNTIME_ENV=1 \
MOBILECODE_CONNECT_STRICT_AUTH=true \
MOBILECODE_CONNECT_TEST_TOKEN_SECRET="<production-secret>" \
./scripts/production-check.sh
```

Do not deploy if any gate fails.

## Security

- Run Control with `MOBILECODE_CONNECT_STRICT_AUTH=true` or `--strict-auth`.
  Legacy `QUIC_TUNNEL_*`, `QUIC_TEST_*`, and `QUIC_PROD_CHECK_*` names remain
  accepted as fallback aliases where those settings existed before the rename.
- Treat Control `/admin` as the Relay and system management surface. Put it
  behind authenticated access, private networking, or an operator VPN.
- Do not expose a standalone Relay Admin endpoint in production. `relayd`
  installs with Relay admin HTTP disabled by default; `--debug-admin-listen`
  is for a debug-only local Relay admin during controlled troubleshooting.
- Keep user tokens, admin tokens, and Relay registration tokens out of logs and
  shell history where possible.
- Use role-aware tokens: normal users must not call admin-only routes, Relay
  tokens must only register and heartbeat Relay nodes, and admin tokens should
  be short-lived operational credentials.

## Authentication

- Bootstrap a persistent admin account before exposing the service.
- Configure GitHub OAuth before enabling public login:
  `MOBILECODE_CONNECT_PUBLIC_URL`, `MOBILECODE_CONNECT_GITHUB_CLIENT_ID`,
  `MOBILECODE_CONNECT_GITHUB_CLIENT_SECRET`, and optional
  `MOBILECODE_CONNECT_GITHUB_REDIRECT_URL`.
- Confirm the system curl binary is installed on Control hosts; the GitHub OAuth
  client uses `curl` for token exchange and GitHub API calls.
- Verify user login through `GET /auth/oauth/github/start` and
  `GET /auth/oauth/github/callback` in staging before release.
- Verify OAuth account linking appears in `GET /oauth/identities`, and use
  `user_id`/`q` filters when auditing linked GitHub identities as an admin.
- Confirm OAuth unlink requests reject deleting a user's final login method, and
  review `oauth_identity.unlink` audit log entries after account support work.
- Confirm OAuth-only users can set a first password through `POST /auth/password`
  before unlinking GitHub, and review `auth.password.set` /
  `auth.password.change` audit log entries for password support work.
- Use `agentd login` or `agentd login --device-code` to create each controlled
  server credential, then run servers with `agentd run --credential-file`.
- Confirm each `ControlRole::Agent` credential is scoped to exactly one server
  device, and that an Agent credential cannot register services, P2P
  certificates, or sessions for another device.
- Review `/server-credentials` with `user_id`, `device_id`, `enabled`, and `q`
  filters; use `last_used_epoch_sec` to find stale controlled-server
  credentials.
- Disable stale server credential records during offboarding, and rotate
  credentials with `/server-credentials/{credential_id}/rotate` after suspected
  leakage or host rebuilds.
- Confirm disabled users cannot log in and existing disabled-user bearer tokens
  are rejected.
- Confirm `mobile-cli admin` commands use an admin token, not a normal user
  token.
- Rotate admin credentials after any shared development or staging use.

## Secrets

- Never use `dev-secret` in production.
- Store Control token secrets, Relay token secrets, database credentials, and
  operator credentials in a secret manager or deployment secret store.
- Store GitHub OAuth client secrets and generated Agent credential files as
  production secrets. Agent credential files should be readable only by the
  service account running `agentd`.
- Prefer injecting secrets through environment variables or mounted secret
  files, not command history.
- Rotate secrets after staging demos, incident response, or operator turnover.

## TLS

- Serve Control through HTTPS in production.
- Protect Control Admin and Relay data-plane endpoints with TLS or a trusted
  private network boundary.
- Define a certificate rotation procedure for Relay certificates consumed by
  Agent and Mobile clients.
- Do not rely on local self-signed development certificates for public
  deployment.

## Persistence

- Use `--state-db` with a durable SQLite path or replace it with the selected
  production database backend before multi-node deployment.
- Back up the Control state database before migrations, releases, and manual
  repair work.
- Verify that users, roles, plans, Relay credentials, access grants, usage
  periods, audit logs, and Relay pool state survive restart.
- Keep migration steps reversible until a release is stable.

## Mobile

- Regenerate UniFFI bindings with `scripts/gen-mobile-bindings.sh --language all`
  before packaging iOS or Android artifacts.
- Package iOS and Android release artifacts with `scripts/package-mobile-ios.sh`
  and `scripts/package-mobile-android.sh`; use dry-run only for planning, not
  as release evidence. Set `MOBILECODE_CONNECT_PROD_CHECK_MOBILE_PACKAGE=1`, or
  the narrower `MOBILECODE_CONNECT_PROD_CHECK_IOS_PACKAGE=1` /
  `MOBILECODE_CONNECT_PROD_CHECK_ANDROID_PACKAGE=1`, before release to make
  `scripts/production-check.sh` run the real package builds.
- Store mobile grant credentials with `MobileCodeConnectMobileGrantSecureStore`.
  The iOS wrapper stores credential JSON in Keychain; the Android wrapper
  encrypts the credential with an Android Keystore AES-GCM key before writing
  app-private preferences.
- Verify mobile grant pairing, `FfiMobileTunnel.startWithMobileGrant(...)`,
  browser proxy routing, WebView proxy application, and P2P-to-Relay fallback on
  physical iOS and Android devices before release. Record that evidence using
  `docs/mobile-device-acceptance.md`; set
  `MOBILECODE_CONNECT_PROD_CHECK_DEVICE_SIGNOFF=1` to require a signoff file
  during the release gate.
- Confirm mobile grant revocation behavior in staging. Current revocation is
  enforced when new grant sessions are requested; long-lived already-open
  streams require a separate active-stream termination policy if immediate
  disconnect is required.

## Operations

- Run `control-server`, `relayd`, `punch-server`, and `agentd` under a process
  manager such as systemd, launchd, Kubernetes, or a supervised container
  runtime.
- Control-owned Relay management is the production model: operators manage
  Relays through Control `/admin` and Control APIs, while `relayd` only reports
  registration, health, and usage back to Control by default.
- Control-owned Relay Live Ops is also pull-based: `relayd` heartbeat reports
  per-session snapshots to Control, operators issue disconnect actions through
  Control, and `relayd` polls Control for relay-scoped commands before reporting
  command results. Production browsers must not connect directly to Relay Admin HTTP.
- Use Relay Bootstrap for new Relay nodes: create a one-time install command
  with `mobile-cli admin create-relay-bootstrap`, run `scripts/install-relayd.sh`
  on the target Linux host, and verify the Relay appears healthy in Control.
  The installer exchanges the one-time bootstrap token once, writes the
  Relay-scoped control token and Relay token secret into the service environment
  file, downloads or installs the `relayd` binary, and starts normal `relayd`
  registration/heartbeat. Use
  `scripts/install-relayd.sh --dry-run` before touching `/etc` on a target host.
  For startup testing without installing a systemd service, append
  `--no-service`; the installer still exchanges the one-time token and writes
  the resolved env file, then prints the manual `relayd` command to run.
  If a maintainer needs the debug-only local Relay admin for a one-off
  investigation, pass `--debug-admin-listen 127.0.0.1:9090` and keep that
  listener bound to loopback.
  In production, prefer a pinned `--relayd-url` with `--relayd-sha256`; the
  default Control-hosted `/relayd` endpoint is mainly for controlled internal
  installs and local bootstrap testing.
- Verify Relay health reports after install. `relayd` reports data-plane state,
  version, uptime, active sessions, active streams, and traffic totals through
  Control heartbeats; Control is the source of truth for Relay health in normal
  operation.
- Do not use `scripts/dev-stack.sh` as the production supervisor; it is a local
  development tool.
- Pin ports, state paths, log paths, and token secrets in deployment manifests.
- Keep a manual break-glass procedure for disabling users, rotating Relay
  credentials, disabling server credentials, closing sessions, and resetting
  usage periods.

## Observability

- Collect service logs with timestamps and request/session identifiers.
- Monitor Control health, Relay heartbeat freshness, Relay session counts,
  failed auth attempts, audit log volume, and quota exhaustion.
- Alert when a Relay is unhealthy, health status is `degraded`, heartbeats stop,
  token validation spikes, disk space is low, or the Control database backup
  fails.
- Add an external active probe from your production monitoring system to the
  Relay data-plane address. Control currently consumes relayd self-reports and
  heartbeat freshness; it does not yet perform network probes from Control to
  every Relay.
- Review audit logs for admin mutations before and after releases.

## Billing

- Current plans and quotas are technical enforcement primitives, not a full
  billing system.
- Automatic subscription state, payment provider integration, invoices,
  renewals, proration, refunds, and dunning are not implemented yet.
- Until billing is implemented, assign plans manually through Control Admin or
  `mobile-cli admin assign-plan`.

## Quota

- Relay traffic quota enforcement uses reported actual Relay usage for the
  current user usage period.
- Admins can reset a user's current usage period manually.
- Automatic calendar billing periods are not modeled yet.
- Confirm quota behavior with a low-limit test plan before release.

## Backup

- Back up the Control state database on a fixed schedule.
- Store backups outside the host running Control.
- Test restore into a staging environment before trusting backups.
- Include Relay credentials and plan catalog state in restore verification.

## Rollback

- Keep the previous binary or container image available for every release.
- Record the exact Control state database backup associated with each release.
- For schema-affecting changes, define a rollback path before applying the
  migration.
- After rollback, verify login, session creation, Relay heartbeat, usage
  summary, and admin audit views.

## Known Non-Production Gaps

- The Control Admin console is a simple zero-build operational page, not the
  final user and management back office.
- Billing is manual and plan assignment is operator-driven.
- Calendar-based usage periods are not automatic.
- The default local scripts use development secrets unless production
  environment gates are explicitly enforced.
- Deployment manifests, metrics exporters, alert rules, and backup automation
  still need to be added for a real production environment.
