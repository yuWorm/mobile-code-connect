package dev.mobilecode.connect.mobile

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.mobilecode_connect_mobile_core.FfiMobileException
import uniffi.mobilecode_connect_mobile_core.FfiMobileGrantCredential
import uniffi.mobilecode_connect_mobile_core.mobileGrantCredentialFromJson
import uniffi.mobilecode_connect_mobile_core.mobileGrantCredentialToJson

class MobileCodeConnectMobileGrantSecureStore(
    context: Context,
    private val keyAlias: String = "dev.mobilecode.connect.mobile.mobile_grant",
    preferencesName: String = "dev.mobilecode.connect.mobile.grants",
    private val credentialKey: String = "default",
) {
    private val preferences =
        context.applicationContext.getSharedPreferences(preferencesName, Context.MODE_PRIVATE)

    @Throws(FfiMobileException::class, GeneralSecurityException::class)
    fun save(grant: FfiMobileGrantCredential) {
        val json = mobileGrantCredentialToJson(grant)
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        val ciphertext = cipher.doFinal(json.toByteArray(Charsets.UTF_8))

        preferences.edit()
            .putString(ciphertextPreferenceKey(), encode(ciphertext))
            .putString(ivPreferenceKey(), encode(cipher.iv))
            .apply()
    }

    @Throws(FfiMobileException::class, GeneralSecurityException::class)
    fun load(): FfiMobileGrantCredential? {
        val encodedCiphertext = preferences.getString(ciphertextPreferenceKey(), null) ?: return null
        val encodedIv = preferences.getString(ivPreferenceKey(), null) ?: return null
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(
            Cipher.DECRYPT_MODE,
            secretKey(),
            GCMParameterSpec(GCM_TAG_BITS, decode(encodedIv)),
        )
        val json = String(cipher.doFinal(decode(encodedCiphertext)), Charsets.UTF_8)
        return mobileGrantCredentialFromJson(json)
    }

    fun clear() {
        preferences.edit()
            .remove(ciphertextPreferenceKey())
            .remove(ivPreferenceKey())
            .apply()
    }

    @Synchronized
    @Throws(GeneralSecurityException::class)
    private fun secretKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getKey(keyAlias, null) as? SecretKey)?.let { return it }

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                keyAlias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .build(),
        )
        return generator.generateKey()
    }

    private fun ciphertextPreferenceKey(): String = "$credentialKey.ciphertext"

    private fun ivPreferenceKey(): String = "$credentialKey.iv"

    private fun encode(bytes: ByteArray): String =
        Base64.encodeToString(bytes, Base64.NO_WRAP)

    private fun decode(value: String): ByteArray =
        Base64.decode(value, Base64.NO_WRAP)

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val GCM_TAG_BITS = 128
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}
