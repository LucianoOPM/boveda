package com.jesus.boveda

import java.io.File

/**
 * `true` solo si [path] es un archivo que cuelga directamente de [dir].
 *
 * Compara rutas canónicas y no texto: así `..`, un enlace simbólico que
 * apunta fuera, el propio [dir] y un directorio hermano de nombre parecido
 * (`backup-tmp2/`) quedan rechazados. Una ruta inexistente dentro de [dir]
 * se acepta: borrarla no hace nada. No depende de Android para poder
 * probarse en la JVM (#44).
 */
fun isInsideTempDir(dir: File, path: File): Boolean {
    val parent = path.canonicalFile.parentFile ?: return false
    return parent == dir.canonicalFile
}
