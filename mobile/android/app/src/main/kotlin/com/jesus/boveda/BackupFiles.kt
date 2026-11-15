package com.jesus.boveda

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.util.UUID
import java.util.concurrent.Executors

/**
 * Selector de archivos del sistema (Storage Access Framework) para exportar
 * e importar el respaldo, detrás del canal `boveda/backup` (#44).
 *
 * - Dart solo maneja rutas de temporales en `noBackupFilesDir/backup-tmp/`:
 *   la copia entre la URI `content://` y el temporal es por flujo, en un hilo
 *   de fondo, y los bytes nunca cruzan a Dart.
 * - Métodos: `newTempPath`, `saveTo(temp, name)`, `pickToTemp` y
 *   `deleteTemp(path)`. Cancelar el selector no es error: `saveTo` responde
 *   `false` y `pickToTemp`, `null`.
 * - Los errores salen con `result.error(código, null, null)`, sin mensaje.
 *   Códigos: `failed`, `busy` (hay otra petición abierta), `tooLarge` (más de
 *   [MAX_BYTES] al importar) e `invalidPath` (ruta fuera de `backup-tmp/`).
 * - Si la copia falla a medias, el temporal se borra aquí antes de responder,
 *   sin depender de la interfaz. `saveTo` lo borra en todos los resultados
 *   salvo `busy` e `invalidPath`.
 * - Si el proceso muere con el selector abierto, el resultado se pierde: el
 *   temporal se limpia en el siguiente arranque ([cleanTempDir]).
 * - Nada se escribe en logs.
 */
class BackupFiles(private val activity: Activity) {
    private val dir get() = File(activity.noBackupFilesDir, "backup-tmp")
    private val executor = Executors.newSingleThreadExecutor()

    /** Petición abierta con el selector: solo una a la vez. */
    private var pending: Pending? = null

    fun handle(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "newTempPath" -> newTempPath(Reply(result))
            "saveTo" -> saveTo(call.arguments as? Map<*, *>, Reply(result))
            "pickToTemp" -> pickToTemp(Reply(result))
            "deleteTemp" -> deleteTemp(call.arguments as? String, Reply(result))
            else -> result.notImplemented()
        }
    }

    /**
     * La actividad se destruye: el hilo de la copia no sobrevive a ella. No
     * interrumpe una copia en curso (`shutdown`, no `shutdownNow`).
     */
    fun onDestroy() {
        executor.shutdown()
    }

    /** Vacía `backup-tmp/`. Se llama al arrancar, cuando no puede haber nada en curso. */
    fun cleanTempDir() {
        dir.listFiles()?.forEach { if (it.isFile) it.delete() }
    }

    private fun newTempPath(reply: Reply) {
        if (!dir.isDirectory && !dir.mkdirs()) return reply.error(FAILED)
        reply.success(File(dir, "${UUID.randomUUID()}.bak").absolutePath)
    }

    private fun saveTo(args: Map<*, *>?, reply: Reply) {
        if (pending != null) return reply.error(BUSY)
        val temp = (args?.get("temp") as? String)?.let(::File)
        val name = args?.get("name") as? String
        if (temp == null || !isInsideTempDir(dir, temp)) return reply.error(INVALID_PATH)
        if (name.isNullOrEmpty() || !temp.isFile) {
            temp.delete()
            return reply.error(FAILED)
        }
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = MIME
            putExtra(Intent.EXTRA_TITLE, name)
        }
        pending = Pending(REQUEST_SAVE, reply, temp)
        try {
            activity.startActivityForResult(intent, REQUEST_SAVE)
        } catch (_: Exception) {
            pending = null
            temp.delete()
            reply.error(FAILED)
        }
    }

    private fun pickToTemp(reply: Reply) {
        if (pending != null) return reply.error(BUSY)
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            // Sin archivos virtuales (documentos de Drive): un respaldo es un archivo real.
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
        }
        pending = Pending(REQUEST_PICK, reply, null)
        try {
            activity.startActivityForResult(intent, REQUEST_PICK)
        } catch (_: Exception) {
            pending = null
            reply.error(FAILED)
        }
    }

    private fun deleteTemp(path: String?, reply: Reply) {
        val file = path?.let(::File)
        if (file == null || !isInsideTempDir(dir, file)) return reply.error(INVALID_PATH)
        if (file.exists() && !file.delete()) return reply.error(FAILED)
        reply.success(null)
    }

    /** Resultado del selector. `true` si la petición era de este canal. */
    fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean {
        if (requestCode != REQUEST_SAVE && requestCode != REQUEST_PICK) return false
        val request = pending ?: return true
        pending = null
        val uri = if (resultCode == Activity.RESULT_OK) data?.data else null
        when (request.code) {
            REQUEST_SAVE -> finishSave(request, uri)
            else -> finishPick(request, uri)
        }
        return true
    }

    private fun finishSave(request: Pending, uri: Uri?) {
        val temp = request.temp!!
        if (uri == null) {
            temp.delete()
            return request.reply.success(false)
        }
        executor.execute {
            val saved = try {
                copyFileToUri(temp, uri)
                true
            } catch (_: Exception) {
                // El documento quedó a medias: se intenta borrarlo.
                try {
                    DocumentsContract.deleteDocument(activity.contentResolver, uri)
                } catch (_: Exception) {
                }
                false
            }
            temp.delete()
            activity.runOnUiThread {
                if (saved) request.reply.success(true) else request.reply.error(FAILED)
            }
        }
    }

    private fun finishPick(request: Pending, uri: Uri?) {
        if (uri == null) return request.reply.success(null)
        executor.execute {
            val out = File(dir, "${UUID.randomUUID()}.bak")
            val code = try {
                dir.mkdirs()
                copyUriToFile(uri, out)
                null
            } catch (_: TooLargeException) {
                TOO_LARGE
            } catch (_: Exception) {
                FAILED
            }
            if (code != null) out.delete()
            activity.runOnUiThread {
                if (code == null) request.reply.success(out.absolutePath) else request.reply.error(code)
            }
        }
    }

    private fun copyFileToUri(from: File, uri: Uri) {
        val descriptor = activity.contentResolver.openFileDescriptor(uri, "w")
            ?: throw IOException()
        descriptor.use { pfd ->
            FileOutputStream(pfd.fileDescriptor).use { output ->
                from.inputStream().use { it.copyTo(output) }
                output.flush()
                // Durabilidad como en `copyUriToFile`; algunos proveedores no
                // admiten `sync`, y eso no es un fallo de la copia.
                try {
                    output.fd.sync()
                } catch (_: IOException) {
                }
            }
        }
    }

    /** Copia con tope de [MAX_BYTES]: un archivo enorme no llena el almacenamiento interno. */
    private fun copyUriToFile(uri: Uri, out: File) {
        val input = activity.contentResolver.openInputStream(uri) ?: throw IOException()
        input.use { source ->
            FileOutputStream(out).use { output ->
                val buffer = ByteArray(BUFFER_SIZE)
                var total = 0L
                while (true) {
                    val read = source.read(buffer)
                    if (read < 0) break
                    total += read
                    if (total > MAX_BYTES) throw TooLargeException()
                    output.write(buffer, 0, read)
                }
                output.fd.sync()
            }
        }
    }

    private class TooLargeException : IOException()

    private class Pending(val code: Int, val reply: Reply, val temp: File?)

    /** Responde una sola vez al canal. */
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
        const val REQUEST_SAVE = 0x4244
        const val REQUEST_PICK = 0x4245
        const val MIME = "application/octet-stream"
        const val MAX_BYTES = 20L * 1024 * 1024
        const val BUFFER_SIZE = 64 * 1024

        const val FAILED = "failed"
        const val BUSY = "busy"
        const val TOO_LARGE = "tooLarge"
        const val INVALID_PATH = "invalidPath"
    }
}
