# Diferencias entre el diseño y la app

Actualizado: 2026-11-19

Lo que el diseño de Penpot y [`docs/DESIGN.md`](DESIGN.md) muestran y la app **no hace hoy**. Cada punto es un candidato a feature: se deja fuera de la restilización de las pantallas y se anota aquí para implementarlo después.

## Regla de trabajo

Al llevar una pantalla del diseño a Flutter se trabaja sobre lo que la app ya tiene: mismos textos, títulos, botones, flujos y posiciones. Lo que el diseño pide y la app no hace no se agrega ni se mueve; se escribe en este documento. Si una función de las que se listan aquí se implementa, se borra su fila y se actualiza la pantalla.

- **Diseño** es lo que muestran Penpot (página *Android*) o `docs/DESIGN.md`.
- **App hoy** es lo que hace el código en `dev`.
- **Qué haría falta** es lo mínimo que habría que construir; la especificación (`docs/especificacion.md`) manda sobre el diseño.

Pantallas todavía sin restilizar (sus diferencias se agregarán cuando se hagan): formulario de crear y editar, Ajustes y diálogo de importar.

## General

Son las decisiones que `DESIGN.md` (sección 15) da por hechas y que cambian la especificación o el comportamiento.

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| **D2.** Acento seleccionable en Ajustes (cuatro temas: Cyber Lime, Electric Emerald, Solar Amber y Neon Crimson). | Cyber Lime fijo. Los cuatro acentos existen en `lib/theme/accents.dart` y las pruebas los recorren, pero no hay selector. | Sección "Apariencia" en Ajustes con las cuatro muestras, guardar la elección fuera de la bóveda (no es secreto) y aplicarla antes del desbloqueo. |
| **D3.** Barra de navegación flotante con pestañas Bóveda, Generador, PC y Ajustes (etapa 3). | Pantalla principal sola; Ajustes se abre desde el engranaje del encabezado. | Las pestañas en un `IndexedStack` dentro de la misma ruta (el bloqueo vacía el `Navigator` raíz); entra con al menos tres destinos. |
| **D4.** Aviso de portapapeles con cuenta regresiva y "Borrar ahora" (`ClipboardToast`). | Al copiar, Android 13 o posterior usa el aviso del sistema; la app borra la contraseña a los 30 s. | Mostrar el aviso en todas las versiones y un método nuevo en el canal `boveda/clipboard` para "Borrar ahora". Ver D4 en `DESIGN.md`. |
| **D5.** Generador de contraseñas con modo frase (3 a 8 palabras en español). | No hay generador (etapa 4). | Generador en el núcleo y una lista de palabras en español. |
| **D6.** Acción rápida "Enviar a PC" en la tarjeta, además de deslizar para enviar (etapas 3 y 4). | La tarjeta solo abre el detalle. | Todo el envío a la PC (etapa 3) y el gesto de deslizar (etapa 4). |
| Pulso animado del LED de la cápsula de estado con la bóveda abierta (`DESIGN.md` 6.1 y 9). | El LED es fijo. | Animación continua que respete `MediaQuery.disableAnimations`. |
| Escala al presionar del engranaje (0.90) y del botón "+" (0.95), `DESIGN.md` 6.8 y 9. | Responden al toque sin escala. | Animación de 100 ms que respete `disableAnimations`. |
| Respuesta háptica: ligera al copiar y al confirmar un envío; fuerte en un error de desbloqueo (`DESIGN.md` 9). | La app no usa `HapticFeedback`. | Llamadas a `HapticFeedback` en esos tres momentos. |

## Desbloqueo

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Casilla "Desbloquear con huella la próxima vez" (`DESIGN.md` 7.3). | "Usar huella para desbloquear". | Solo el texto. |
| Etiqueta `HUELLA • KEYSTORE` bajo el botón de huella. | `USAR HUELLA`. | Solo el texto. |

## Primer uso

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Aviso de advertencia con ícono y **sin casilla**: "Crear bóveda" se habilita con la fortaleza aceptable y la confirmación igual (`DESIGN.md` 7.1). | La casilla "Entiendo que si olvido la contraseña maestra…" es obligatoria para crear; va dentro del aviso. | Decidir si la casilla se quita (cambia el comportamiento y la especificación). |
| Párrafo "Es la única llave de tu bóveda. Se usa para cifrar todo en este celular." | "Usa una frase de varias palabras. Mínimo 12 caracteres." | Solo el texto. |
| Textos de fortaleza en mayúsculas: `MUY CORTA: MÍNIMO 12 CARACTERES`, `ACEPTABLE`, `FUERTE`. | "Demasiado corta", "Aceptable" y "Fuerte", en `meta` y sin mayúsculas. | Solo los textos. |
| El aviso y el botón "Crear bóveda" se ven sin desplazar en 360 × 640 (`DESIGN.md` 7.1). | Con el formulario vacío y sin barras del sistema solo cabe la advertencia (180 dp, por la casilla); "Crear bóveda" queda por debajo y la pantalla se desplaza. | Quitar la casilla, o recortar espaciados. |
| No hay "Restaurar un respaldo". | "Restaurar un respaldo" (para un celular nuevo o un emulador limpio, #44). | Decidir dónde vive en el diseño. |

## Oferta de huella

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Título "¿Desbloquear con huella?", botón "Activar huella" y el párrafo "Abre tu bóveda con la huella del teléfono. Tu contraseña maestra seguirá funcionando.". | "¿Activar huella?", "Activar" y el párrafo "Desbloquea la bóveda con tu huella. La contraseña maestra seguirá funcionando y la necesitarás si registras una huella nueva.". | Solo los textos. |

## Bóveda (pantalla principal)

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Pista del campo "Buscar servicio, usuario o etiqueta". | "Buscar". | Solo el texto. |
| Estado vacío "Todavía no hay contraseñas". | "Todavía no hay contraseñas guardadas. Toca + para agregar la primera.", más el botón "Agregar la primera". | Solo el texto. |
| Sin resultados: "Nada coincide con «x»". | "Sin resultados para «x»". | Solo el texto. |
| Aviso de entradas dañadas con un botón de cerrar. | Botón de texto "Entendido". | Cambiar el botón por un ícono de cerrar. |
| A 390 dp o más, el subtítulo `XCHACHA20-POLY1305 • ARGON2ID` va bajo la marca, junto a la cápsula y el engranaje (board *Bóveda*). | A 360 dp el subtítulo va en su propia línea, bajo la fila; con texto del sistema por encima del 110 % la cápsula también pasa a su línea. | Ninguno si se acepta; o un diseño del encabezado para 360 dp. |
| Acción "Enviar a PC" en la tarjeta y deslizar para enviar. | Solo abre el detalle. | Ver D6. |

## Detalle de entrada

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Título de la barra "Detalle" y un bloque de cabecera: caja de inicial de 56, servicio en `titleLarge`, etiquetas como insignias y el dominio de la URL. | El título de la barra es el nombre del servicio; las etiquetas son una fila más. No hay bloque de cabecera ni dominio. | Decidir la cabecera y derivar el dominio de la URL guardada. |
| Botón principal al pie "Copiar contraseña" (hasta la etapa 3; luego "Enviar a PC", y copiar pasa a la fila). | "Copiar" solo está en la fila de la contraseña y en la del usuario. | Agregar el botón al pie; ver D6 para "Enviar a PC". |
| "Sin notas" visible antes de revelar las notas. | "Sin notas" aparece solo al revelarlas: las notas son secreto y la lista y el detalle no saben si existen sin descifrarlas. | Un dato nuevo en la lista o en el detalle que diga si hay notas, sin descifrarlas (cambia el núcleo y el modelo de búsqueda). |
| Contraseña oculta con **doce** puntos (`DESIGN.md` 6.3). | **Ocho** puntos fijos, que no delatan el largo. | Cambiar el número de puntos. |
| Orden de las acciones de la contraseña: "Mostrar", "Copiar". | "Copiar", "Mostrar". | Cambiar el orden. |
| "Eliminar" dentro del formulario de editar, con confirmación (`DESIGN.md` 7.6); el detalle solo tiene "Editar" en la barra. | "Eliminar" está en la barra del detalle, junto a "Editar", y en el aviso de dañado. | Mover la acción al formulario. |
