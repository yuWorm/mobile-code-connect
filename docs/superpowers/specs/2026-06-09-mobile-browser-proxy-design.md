# Mobile Browser Proxy Design

## Goal

Add a browser-oriented mobile SDK mode that avoids per-service local port
management. The mobile app configures its embedded WebView to use one
SDK-managed local HTTP proxy. The proxy resolves synthetic service hosts to
`device_id + service_id`, opens the existing P2P-or-Relay tunnel stream, and
forwards browser traffic.

## Scope

First version:

- one local proxy listener on `127.0.0.1:<ephemeral>`;
- HTTP absolute-form proxy requests;
- HTTPS `CONNECT` passthrough;
- host mapping: `<service_id>.<device_id>.qtunnel.local`;
- UniFFI API to start/stop the browser proxy and read host/port;
- no legacy WebView fallback.

## Non-Goals

- No system VPN.
- No MITM or HTTPS interception.
- No WebView request-interception fallback for old iOS/Android versions.
- No service discovery UI or generated browser landing page in this slice.
- No authentication injection or cookie rewriting.

## Architecture

```text
WKWebView / Android WebView
  -> proxy 127.0.0.1:<port>
    -> BrowserProxy
      -> StreamConnector.open_stream(device_id, service_id)
        -> P2P or Relay stream
          -> Agent local service
```

`BrowserProxy` reuses the same stream connector boundary as `LocalForwarder`.
The proxy strips HTTP proxy absolute-form URLs to origin-form before forwarding
to the agent service. Plain HTTP requests are forwarded as one request per proxy
connection with `Connection: close`; `Content-Length` request bodies are copied
exactly and extra pipelined bytes are not forwarded to the first service stream.
Chunked request bodies keep their original chunk framing and are copied through
the final zero chunk and trailers only. Ambiguous requests containing both
`Transfer-Encoding: chunked` and `Content-Length` are rejected before any service
stream is opened. HTTP head and request body parsing use bounded buffered reads
so split chunk lines are handled without per-byte socket reads. For `CONNECT`,
it sends a `200 Connection Established` response to the browser and then copies bytes
bidirectionally without touching TLS.

## Mobile Platform Contract

- iOS: app uses modern WebKit proxy configuration for the embedded browser.
- Android: app uses AndroidX WebKit `ProxyController.setProxyOverride`.
- The SDK exposes only the proxy endpoint. Native app code owns applying that
  endpoint to the WebView.

## Testing

Use `MemoryStreamConnector` so tests can assert forwarded bytes without Control,
Relay, P2P, or an actual agent. Cover:

- host parsing;
- HTTP absolute-form rewrite;
- one-request plain HTTP connection handling, `Content-Length` body bounds, and
  chunked body bounds;
- CONNECT handshake and tunnel bytes;
- proxy shutdown;
- UniFFI handle lifecycle.
