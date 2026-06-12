import Foundation
import Security

public enum MobileCodeConnectMobileGrantSecureStoreError: Error, Equatable {
    case invalidStoredCredential
    case keychainFailed(OSStatus)
}

public final class MobileCodeConnectMobileGrantSecureStore {
    public let service: String
    public let account: String
    public let accessGroup: String?

    public init(
        service: String = "dev.mobilecode.connect.mobile.mobile-grant",
        account: String = "default",
        accessGroup: String? = nil
    ) {
        self.service = service
        self.account = account
        self.accessGroup = accessGroup
    }

    public func save(_ grant: FfiMobileGrantCredential) throws {
        let json = try mobileGrantCredentialToJson(grant: grant)
        guard let data = json.data(using: .utf8) else {
            throw MobileCodeConnectMobileGrantSecureStoreError.invalidStoredCredential
        }

        try clear()
        var query = baseQuery()
        query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        query[kSecValueData as String] = data

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw MobileCodeConnectMobileGrantSecureStoreError.keychainFailed(status)
        }
    }

    public func load() throws -> FfiMobileGrantCredential? {
        var query = baseQuery()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw MobileCodeConnectMobileGrantSecureStoreError.keychainFailed(status)
        }
        guard
            let data = item as? Data,
            let json = String(data: data, encoding: .utf8)
        else {
            throw MobileCodeConnectMobileGrantSecureStoreError.invalidStoredCredential
        }
        return try mobileGrantCredentialFromJson(json: json)
    }

    public func clear() throws {
        let status = SecItemDelete(baseQuery() as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw MobileCodeConnectMobileGrantSecureStoreError.keychainFailed(status)
        }
    }

    private func baseQuery() -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }
        return query
    }
}
