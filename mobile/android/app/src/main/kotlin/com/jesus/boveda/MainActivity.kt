package com.jesus.boveda

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // La bóveda vive en noBackupFilesDir: fuera de Auto Backup y de la
        // transferencia entre dispositivos. El único camino de restauración
        // es importar un respaldo.
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "boveda/storage")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "noBackupFilesDir" -> result.success(noBackupFilesDir.absolutePath)
                    else -> result.notImplemented()
                }
            }
    }
}
