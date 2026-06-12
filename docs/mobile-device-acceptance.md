# Mobile Device Acceptance

Use this checklist for release-candidate validation on physical iOS and Android
devices. Record evidence in `docs/mobile-device-acceptance-signoff.md`, then run
`MOBILECODE_CONNECT_PROD_CHECK_DEVICE_SIGNOFF=1 ./scripts/production-check.sh`
to enforce that the signoff exists.

## Build Inputs

- Run `scripts/gen-mobile-bindings.sh --language all`.
- Run a real iOS package build with `scripts/package-mobile-ios.sh`; dry-run is
  not release evidence.
- Run a real Android package build with `scripts/package-mobile-android.sh`;
  dry-run is not release evidence.

## iOS

- Install an app build that links `MobileCodeConnectMobileSdk`.
- Pair from an agent invite and verify the grant is saved through Keychain via
  `MobileCodeConnectMobileGrantSecureStore`.
- Restart the app and verify the grant loads without logging into Control.
- Start `FfiMobileTunnel.startWithMobileGrant(...)`, start the browser proxy,
  apply the proxy to `WKWebView`, and load a synthetic device-service URL.
- Verify public domain navigation uses direct fallback, while a public IP
  literal is rejected by the default `LocalNetworkAndDomain` policy.

## Android

- Install an app build that links `MobileCodeConnectMobileSdk`.
- Pair from an agent invite and verify the grant is encrypted with Android
  Keystore before app-private preference storage.
- Restart the app and verify the grant loads without logging into Control.
- Start `FfiMobileTunnel.startWithMobileGrant(...)`, start the browser proxy,
  apply the proxy to WebView with AndroidX `ProxyController`, and load a
  synthetic device-service URL.
- Verify public domain navigation uses direct fallback, while a public IP
  literal is rejected by the default `LocalNetworkAndDomain` policy.

## Network

- Verify P2P is used when both peers can establish the path.
- Block P2P or change networks and verify Relay fallback carries browser proxy
  traffic without a user-visible retry loop.
- Switch between Wi-Fi and cellular while the embedded browser is open and
  confirm the app recovers by recreating the tunnel/proxy when needed.

## Security

- Revoke the mobile grant on the agent and verify new browser proxy requests are
  denied.
- Record whether already-open long-lived streams remain connected until closed;
  immediate revoke disconnect requires an active-stream termination policy.
- Confirm agent target filtering still blocks receiver-side public IP targets.

## Signoff Template

```text
Release:
Date:
iOS device / OS:
Android device / OS:
Control URL:
Agent device:
Services tested:
Evidence:
- iOS Keychain grant load:
- Android Keystore grant load:
- WebView browser proxy:
- P2P path:
- Relay fallback:
- direct fallback:
- public IP rejection:
- revoke behavior:
- LocalNetworkAndDomain default:
Approver:
```
