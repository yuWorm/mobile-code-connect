package dev.quictunnel.mobile

import android.util.Base64
import java.security.SecureRandom
import uniffi.quic_tunnel_mobile_core.FfiMobileException
import uniffi.quic_tunnel_mobile_core.FfiMobileGrantPairingOptions
import uniffi.quic_tunnel_mobile_core.FfiMobileGrantPairingPollResult
import uniffi.quic_tunnel_mobile_core.FfiMobileGrantPairingSession
import uniffi.quic_tunnel_mobile_core.FfiMobileInvitePayload
import uniffi.quic_tunnel_mobile_core.mobileGrantPairingOptionsWithDefaults
import uniffi.quic_tunnel_mobile_core.pollMobileGrantPairingOnce
import uniffi.quic_tunnel_mobile_core.startMobileGrantPairing

class QuicTunnelMobileGrantPairingController(
    private val options: FfiMobileGrantPairingOptions = mobileGrantPairingOptionsWithDefaults(),
) {
    companion object {
        private const val DEFAULT_NONCE_BYTES = 16
        private val secureRandom = SecureRandom()

        @JvmStatic
        fun makePairingOptions(
            controlRequestTimeoutMs: ULong? = 5_000u,
            controlMaxRetries: UInt = 2u,
            controlRetryBackoffMs: ULong = 100u,
        ): FfiMobileGrantPairingOptions =
            mobileGrantPairingOptionsWithDefaults().apply {
                this.controlRequestTimeoutMs = controlRequestTimeoutMs
                this.controlMaxRetries = controlMaxRetries
                this.controlRetryBackoffMs = controlRetryBackoffMs
            }

        @JvmStatic
        fun generateNonce(byteCount: Int = DEFAULT_NONCE_BYTES): String {
            require(byteCount > 0) { "byteCount must be greater than zero" }
            val bytes = ByteArray(byteCount)
            secureRandom.nextBytes(bytes)
            return Base64.encodeToString(
                bytes,
                Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP,
            )
        }
    }

    @Throws(FfiMobileException::class)
    fun start(
        invite: FfiMobileInvitePayload,
        clientId: String,
        requestedServices: List<String>,
        nonce: String = generateNonce(),
        options: FfiMobileGrantPairingOptions = this.options,
    ): FfiMobileGrantPairingSession =
        startMobileGrantPairing(
            invite = invite,
            clientId = clientId,
            requestedServices = requestedServices,
            nonce = nonce,
            options = options,
        )

    @Throws(FfiMobileException::class)
    fun pollOnce(
        pairing: FfiMobileGrantPairingSession,
        options: FfiMobileGrantPairingOptions = this.options,
    ): FfiMobileGrantPairingPollResult =
        pollMobileGrantPairingOnce(
            pairing = pairing,
            options = options,
        )
}
