# Diferencias entre el diseño y la app

Actualizado: 2026-11-20

Lo que el diseño de Penpot y [`docs/DESIGN.md`](DESIGN.md) muestran y la app **no hace hoy**. Cada punto es un candidato a feature: se deja fuera de la restilización de las pantallas y se anota aquí para implementarlo después.

## Regla de trabajo

Al llevar una pantalla del diseño a Flutter se trabaja sobre lo que la app ya tiene: mismos textos, títulos, botones, flujos y posiciones. Lo que el diseño pide y la app no hace no se agrega ni se mueve; se escribe en este documento. Si una función de las que se listan aquí se implementa, se borra su fila y se actualiza la pantalla.

- **Diseño** es lo que muestran Penpot (página *Android*) o `docs/DESIGN.md`.
- **App hoy** es lo que hace el código en `dev`.
- **Qué haría falta** es lo mínimo que habría que construir; la especificación (`docs/especificacion.md`) manda sobre el diseño.

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
| Escala al presionar de todos los botones: 0.95, y 0.90 en los de ícono (`DESIGN.md` 6.8 y 9): engranaje, "+", botones de la barra, "Copiar", "Mostrar", "Reparar"… | Solo la tarjeta de entrada se reduce al presionarla (0.98); los botones responden al toque con la animación de tinta de Material, sin escala. | Animación de 100 ms en los botones que respete `disableAnimations`. |
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
| Orden de las filas: Usuario, Contraseña, URL, Notas (`DESIGN.md` 7.5), con las etiquetas en la cabecera. | Usuario, URL, Etiquetas, Contraseña y Notas, en ese orden; cada fila aparece solo si tiene dato (la contraseña y las notas, siempre). | Reordenar las filas (y decidir dónde van las etiquetas si hay cabecera). |
| Contraseña oculta con **doce** puntos (`DESIGN.md` 6.3). | **Ocho** puntos fijos, que no delatan el largo. | Cambiar el número de puntos. |
| Orden de las acciones de la contraseña: "Mostrar", "Copiar". | "Copiar", "Mostrar". | Cambiar el orden. |
| "Eliminar" dentro del formulario de editar, con confirmación (`DESIGN.md` 7.6); el detalle solo tiene "Editar" en la barra. | "Eliminar" está en la barra del detalle, junto a "Editar", y en el aviso de dañado. | Agregar o mover la acción al formulario (es la misma diferencia que la fila de "Eliminar entrada" del formulario). |

## Formulario de crear y editar

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Botón "Eliminar entrada", en texto rojo bajo "Guardar", con confirmación (`DESIGN.md` 7.6 y board *Formulario de entrada*). | El formulario no elimina: "Eliminar" está en la barra del detalle y en su aviso de dañado. | Agregar o mover la acción al formulario (es la misma diferencia que la fila de "Eliminar" del detalle). |
| Etiqueta de grupo "ETIQUETAS" sobre las etiquetas, con las insignias y la entrada de texto dentro de un mismo contenedor. | Las etiquetas (`InputChip`) van encima de un campo aparte con la etiqueta "AGREGAR ETIQUETA" y la ayuda "Enter o coma para agregar"; no hay etiqueta de grupo. | Unir el grupo y el campo en un solo contenedor con su etiqueta. |
| Contraseña cerrada como un campo con los puntos `••••••••••••` y el botón "Cambiar" dentro. | Una tarjeta con el texto "Contraseña: sin cambios" y el botón "Cambiar". | Solo el contenido de la tarjeta. |
| Notas cerradas con la pista "Sin notas" y el botón "Editar notas". | "Notas: sin cambios" y "Editar notas": las notas son secreto y el formulario no sabe si existen sin descifrarlas. | Un dato que diga si hay notas sin descifrarlas (cambia el núcleo y el modelo de búsqueda). |
| Orden de los campos: Servicio, Usuario, Contraseña, URL, Etiquetas, Notas. | Servicio, Usuario, URL, Etiquetas, Contraseña, Notas. | Reordenar los campos. |
| Los campos obligatorios dicen "Obligatorio" en su texto de ayuda desde el principio (`DESIGN.md` 7.6). | Sin texto de ayuda; los errores "El servicio es obligatorio" y "La contraseña es obligatoria" salen del núcleo al guardar y se muestran en su campo. | Agregar la ayuda "Obligatorio" a Servicio y a la contraseña de una entrada nueva o en reparación (en las dos la contraseña es obligatoria). |
| Botón del generador junto al campo de contraseña, que abre `EntropyGenerator` en una hoja (etapa 4). | No hay generador. | Todo el generador (D5 y la etapa 4). |
| El aviso de reparación no está dibujado (ni el board ni `DESIGN.md` 7.6 lo describen). | Un aviso con el texto "La contraseña y las notas de esta entrada están dañadas. Escribe una contraseña nueva para repararla.", en una tarjeta `errorContainer` como las demás entradas dañadas. | Dibujarlo en el diseño. |

## Ajustes y diálogo de importar

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| Encabezados de sección en `meta` y color de acento: SEGURIDAD, RESPALDOS, APARIENCIA y DISPOSITIVOS (`DESIGN.md` 7.9). | Dos filas sueltas, sin agrupar ni rotular. | Agrupar las filas bajo sus encabezados cuando haya más de dos secciones con contenido. |
| Título grande "Ajustes" dentro del cuerpo. | "Ajustes" solo en la barra superior. | Mover el título al cuerpo junto con la barra de navegación (D3). |
| "Bloqueo automático" con su valor ("Tras 1 minuto"). | La app bloquea por inactividad con un tiempo fijo (`defaultAutoLockTimeout`, 5 minutos) y no hay forma de elegirlo (etapa 4). | Pantalla o selector del tiempo y guardar la elección. |
| "Huella" con su estado ("Activada"). | La huella se activa en la oferta tras el primer uso y se usa en el desbloqueo; Ajustes no la muestra ni permite apagarla. | Fila con interruptor que active y desactive la huella. |
| "Cambiar contraseña maestra". | El núcleo tiene `change_master_password`, pero la fachada de `mobile/rust` no lo expone y no hay pantalla (etapa 4). | Exponerlo en la fachada y una pantalla para pedir la actual y la nueva. |
| Selector de acento con cuatro muestras (D2). | Cyber Lime fijo. | Ver D2 en General. |
| "Dispositivos": "PC emparejada" con el nombre de la PC (etapa 3). | No hay emparejamiento. | Todo el emparejamiento por QR (etapa 3). |
| Barra de navegación flotante (D3). | Ajustes se abre desde el engranaje del encabezado. | Ver D3 en General. |
| Texto de la advertencia del diálogo: "Esto reemplazará tu bóveda actual por la del respaldo." (`DESIGN.md` 7.9). | "Reemplaza la bóveda actual." (también es el subtítulo de la fila "Importar respaldo"). | Cambiar el texto; hoy lo buscan las pruebas de `backup_gate_test.dart`. |
| Íconos de las filas: `download` (exportar) y `refresh-cw` (importar), con una flecha a la derecha. | Se adoptaron los del diseño. La flecha es solo decoración: las dos filas abren el selector de archivos o un diálogo, no otra pantalla. El de exportar antes era una flecha de subir (`upload_file`). | Nada; si la flecha confunde, se quita. |

## App de Windows (bandeja)

| Diseño | App hoy | Qué haría falta |
| --- | --- | --- |
| El clic izquierdo en el ícono abre "Dispositivos emparejados" (`DESIGN.md` 8). | El clic izquierdo muestra el menú, igual que el derecho. Es **temporal**. | #107 implementa el comportamiento definitivo, cuando exista esa ventana. |
| "Emparejar celular" y "Dispositivos emparejados" abren sus ventanas. | Las dos opciones del menú están deshabilitadas. | Habilitarlas con sus ventanas (#101 y #107). |
| Ícono monocromo de 16 px que sigue la barra de tareas, con un punto en `accent` mientras hay una contraseña en el portapapeles. | Se usa el ícono de la app, sin versión monocroma ni punto. | Un ícono monocromo propio y el punto en `accent`. El portapapeles (#98) ya sabe cuándo hay una contraseña pendiente, pero el ícono no lo refleja todavía: queda para una issue propia. |
| Aviso de portapapeles con cuenta regresiva (ver "Ideas fuera del MVP" del roadmap). | Un toast del sistema con el texto fijo «Contraseña lista para pegar» (#98), sin cuenta regresiva. En desarrollo (`tauri dev`) Windows puede mostrarlo con el nombre de PowerShell; con la app instalada usa el `identifier` (`com.jesus.boveda`). | Una ventana o aviso propio con la cuenta regresiva, y probar el nombre del toast con el instalador (etapa 4). |
| Fuentes Plus Jakarta Sans y JetBrains Mono empaquetadas en `desktop/src/assets/fonts/` (`DESIGN.md` 3). | `tokens.css` declara las familias y cae a las del sistema; aún no hay ventanas que las muestren. | Empaquetar las fuentes y su licencia junto con la primera ventana (#101). |
