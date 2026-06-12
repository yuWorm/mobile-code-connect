import Foundation
import Security

public enum QuicTunnelMobileGrantPairingError: Error, Equatable {
    case invalidNonceByteCount(Int)
    case nonceGenerationFailed(OSStatus)
}

public final class QuicTunnelMobileGrantPairingController {
    public var options: FfiMobileGrantPairingOptions

    public static func makePairingOptions(
        controlRequestTimeoutMs: UInt64? = 5_000,
        controlMaxRetries: UInt32 = 2,
        controlRetryBackoffMs: UInt64 = 100
    ) -> FfiMobileGrantPairingOptions {
        var options = mobileGrantPairingOptionsWithDefaults()
        options.controlRequestTimeoutMs = controlRequestTimeoutMs
        options.controlMaxRetries = controlMaxRetries
        options.controlRetryBackoffMs = controlRetryBackoffMs
        return options
    }

    public static func generateNonce(byteCount: Int = 16) throws -> String {
        guard byteCount > 0 else {
            throw QuicTunnelMobileGrantPairingError.invalidNonceByteCount(byteCount)
        }
        var bytes = [UInt8](repeating: 0, count: byteCount)
        let status = bytes.withUnsafeMutableBytes { buffer -> OSStatus in
            guard let baseAddress = buffer.baseAddress else {
                return errSecParam
            }
            return SecRandomCopyBytes(kSecRandomDefault, buffer.count, baseAddress)
        }
        guard status == errSecSuccess else {
            throw QuicTunnelMobileGrantPairingError.nonceGenerationFailed(status)
        }

        return Data(bytes)
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    public init(options: FfiMobileGrantPairingOptions = mobileGrantPairingOptionsWithDefaults()) {
        self.options = options
    }

    public func start(
        invite: FfiMobileInvitePayload,
        clientId: String,
        requestedServices: [String],
        nonce: String? = nil,
        options explicitOptions: FfiMobileGrantPairingOptions? = nil
    ) throws -> FfiMobileGrantPairingSession {
        let resolvedNonce = nonce ?? (try Self.generateNonce())
        return try startMobileGrantPairing(
            invite: invite,
            clientId: clientId,
            requestedServices: requestedServices,
            nonce: resolvedNonce,
            options: explicitOptions ?? options
        )
    }

    public func pollOnce(
        pairing: FfiMobileGrantPairingSession,
        options explicitOptions: FfiMobileGrantPairingOptions? = nil
    ) throws -> FfiMobileGrantPairingPollResult {
        try pollMobileGrantPairingOnce(
            pairing: pairing,
            options: explicitOptions ?? options
        )
    }
}
