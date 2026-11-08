package com.jesus.boveda

import android.app.Activity
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import androidx.annotation.RequiresApi
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.FileOutputStream
import java.security.KeyStore
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Copia de la llave de bóveda cifrada con una llave del Keystore que solo se
 * usa con huella (`BIOMETRIC_STRONG`), detrás del canal `boveda/biometric`.
 *
 * - La copia vive en `noBackupFilesDir/biometric/vault_key.bin`, fuera de
 *   Auto Backup: restaurada en otro celular no serviría, porque la llave del
 *   Keystore no viaja. Formato (61 bytes): versión (1) | IV de GCM (12) |
 *   texto cifrado (48). El texto cifrado nunca cruza a Dart.
 * - Los errores salen con `result.error(código, null, null)`, sin mensaje:
 *   ninguna excepción cruza texto. Códigos: `invalidated` (llave invalidada o
 *   inexistente, o copia ilegible; Dart desactiva la huella), `canceled`,
 *   `lockout`, `failed` y `busy`.
 * - Kotlin nunca borra la copia por su cuenta al desbloquear: Dart decide y
 *   llama a `disable`.
 * - Los arreglos con la llave se ponen en ceros apenas se entregan. Es de
 *   mejor esfuerzo, como todo borrado fuera del núcleo.
 * - Nada se escribe en logs.
 *
 * La huella solo se ofrece desde la API 30; por debajo, el canal responde
 * que no está disponible sin tocar el Keystore.
 */
class BiometricVault(private val activity: Activity) {
    private val dir get() = File(activity.noBackupFilesDir, "biometric")
    private val file get() = File(dir, "vault_key.bin")
    private val tmp get() = File(dir, "vault_key.bin.tmp")

    /** `Cipher` autenticado entre `beginEnable` y `completeEnable`/`abortEnable`. */
    private var pendingCipher: Cipher? = null

    /** Diálogo abierto: solo uno a la vez. */
    private var signal: CancellationSignal? = null

    fun handle(call: MethodCall, result: MethodChannel.Result) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            when (call.method) {
                "isAvailable", "isEnabled" -> result.success(false)
                "disable" -> {
                    deleteFiles()
                    result.success(null)
                }
                else -> result.error(FAILED, null, null)
            }
            return
        }
        when (call.method) {
            "isAvailable" -> result.success(isAvailable())
            "isEnabled" -> result.success(file.exists())
            "beginEnable" -> beginEnable(Reply(result))
            "completeEnable" -> completeEnable(call.arguments as? ByteArray, result)
            "abortEnable" -> {
                abortEnable()
                result.success(null)
            }
            "unlock" -> unlock(Reply(result))
            "disable" -> {
                deleteAll()
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /** La actividad se destruye: cierra el diálogo y descarta una activación a medias. */
    fun onDestroy() {
        signal?.cancel()
        signal = null
        if (pendingCipher != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            abortEnable()
        }
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun isAvailable(): Boolean = try {
        activity.getSystemService(BiometricManager::class.java)
            .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG) ==
            BiometricManager.BIOMETRIC_SUCCESS
    } catch (_: Exception) {
        false
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun beginEnable(reply: Reply) {
        if (signal != null) return reply.error(BUSY)
        val cipher = try {
            deleteAll()
            Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.ENCRYPT_MODE, createKey())
                updateAAD(byteArrayOf(FILE_VERSION))
            }
        } catch (_: Exception) {
            deleteAll()
            return reply.error(FAILED)
        }
        prompt("Activar huella", "Cancelar", cipher, reply, onError = { deleteAll() }) {
            pendingCipher = it
            reply.success(null)
        }
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun completeEnable(key: ByteArray?, result: MethodChannel.Result) {
        val cipher = pendingCipher
        pendingCipher = null
        try {
            if (cipher == null || key == null) throw IllegalStateException()
            val ciphertext = cipher.doFinal(key)
            if (ciphertext.size != CIPHERTEXT_LEN || cipher.iv.size != IV_LEN) {
                throw IllegalStateException()
            }
            dir.mkdirs()
            FileOutputStream(tmp).use { out ->
                out.write(byteArrayOf(FILE_VERSION))
                out.write(cipher.iv)
                out.write(ciphertext)
                out.fd.sync()
            }
            if (!tmp.renameTo(file)) throw IllegalStateException()
            result.success(null)
        } catch (_: Exception) {
            deleteAll()
            result.error(FAILED, null, null)
        } finally {
            key?.fill(0)
        }
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun abortEnable() {
        pendingCipher = null
        deleteAll()
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun unlock(reply: Reply) {
        if (signal != null) return reply.error(BUSY)
        val bytes = try {
            if (file.exists()) file.readBytes() else null
        } catch (_: Exception) {
            null
        }
        if (bytes == null || bytes.size != FILE_LEN || bytes[0] != FILE_VERSION) {
            return reply.error(INVALIDATED)
        }
        val iv = bytes.copyOfRange(1, 1 + IV_LEN)
        val ciphertext = bytes.copyOfRange(1 + IV_LEN, FILE_LEN)
        val cipher = try {
            val key = keyStore().getKey(ALIAS, null) as? SecretKey
                ?: return reply.error(INVALIDATED)
            Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, iv))
                updateAAD(byteArrayOf(FILE_VERSION))
            }
        } catch (_: KeyPermanentlyInvalidatedException) {
            return reply.error(INVALIDATED)
        } catch (_: Exception) {
            return reply.error(FAILED)
        }
        prompt("Desbloquear Bóveda", "Usar contraseña", cipher, reply, onError = {}) {
            val plain = try {
                it.doFinal(ciphertext)
            } catch (_: AEADBadTagException) {
                return@prompt reply.error(INVALIDATED)
            } catch (_: Exception) {
                return@prompt reply.error(FAILED)
            }
            // En el hilo principal (mainExecutor): ahí `success` copia los
            // bytes de forma síncrona en el mensaje, así que los ceros no
            // alcanzan la copia en tránsito. Mejor esfuerzo.
            reply.success(plain)
            plain.fill(0)
        }
    }

    /** Muestra el diálogo con el `Cipher`; `onSuccess` recibe el `Cipher` autenticado. */
    @RequiresApi(Build.VERSION_CODES.R)
    private fun prompt(
        title: String,
        negative: String,
        cipher: Cipher,
        reply: Reply,
        onError: () -> Unit,
        onSuccess: (Cipher) -> Unit,
    ) {
        val executor = activity.mainExecutor
        val cancel = CancellationSignal()
        signal = cancel
        val dialog = BiometricPrompt.Builder(activity)
            .setTitle(title)
            .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
            // En la API de la plataforma, el botón negativo llama a este
            // listener y no a `onAuthenticationError`: cuenta como cancelar.
            .setNegativeButton(negative, executor) { _, _ ->
                signal = null
                onError()
                reply.error(CANCELED)
            }
            .build()
        val callback = object : BiometricPrompt.AuthenticationCallback() {
            override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                signal = null
                val authenticated = result.cryptoObject?.cipher
                if (authenticated == null) {
                    onError()
                    reply.error(FAILED)
                } else {
                    onSuccess(authenticated)
                }
            }

            override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                signal = null
                onError()
                reply.error(codeFor(errorCode))
            }

            // Dedo no reconocido: el diálogo sigue esperando.
            override fun onAuthenticationFailed() {}
        }
        try {
            dialog.authenticate(BiometricPrompt.CryptoObject(cipher), cancel, executor, callback)
        } catch (_: Exception) {
            signal = null
            onError()
            reply.error(FAILED)
        }
    }

    private fun codeFor(errorCode: Int): String = when (errorCode) {
        BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED,
        ERROR_NEGATIVE_BUTTON,
        BiometricPrompt.BIOMETRIC_ERROR_CANCELED -> CANCELED
        BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT,
        BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT -> LOCKOUT
        // Incluye ERROR_TIMEOUT, ERROR_HW_UNAVAILABLE y ERROR_NO_BIOMETRICS:
        // nunca desactivan la huella.
        else -> FAILED
    }

    @RequiresApi(Build.VERSION_CODES.R)
    private fun createKey(): SecretKey {
        val spec = KeyGenParameterSpec.Builder(
            ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUserAuthenticationRequired(true)
            // Explícitos aunque coincidan con el valor por omisión: huella
            // fuerte en cada uso, e invalidada al registrar una huella nueva.
            .setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
            .setInvalidatedByBiometricEnrollment(true)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
            .apply { init(spec) }
            .generateKey()
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }

    /** Borra la copia, el temporal y la llave del Keystore. Nunca lanza. */
    private fun deleteAll() {
        deleteFiles()
        try {
            keyStore().deleteEntry(ALIAS)
        } catch (_: Exception) {
        }
    }

    private fun deleteFiles() {
        file.delete()
        tmp.delete()
    }

    /** Responde una sola vez al canal, aunque el diálogo avise dos veces. */
    private class Reply(private val result: MethodChannel.Result) {
        private var done = false

        fun success(value: Any?) {
            if (done) return
            done = true
            result.success(value)
        }

        fun error(code: String) {
            if (done) return
            done = true
            result.error(code, null, null)
        }
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "boveda_biometric_wrap"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val FILE_VERSION: Byte = 1
        const val IV_LEN = 12
        const val CIPHERTEXT_LEN = 48
        const val FILE_LEN = 1 + IV_LEN + CIPHERTEXT_LEN
        const val TAG_BITS = 128

        // ERROR_NEGATIVE_BUTTON (13) de AndroidX; la API de la plataforma no
        // lo publica, pero por si algún fabricante lo envía.
        const val ERROR_NEGATIVE_BUTTON = 13

        const val INVALIDATED = "invalidated"
        const val CANCELED = "canceled"
        const val LOCKOUT = "lockout"
        const val FAILED = "failed"
        const val BUSY = "busy"
    }
}
