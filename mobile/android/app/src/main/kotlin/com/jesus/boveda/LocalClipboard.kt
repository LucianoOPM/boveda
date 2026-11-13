package com.jesus.boveda

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * Portapapeles local de la app.
 *
 * Las contraseñas llevan la marca de contenido sensible y un token interno
 * que permite borrarlas a los 30 segundos solo si el portapapeles todavía
 * contiene el clip de Bóveda. La tarea diferida conserva el token, nunca el
 * texto. Desde Android 10, leer el portapapeles sin foco puede devolver
 * `null`; en ese caso se vuelve a comprobar al recuperar el foco y nunca se
 * borra a ciegas contenido que el usuario pudo copiar después.
 */
class LocalClipboard(private val context: Context) {
    // La actividad todavía no tiene su Context base durante el constructor;
    // se resuelve cuando Flutter registra o usa el canal.
    private val clipboard by lazy {
        context.applicationContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    }
    private val handler = Handler(Looper.getMainLooper())

    private var nextToken = 0L
    private var activeToken: Long? = null
    private var expired = false

    fun handle(call: MethodCall, result: MethodChannel.Result) {
        val sensitive =
            when (call.method) {
                "copySensitive" -> true
                "copyPlain" -> false
                else -> {
                    result.notImplemented()
                    return
                }
            }
        val text = call.arguments as? String
        if (text == null) {
            result.error("invalid_arguments", null, null)
            return
        }

        try {
            val clip = ClipData.newPlainText("", text)
            if (sensitive) {
                val token = ++nextToken
                clip.description.extras =
                    PersistableBundle().apply {
                        // La constante se agregó en API 33, pero la clave
                        // literal es la recomendada para SDK anteriores.
                        putBoolean(SENSITIVE_KEY, true)
                        putLong(TOKEN_KEY, token)
                    }
                clipboard.setPrimaryClip(clip)
                activeToken = token
                expired = false
                handler.postDelayed({ expire(token) }, CLEAR_DELAY_MILLIS)
            } else {
                // Un clip normal reemplaza cualquier contraseña anterior;
                // su tarea diferida ya no debe tocar el portapapeles.
                clipboard.setPrimaryClip(clip)
                activeToken = null
                expired = false
            }
            // Android 13 o posterior muestra su propia confirmación.
            result.success(Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU)
        } catch (_: Exception) {
            // Nunca cruza el mensaje: podría incluir el texto copiado.
            result.error("failed", null, null)
        }
    }

    /** Vuelve a intentar un borrado vencido al recuperar el foco. */
    fun onResume() {
        clearIfExpired()
    }

    private fun expire(token: Long) {
        if (activeToken != token) return
        expired = true
        clearIfExpired()
    }

    private fun clearIfExpired() {
        val token = activeToken ?: return
        if (!expired) return
        try {
            val current =
                clipboard.primaryClipDescription?.extras?.getLong(TOKEN_KEY, MISSING_TOKEN)
                    ?: return
            if (current != token) {
                activeToken = null
                expired = false
                return
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                clipboard.clearPrimaryClip()
            } else {
                clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
            }
            activeToken = null
            expired = false
        } catch (_: Exception) {
            // Mejor esfuerzo: sin foco no se borra a ciegas. Se reintenta en
            // onResume mientras el proceso conserve el token.
        }
    }

    private companion object {
        const val SENSITIVE_KEY = "android.content.extra.IS_SENSITIVE"
        const val TOKEN_KEY = "com.jesus.boveda.CLIP_TOKEN"
        const val MISSING_TOKEN = -1L
        const val CLEAR_DELAY_MILLIS = 30_000L
    }
}
