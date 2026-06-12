import Foundation
import Network
import WebKit

public enum QuicTunnelBrowserProxyError: Error, Equatable {
    case invalidProxyPort(UInt16)
}

@MainActor
public final class QuicTunnelBrowserProxyController {
    public let tunnel: FfiMobileTunnel
    private var browserProxy: FfiBrowserProxy?

    public static func makeBrowserProxyConfig(
        bindHost: String = "127.0.0.1",
        localPort: UInt16 = 0,
        domainSuffix: String = ".qtunnel.local",
        maxConnections: UInt64 = 256,
        directFallbackPolicy: FfiBrowserProxyDirectFallbackPolicy = .localNetworkAndDomain,
        requestHeadTimeoutMs: UInt64 = 10_000,
        directConnectTimeoutMs: UInt64 = 10_000,
        tunnelOpenTimeoutMs: UInt64 = 15_000,
        idleTimeoutMs: UInt64 = 120_000
    ) -> FfiBrowserProxyConfig {
        var config = browserProxyConfigWithDefaults()
        config.bindHost = bindHost
        config.localPort = localPort
        config.domainSuffix = domainSuffix
        config.maxConnections = maxConnections
        config.directFallbackPolicy = directFallbackPolicy
        config.requestHeadTimeoutMs = requestHeadTimeoutMs
        config.directConnectTimeoutMs = directConnectTimeoutMs
        config.tunnelOpenTimeoutMs = tunnelOpenTimeoutMs
        config.idleTimeoutMs = idleTimeoutMs
        return config
    }

    public init(tunnel: FfiMobileTunnel) {
        self.tunnel = tunnel
    }

    public var currentBrowserProxy: FfiBrowserProxy? {
        browserProxy
    }

    @discardableResult
    public func startBrowserProxy(
        config: FfiBrowserProxyConfig = browserProxyConfigWithDefaults()
    ) throws -> FfiBrowserProxy {
        if let current = browserProxy, !current.isClosed() {
            return current
        }

        let proxy = try tunnel.startBrowserProxyWithConfig(config: config)
        browserProxy = proxy
        return proxy
    }

    public func deviceServiceUrl(
        deviceId: String,
        serviceId: String,
        pathAndQuery: String = "/"
    ) throws -> String {
        let route = try browserProxyDeviceServiceRoute(deviceId: deviceId, serviceId: serviceId)
        return browserProxyRouteHttpUrl(route: route, pathAndQuery: pathAndQuery)
    }

    public func classify(
        url: String,
        controlServerUrl: String
    ) throws -> FfiBrowserProxyUrlClassification {
        try browserProxyClassifyUrlWithDefaults(url: url, controlServerUrl: controlServerUrl)
    }

    public func stats(
        proxy explicitProxy: FfiBrowserProxy? = nil
    ) throws -> FfiBrowserProxyStats {
        let proxy: FfiBrowserProxy
        if let explicitProxy {
            proxy = explicitProxy
        } else {
            proxy = try startBrowserProxy()
        }

        return proxy.stats()
    }

    @available(iOS 17.0, macOS 14.0, visionOS 1.0, *)
    public func proxyConfiguration(for proxy: FfiBrowserProxy) throws -> ProxyConfiguration {
        guard let port = NWEndpoint.Port(rawValue: proxy.port()) else {
            throw QuicTunnelBrowserProxyError.invalidProxyPort(proxy.port())
        }

        let endpoint = NWEndpoint.hostPort(host: NWEndpoint.Host(proxy.host()), port: port)
        var configuration = ProxyConfiguration(httpCONNECTProxy: endpoint)
        configuration.allowFailover = false
        return configuration
    }

    @available(iOS 17.0, macOS 14.0, visionOS 1.0, *)
    public func applyProxy(
        to configuration: WKWebViewConfiguration,
        proxy explicitProxy: FfiBrowserProxy? = nil
    ) throws {
        let proxy: FfiBrowserProxy
        if let explicitProxy {
            proxy = explicitProxy
        } else {
            proxy = try startBrowserProxy()
        }

        configuration.websiteDataStore.proxyConfigurations = [
            try proxyConfiguration(for: proxy)
        ]
    }

    @available(iOS 17.0, macOS 14.0, visionOS 1.0, *)
    public func clearProxy(from configuration: WKWebViewConfiguration) {
        configuration.websiteDataStore.proxyConfigurations = []
    }

    public func closeBrowserProxy() throws {
        if let proxy = browserProxy, !proxy.isClosed() {
            try proxy.close()
        }
        browserProxy = nil
    }

    public func shutdown() throws {
        try closeBrowserProxy()
        try tunnel.shutdown()
    }
}
