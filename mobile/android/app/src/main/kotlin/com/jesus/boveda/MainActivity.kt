package com.jesus.boveda

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    private val biometric = BiometricVault(this)

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val messenger = flutterEngine.dartExecutor.binaryMessenger
        // La bóveda vive en noBackupFilesDir: fuera de Auto Backup y de la
        // transferencia entre dispositivos. El único camino de restauración
        // es importar un respaldo.
        MethodChannel(messenger, "boveda/storage")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "noBackupFilesDir" -> result.success(noBackupFilesDir.absolutePath)
                    else -> result.notImplemented()
                }
            }
        // Huella y copia de la llave de bóveda en el Keystore.
        MethodChannel(messenger, "boveda/biometric")
            .setMethodCallHandler(biometric::handle)
    }

    // La actividad deja de verse: el diálogo de la huella se cancela y Dart
    // recibe `canceled` (#39). `super.onStop()` va primero: envía a Dart el
    // aviso de ciclo de vida (`paused`, con `hidden` sintetizado) antes que
    // la respuesta `canceled`, para que la pantalla sepa que la cancelación
    // vino de ocultar la app y vuelva a pedir la huella al regresar.
    override fun onStop() {
        super.onStop()
        biometric.cancelPrompt()
    }

    override fun onDestroy() {
        biometric.onDestroy()
        super.onDestroy()
    }
}
