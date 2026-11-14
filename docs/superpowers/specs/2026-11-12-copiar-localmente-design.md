# Diseño: copiar localmente marcado como sensible (issue #42)

Fecha: 2026-11-12 · Etapa 2 · Rama `dev`

## Objetivo

Copiar desde el detalle la contraseña de una entrada al portapapeles de
Android sin mostrarla ni conservarla en el estado de Dart. Android recibe el
clip con la marca de contenido sensible. El usuario también se puede copiar,
pero como texto normal.

## Alcance

- Canal `boveda/clipboard` en Kotlin.
- Fachada Dart inyectable para las pruebas.
- Botones "Copiar" para contraseña y usuario en `EntryDetailScreen`.
- Confirmación propia hasta Android 12L; desde Android 13 se usa la del
  sistema para no duplicarla.
- Borrado condicional de contraseñas a los 30 segundos.

Fuera de alcance: copiar notas, portapapeles de Windows y la alternativa de
copiar cuando no se encuentra la PC.

## Decisiones

### Marca sensible

El clip de una contraseña lleva en `ClipDescription.extras` el booleano
`android.content.extra.IS_SENSITIVE`. Se usa la clave literal: la constante
`ClipDescription.EXTRA_IS_SENSITIVE` se agregó en API 33, pero el valor
literal es el camino documentado para compilar con SDK anteriores. La marca
es una indicación visual, no cifra ni restringe el portapapeles.

### Borrado

El plazo es 30 segundos, igual que el portapapeles de Windows. Cada clip
sensible lleva además un token interno; la tarea diferida conserva solo ese
token, no la contraseña. Al vencer, borra únicamente si el token actual sigue
siendo el de Bóveda. Desde API 28 usa `clearPrimaryClip`; en API 24 a 27 pone
un clip vacío.

Desde Android 10, leer el portapapeles sin foco puede devolver `null`. Bóveda
nunca borra a ciegas: conserva el token y vuelve a comprobar al recuperar el
foco. Si el proceso muere, el borrado propio es de mejor esfuerzo; Android 13
o posterior aporta además el borrado automático del sistema.

### Frontera del secreto

"Copiar" llama siempre a `revealPassword`, aunque la contraseña ya esté
visible, porque copiar cuenta como uso. El resultado vive solo en una variable
local mientras cruza el canal. No se asigna al `State`, a un controlador ni a
una caché. Tras cada `await` se comprueban `mounted`, la ruta activa y la
generación de la operación.

Los `String` y los búferes de `StandardMessageCodec` no se pueden borrar de
forma garantizada; quedan dentro de la excepción de mejor esfuerzo ya
documentada para secretos fuera de `vault-core`.

## Errores

- `CorruptEntry`: el aviso existente de secreto dañado.
- `Locked`: silencio; es una carrera con el bloqueo.
- `EntryNotFound`: volver a la lista.
- Otro error de Rust o del canal: "No se pudo copiar. Intenta de nuevo.".

Kotlin y Dart nunca propagan mensajes de excepción ni registran el texto.

## Pruebas

- Canal Dart con `MethodChannel` simulado y el codec real.
- Widgets con `FakeVaultSession` y `FakeLocalClipboard`.
- Copia sensible separada de mostrar, doble toque, errores y respuesta tras
  quitar la ruta.
- Compilación del APK para comprobar el código Kotlin.
- Manual: API 33 o posterior con vista previa oculta; API menor que 33 con
  aviso propio, borrado y conservación de un clip posterior.

## Fuentes

Consultadas mediante Context7 (`/websites/developer_android`): guía de
copiar y pegar, referencia de `ClipDescription`, referencia de
`ClipboardManager` y guía de manejo seguro del portapapeles.
