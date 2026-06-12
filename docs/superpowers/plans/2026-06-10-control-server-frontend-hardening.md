# Control Server Frontend Hardening Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the Control Server admin console and user center from "feature-complete pages" to stable, consistent, production-usable frontend workflows.

**Architecture:** Keep the current Vite + Vue 3 + TypeScript + shadcn-vue structure. Centralize cross-cutting behavior in `web/src/lib/control/*`, `web/src/composables/*`, and shared layout components, then keep route views thin and consistent.

**Tech Stack:** Vite, Vue 3, TypeScript, Vue Router, shadcn-vue/Reka UI, Tailwind CSS, Bun tests, vue-tsc.

---

## Execution Rules

- Track progress in this file by changing `[ ]` to `[x]` as work completes.
- Use TDD for behavior changes: write a failing test, run it red, implement, run it green.
- After each task, run the task-specific test command listed below.
- After each phase, run `cd web && bun test`.
- Before calling the work complete, run `cd web && bun test` and `cd web && bun run build`.
- Keep changes scoped to the files listed for the task unless the implementation proves another shared file is the right boundary.

## Current Baseline

- [x] i18n foundation exists in `web/src/lib/i18n/*`.
- [x] All route views and shared layout components use i18n for visible copy.
- [x] `rg -n "[\p{Han}]" web/src/views web/src/components -g '*.vue'` returns no matches.
- [x] Full test suite passed after i18n sweep.
- [x] Production build passed after i18n sweep.

---

## Phase 1: Auth, Route Stability, and Global API Errors

**Goal:** Make login state, route guards, token expiry, and 401/403 handling predictable.

**Primary files:**
- Modify: `web/src/lib/control/auth.ts`
- Modify: `web/src/lib/control/api.ts`
- Modify: `web/src/lib/control/client.ts`
- Modify: `web/src/composables/useAuth.ts`
- Modify: `web/src/router/index.ts`
- Modify: `web/src/lib/i18n/messages.ts`
- Test: `web/src/lib/control/__tests__/auth.test.ts`
- Test: `web/src/lib/control/__tests__/api.test.ts`
- Test: `web/src/views/__tests__/oauth-login.test.ts`
- Test: create `web/src/router/__tests__/guards.test.ts` if router behavior needs direct coverage.

### Task 1.1: Session Expiry Helpers

- [x] Add failing tests for expired and valid JWT sessions in `web/src/lib/control/__tests__/auth.test.ts`.
- [x] Run `cd web && bun test src/lib/control/__tests__/auth.test.ts` and confirm failure.
- [x] Add helpers in `web/src/lib/control/auth.ts`:
  - `isSessionExpired(session, nowSeconds?)`
  - `isStoredSessionUsable(session, nowSeconds?)`
  - `redirectForRole(role)`
- [x] Keep the current token decoding API backward compatible.
- [x] Run `cd web && bun test src/lib/control/__tests__/auth.test.ts` and confirm pass.

### Task 1.2: Route Guard Redirects and Expired Session Cleanup

- [x] Add failing tests for unauthenticated redirect preservation, expired session cleanup, and admin-to-center fallback.
- [x] Run the targeted router/auth tests and confirm failure.
- [x] Update `web/src/router/index.ts` to use the new auth helpers.
- [x] Ensure expired sessions are cleared before redirecting to `/login`.
- [x] Ensure `/login?redirect=/center/devices` sends a successful login back to the safe redirect target.
- [x] Run targeted tests and confirm pass.

### Task 1.3: Login/Register Redirect Target

- [x] Add failing tests in `web/src/views/__tests__/oauth-login.test.ts` or a new auth view test for post-login redirect behavior.
- [x] Run targeted tests and confirm failure.
- [x] Update `web/src/composables/useAuth.ts` to resolve safe redirect targets from the current route query.
- [x] Keep admin users from being redirected into user-only paths when a safer admin home is required.
- [x] Run targeted tests and confirm pass.

### Task 1.4: Global Unauthorized/Forbidden Handling

- [x] Add failing tests in `web/src/lib/control/__tests__/api.test.ts` for auth failure hooks.
- [x] Run targeted tests and confirm failure.
- [x] Add an optional auth-failure callback to `ControlApi`.
- [x] Wire `web/src/lib/control/client.ts` and `web/src/composables/useAuth.ts` so 401 clears session and moves to login.
- [x] Do not clear session on 403; surface a localized forbidden message instead.
- [x] Run targeted tests and confirm pass.

**Phase 1 verification:**
- [x] `cd web && bun test src/lib/control/__tests__/auth.test.ts src/lib/control/__tests__/api.test.ts`
- [x] `cd web && bun test src/views/__tests__/oauth-login.test.ts`
- [x] `cd web && bun test`

---

## Phase 2: Global Interaction Consistency

**Goal:** Make forms, dialogs, loading states, destructive actions, and mutation feedback consistent across pages.

**Primary files:**
- Modify: `web/src/components/layout/ConfirmAction.vue`
- Modify: `web/src/components/layout/SearchToolbar.vue`
- Modify: `web/src/components/layout/ResponsiveTable.vue`
- Modify: `web/src/components/control/ByteUnitInput.vue`
- Modify: `web/src/components/control/DurationUnitInput.vue`
- Modify route views only where shared components cannot cover the gap.
- Test: component tests under `web/src/components/**/__tests__/*`
- Test: route source tests under `web/src/views/__tests__/*`

### Task 2.1: Form Submit and Reset Consistency

- [x] Audit create/edit dialogs for missing inline reset, loading icon, disabled submit, and close guard.
- [x] Add failing source tests for any missing route-level consistency.
- [x] Implement the missing controls with existing shadcn-vue components.
- [x] Run targeted route tests and confirm pass.

### Task 2.2: Shared Mutation Feedback Contract

- [x] Extend `web/src/views/__tests__/mutation-feedback.test.ts` to cover remaining mutation-heavy views.
- [x] Run it red for any uncovered view.
- [x] Ensure mutations use `runWithToast` and `useBusyAction` consistently.
- [x] Run targeted tests and confirm pass.

### Task 2.3: Empty, Loading, and Error State Consistency

- [x] Add tests for consistent `ResponsiveTable`, `LoadingState`, and `ErrorState` usage in route views.
- [x] Replace ad hoc empty/loading/error markup only when a shared component already fits.
- [x] Run targeted tests and confirm pass.

**Phase 2 verification:**
- [x] `cd web && bun test src/components src/views/__tests__/mutation-feedback.test.ts`
- [x] `cd web && bun test`

---

## Phase 3: Core Workflow Closure

**Goal:** Confirm the main admin/user workflows have complete UI states and backend-aligned request/response handling.

**Primary files:**
- Modify: `web/src/views/admin/AdminUsersView.vue`
- Modify: `web/src/views/admin/AdminDevicesView.vue`
- Modify: `web/src/views/admin/AdminRelaysView.vue`
- Modify: `web/src/views/user/UserDevicesView.vue`
- Modify: `web/src/views/user/UserCredentialsView.vue`
- Modify: `web/src/lib/control/forms.ts`
- Modify: `web/src/lib/control/types.ts` only when backend contracts require it.
- Test: existing route tests and control form/type helper tests.

### Task 3.1: User Management Workflow

- [x] Review create, enable/disable, role change, plan assignment, plan override, detail search, and usage reset flows.
- [x] Add failing tests for missing stale-state cleanup or backend refresh.
- [x] Implement minimal fixes.
- [x] Run `cd web && bun test src/views/__tests__/admin-users-filters.test.ts`.

### Task 3.2: Device and Session Workflow

- [x] Review admin device access grants and user service/session creation.
- [x] Add failing tests for missing cleanup, refresh, copy, or disabled states.
- [x] Implement minimal fixes.
- [x] Run `cd web && bun test src/views/__tests__/admin-infra-filters.test.ts src/views/user/__tests__/user-devices.test.ts`.

### Task 3.3: Relay and Credential Workflow

- [x] Review Relay register/edit/remove, Relay credential create/toggle/rotate, server credential auth/approve/poll/rotate.
- [x] Add failing tests for missing result handling or stale sensitive token cleanup.
- [x] Implement minimal fixes.
- [x] Run `cd web && bun test src/views/__tests__/admin-infra-filters.test.ts src/views/user/__tests__/user-credentials.test.ts`.

**Phase 3 verification:**
- [x] `cd web && bun test src/views/__tests__/admin-users-filters.test.ts src/views/__tests__/admin-infra-filters.test.ts src/views/user/__tests__/user-devices.test.ts src/views/user/__tests__/user-credentials.test.ts`
- [x] `cd web && bun test`

---

## Phase 4: Responsive QA and Accessibility

**Goal:** Make the console reliable on desktop, tablet, and mobile, with usable keyboard and screen-reader labels.

**Primary files:**
- Modify shared layout and route views as needed.
- Test: `web/src/components/layout/__tests__/*`
- Test: `web/src/views/__tests__/*`

### Task 4.1: Mobile Layout Scan

- [x] Add or update tests that enforce responsive card/table fallbacks for each route view.
- [x] Fix obvious overflow sources: long tokens, IDs, service names, copy buttons, and action groups.
- [x] Run targeted view tests.

### Task 4.2: Accessibility Labels

- [x] Audit icon-only buttons, selects, switches, dialog close buttons, and destructive actions.
- [x] Add failing tests for missing `aria-label` or screen-reader text.
- [x] Implement labels using i18n keys.
- [x] Run targeted component and view tests.

### Task 4.3: Visual Runtime Check

- [x] Start Vite with `cd web && bun run dev --host 127.0.0.1`.
- [ ] Check login, admin users, admin devices, admin relays, user devices, and user credentials on desktop width.
- [ ] Check login and at least one admin/user data page on mobile width.
- [x] Record any issues as new plan items before fixing them.

**Phase 4 verification:**
- [x] `cd web && bun test`
- [ ] Browser/runtime smoke check completed.

---

## Phase 5: Backend Contract Regression and Release Readiness

**Goal:** Validate that the frontend matches the real Control API and can be used as the operations console.

**Primary files:**
- Modify API client/types/form helpers only when backend mismatch is found.
- Modify docs if startup or environment instructions are stale.
- Test: `web/src/lib/control/__tests__/*`
- Test: relevant route tests.

### Task 5.1: Backend Startup and Environment Check

- [x] Document the backend startup command and required env variables in the plan execution notes.
- [x] Start backend locally or connect to the configured backend.
- [x] Confirm login/register/OAuth callback base URL behavior with `VITE_CONTROL_API_BASE_URL`.

### Task 5.2: Contract Walkthrough

- [x] Exercise login/register.
- [x] Exercise dashboard loading.
- [x] Exercise admin users/devices/relays/credentials/plans/oauth/audit pages.
- [x] Exercise user dashboard/devices/controllers/credentials/account pages.
- [x] Fix any mismatched fields, status values, or request payloads with tests first.

### Task 5.3: Final Verification

- [x] `cd web && bun test`
- [x] `cd web && bun run build`
- [x] `rg -n "[\p{Han}]" web/src/views web/src/components -g '*.vue'` stays empty unless a literal product/backend value is intentionally present.
- [x] Record remaining known warnings, especially third-party build warnings.

---

## Execution Log

- 2026-06-10: Created plan after completing i18n sweep. Next task is Phase 1 Task 1.1.
- 2026-06-10: Completed Phase 1 Task 1.1. Added session expiry and role redirect helpers with targeted auth tests passing.
- 2026-06-10: Completed Phase 1 Tasks 1.2 and 1.3. Added router guard helpers, expired-session cleanup, and safe post-login redirect handling for email and GitHub OAuth flows.
- 2026-06-10: Completed Phase 1 Task 1.4 and Phase 1 verification. Added ControlApi 401 auth-failure hook, kept 403 local to page error handling, wired useAuth session cleanup, and confirmed full test suite passed with 196 tests.
- 2026-06-10: Completed Phase 2. Added default non-submit Button behavior, tightened ConfirmAction/SearchToolbar/status component semantics, added route state component coverage, and confirmed full test suite passed with 200 tests.
- 2026-06-10: Completed Phase 3. Reviewed core admin/user workflows, added route-view i18n hardcoded identity-label guard, localized remaining ID/token labels, and confirmed full test suite passed with 201 tests.
- 2026-06-10: Phase 4 source-level checks completed. Added mobile card fallback coverage and sensitive token overflow constraints; dev server returned HTTP 200 at `http://127.0.0.1:5175/`. In-app browser was unavailable (`iab`), so visual desktop/mobile smoke remains unchecked.
- 2026-06-10: Phase 5 startup notes: direct backend smoke used `target/debug/control-server --listen 127.0.0.1:14242 --token-secret dev-secret --relay-addr 127.0.0.1:4443 --punch-addr 127.0.0.1:3478 --state-db /private/tmp/mobilecode-connect-control-smoke-20260610.sqlite --bootstrap-admin-email admin@example.com --bootstrap-admin-password admin-password-123 --bootstrap-admin-display-name Admin`. Required CLI/env mapping confirmed from `apps/control-server/src/main.rs`: `QUIC_TUNNEL_TOKEN_SECRET`, `QUIC_TUNNEL_CONTROL_STATE_DB`, `QUIC_TUNNEL_STRICT_AUTH`, `QUIC_TUNNEL_PUBLIC_URL`, `QUIC_TUNNEL_GITHUB_CLIENT_ID`, `QUIC_TUNNEL_GITHUB_CLIENT_SECRET`, `QUIC_TUNNEL_GITHUB_REDIRECT_URL`, `QUIC_TUNNEL_ADMIN_EMAIL`, `QUIC_TUNNEL_ADMIN_PASSWORD`, and `QUIC_TUNNEL_ADMIN_DISPLAY_NAME`.
- 2026-06-10: Phase 5 frontend API environment notes: `web/src/lib/control/client.ts` uses `VITE_CONTROL_API_BASE_URL` as the direct API origin. When it is empty, requests stay same-origin and `web/vite.config.ts` proxies Control API prefixes to `VITE_CONTROL_API_PROXY_TARGET`, defaulting to `http://127.0.0.1:4242`. Existing dev server at `http://127.0.0.1:5175/` returned HTTP 200 and proxied `/dashboard` to its configured backend, returning 401 for the temporary `14242` token. Starting a fresh Vite server on `5176` with a temporary proxy target was blocked by sandbox listen approval, so runtime proxy retargeting was not completed.
- 2026-06-10: Phase 5 backend contract walkthrough completed against the temporary backend. Verified admin login, user registration/login, password update, dashboard, users, devices, sessions, audit logs, usage, plan catalog/current plan, relays, relay credentials, server credentials, OAuth identities, controller registration, device-code server credential start/approve/poll, agent device/service registration, mobile device/service listing, session creation, and session close. No frontend type or payload mismatch was found.
- 2026-06-10: Final frontend verification passed: `bun test` reported 203 pass and 0 fail; `bun run build` exited 0. The build still prints known Rolldown `INVALID_ANNOTATION` warnings from `node_modules/@vueuse/core/dist/index.js`. The route/component Chinese hardcoded-copy scan returned no matches.
- 2026-06-11: Implemented Relay Bootstrap frontend management for the new Control API. Added `POST /relay-bootstraps` and `POST /relay-bootstraps/{bootstrap_id}/exchange` to the web API client/types, added `/relay-bootstraps` to the Vite dev proxy, and extended Admin Relay management with create/exchange dialogs plus one-time token, install command, relay control token, and token secret result cards with copy and dismiss actions. Verification passed: `bun test` reported 208 pass and 0 fail; `bun run build` exited 0 with the known `@vueuse/core` Rolldown `INVALID_ANNOTATION` warnings; the route/component Chinese hardcoded-copy scan returned no matches.
- 2026-06-11: Relay Bootstrap visual browser smoke was attempted after implementation, but the in-app browser remained unavailable (`Browser is not available: iab`).
