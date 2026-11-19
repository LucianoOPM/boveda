package com.jesus.boveda

import android.content.Intent
import android.os.Bundle
import android.view.WindowManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    private val biometric = BiometricVault(this)
    private val localClipboard = LocalClipboard(this)
    private val backup = BackupFiles(this)

    // Protección de pantalla (#43): la ventana se marca como segura antes de
    // que Flutter pinte el primer cuadro. Sin capturas ni grabación de
    // pantalla, y la miniatura de apps recientes sale sin contenido, también
    // cuando Android la toma antes de que se pinte el desbloqueo. Vale para
    // todas las variantes, incluida debug.
    override fun onCreate(savedInstanceState: Bundle?) {
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        // Temporales de respaldo (#44): al arrancar no puede haber ninguna
        // operación en curso, así que lo que quede es de un cierre forzado.
        backup.cleanTempDir()
        super.onCreate(savedInstanceState)
    }

    // Resultado del selector de archivos del respaldo (#44); lo que no es
    // suyo sigue a Flutter y a sus plugins.
    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (!backup.onActivityResult(requestCode, resultCode, data)) {
            super.onActivityResult(requestCode, resultCode, data)
        }
    }

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
        // Copia local: contraseñas marcadas como sensibles y usuario normal.
        MethodChannel(messenger, "boveda/clipboard")
            .setMethodCallHandler(localClipboard::handle)
        // Exportar e importar el respaldo con el selector del sistema.
        MethodChannel(messenger, "boveda/backup")
            .setMethodCallHandler(backup::handle)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // Si el plazo venció sin foco, ahora se puede comprobar el token sin
        // borrar un clip que otra aplicación haya puesto después. No sirve
        // `onResume`: llega antes que el foco y el portapapeles aún no se
        // puede leer.
        if (hasFocus) localClipboard.onWindowFocusGained()
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
        backup.onDestroy()
        biometric.onDestroy()
        super.onDestroy()
    }
}
