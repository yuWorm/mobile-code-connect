# Mobile UniFFI SDK Design

## Goal

Expose the existing Rust mobile tunnel as a real iOS/Android SDK surface using
UniFFI, starting with local scoped forwarding only.

## Scope

The first version provides app-local forwarding:

- Swift/Kotlin creates a mobile tunnel object.
- Swift/Kotlin opens a local `127.0.0.1:<port>` forward for a target
  device/service.
- Swift/Kotlin closes forwards and reads tunnel status.

This is not a VPN and does not install a system-wide network extension. Future
HTTP proxy support should be another local listener API, not a replacement for
the first SDK shape.

## Architecture

```text
iOS/Android app
  -> generated Swift/Kotlin UniFFI bindings
    -> Rust FFI facade
      -> quic_tunnel_mobile_core::TunnelClient
        -> local TCP forward
          -> P2P or Relay stream connector
```

The FFI facade owns a Tokio runtime so foreign callers can use synchronous
object methods. The public FFI types use UniFFI-friendly primitives:
`String`, `u16`, `u32`, `u64`, `Vec<u8>`, records, enums, and flat errors.

## Public Shape

- `FfiMobileTunnelConfig`: token, control URL, client id, control retry options.
- `FfiP2pOrRelayConfig`: relay certificate bytes, P2P bind address, timeout
  values.
- `FfiMobileTunnel`: UniFFI object with constructors and lifecycle methods.
- `FfiOpenServiceRequest`: device id, service id, local port.
- `FfiForwardHandle`: handle id, local port, target ids.
- `FfiTunnelStatus`: state, selected path, traffic counters, active forwards.
- `FfiMobileError`: flat error for invalid config, runtime, tunnel, and closed
  tunnel failures.

## Non-Goals

- Generated binding artifacts checked into this change.
- iOS `XCFramework` or Android `AAR` packaging scripts.
- VPN, NetworkExtension, VpnService, or global proxy behavior.
- Admin APIs through the mobile FFI surface.

## Testing

Unit tests should validate FFI DTO conversion and lifecycle behavior using the
existing in-memory connector so tests do not require Control, Relay, or P2P
network services.

## Environment Note

The current workspace is not a Git repository from the tool environment, so this
spec cannot be committed here.
