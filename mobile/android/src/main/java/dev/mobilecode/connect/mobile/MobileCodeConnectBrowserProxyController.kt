package dev.mobilecode.connect.mobile

import androidx.webkit.ProxyConfig
import androidx.webkit.ProxyController
import androidx.webkit.WebViewFeature
import java.util.concurrent.Executor
import uniffi.mobilecode_connect_mobile_core.FfiBrowserProxy
import uniffi.mobilecode_connect_mobile_core.FfiBrowserProxyConfig
import uniffi.mobilecode_connect_mobile_core.FfiBrowserProxyDirectFallbackPolicy
import uniffi.mobilecode_connect_mobile_core.FfiBrowserProxyStats
import uniffi.mobilecode_connect_mobile_core.FfiBrowserProxyUrlClassification
import uniffi.mobilecode_connect_mobile_core.FfiMobileException
import uniffi.mobilecode_connect_mobile_core.FfiMobileTunnel
import uniffi.mobilecode_connect_mobile_core.browserProxyClassifyUrlWithDefaults
import uniffi.mobilecode_connect_mobile_core.browserProxyConfigWithDefaults
import uniffi.mobilecode_connect_mobile_core.browserProxyDeviceServiceRoute
import uniffi.mobilecode_connect_mobile_core.browserProxyRouteHttpUrl

class MobileCodeConnectBrowserProxyController(
    private val tunnel: FfiMobileTunnel,
) : AutoCloseable {
    companion object {
        @JvmStatic
        fun makeBrowserProxyConfig(
            bindHost: String = "127.0.0.1",
            localPort: UShort = 0u,
            domainSuffix: String = ".mobilecode-connect.local",
            maxConnections: ULong = 256u,
            directFallbackPolicy: FfiBrowserProxyDirectFallbackPolicy = FfiBrowserProxyDirectFallbackPolicy.LOCAL_NETWORK_AND_DOMAIN,
            requestHeadTimeoutMs: ULong = 10_000u,
            directConnectTimeoutMs: ULong = 10_000u,
            tunnelOpenTimeoutMs: ULong = 15_000u,
            idleTimeoutMs: ULong = 120_000u,
        ): FfiBrowserProxyConfig =
            browserProxyConfigWithDefaults().apply {
                this.bindHost = bindHost
                this.localPort = localPort
                this.domainSuffix = domainSuffix
                this.maxConnections = maxConnections
                this.directFallbackPolicy = directFallbackPolicy
                this.requestHeadTimeoutMs = requestHeadTimeoutMs
                this.directConnectTimeoutMs = directConnectTimeoutMs
                this.tunnelOpenTimeoutMs = tunnelOpenTimeoutMs
                this.idleTimeoutMs = idleTimeoutMs
            }
    }

    private var browserProxy: FfiBrowserProxy? = null

    fun currentBrowserProxy(): FfiBrowserProxy? = browserProxy

    @Throws(FfiMobileException::class)
    fun startBrowserProxy(
        config: FfiBrowserProxyConfig = browserProxyConfigWithDefaults(),
    ): FfiBrowserProxy {
        val current = browserProxy
        if (current != null && !current.isClosed()) {
            return current
        }

        return tunnel.startBrowserProxyWithConfig(config).also {
            browserProxy = it
        }
    }

    @Throws(FfiMobileException::class)
    fun deviceServiceUrl(
        deviceId: String,
        serviceId: String,
        pathAndQuery: String = "/",
    ): String {
        val route = browserProxyDeviceServiceRoute(deviceId, serviceId)
        return browserProxyRouteHttpUrl(route, pathAndQuery)
    }

    @Throws(FfiMobileException::class)
    fun classify(
        url: String,
        controlServerUrl: String,
    ): FfiBrowserProxyUrlClassification =
        browserProxyClassifyUrlWithDefaults(url, controlServerUrl)

    @Throws(FfiMobileException::class)
    fun stats(proxy: FfiBrowserProxy = browserProxy ?: startBrowserProxy()): FfiBrowserProxyStats =
        proxy.stats()

    @Throws(FfiMobileException::class)
    fun proxyConfig(proxy: FfiBrowserProxy = browserProxy ?: startBrowserProxy()): ProxyConfig {
        val endpoint = "${proxy.host()}:${proxy.port().toInt()}"
        return ProxyConfig.Builder()
            .addProxyRule(endpoint)
            .build()
    }

    @Throws(FfiMobileException::class)
    fun applyProxy(
        executor: Executor,
        listener: Runnable = Runnable {},
        proxy: FfiBrowserProxy = browserProxy ?: startBrowserProxy(),
    ) {
        require(WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)) {
            "AndroidX WebKit PROXY_OVERRIDE is not supported by this WebView provider"
        }

        ProxyController.getInstance().setProxyOverride(
            proxyConfig(proxy),
            executor,
            listener,
        )
    }

    fun clearProxy(
        executor: Executor,
        listener: Runnable = Runnable {},
    ) {
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)) {
            return
        }

        ProxyController.getInstance().clearProxyOverride(executor, listener)
    }

    @Throws(FfiMobileException::class)
    fun closeBrowserProxy() {
        browserProxy?.let { proxy ->
            if (!proxy.isClosed()) {
                proxy.shutdown()
            }
        }
        browserProxy = null
    }

    @Throws(FfiMobileException::class)
    fun shutdown() {
        closeBrowserProxy()
        tunnel.shutdown()
    }

    @Throws(FfiMobileException::class)
    override fun close() {
        shutdown()
    }
}
