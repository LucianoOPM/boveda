# Diseño de Bóveda

Actualizado: 2026-11-19

Sistema de diseño de las dos apps: Android (`mobile/`, Flutter) y Windows (`desktop/`, Tauri). Reemplaza a `docs/diseno/identidad.md`, `mobile/DESIGN.md` y `desktop/DESIGN.md`. La estética es *stealth neo-brutalista*: negro OLED, un acento eléctrico y densidad de información de herramienta de ingeniería. El comportamiento (qué hace cada pantalla y qué textos fijos usa) lo decide `docs/especificacion.md`; si este archivo y la especificación chocan, manda la especificación hasta que se actualice.

Antes de crear o cambiar una pantalla, un widget o un estilo, lee este archivo.

---

## 1. Principios

1. **Oscuridad OLED primero.** Superficies carbón y obsidiana (`#070709` a `#1D1D27`), nunca grises lavados. Reduce la fatiga y hace que los estados críticos resalten sin deslumbrar. La app tiene un solo tema, oscuro.
2. **Cero azul corporativo.** Nada del esquema "blanco + azul" de bancos y SaaS. Un acento eléctrico de alto impacto (Cyber Lime por omisión) sobre neutros.
3. **El color significa algo.** El acento marca la acción principal y el estado seguro; rojo, ámbar y verde solo aparecen para error, advertencia y éxito. Un estado nunca se comunica solo con color: va con ícono o texto.
4. **Secretos legibles.** Una contraseña revelada no es texto plano: va en monoespaciada con color por tipo de carácter, para no confundir `O`/`0` ni `l`/`1`.
5. **Un secreto nunca está a la vista por defecto.** Contraseñas y notas ocultas hasta que el usuario pide verlas.
6. **Estado transparente.** Toda acción sensible (copiar, enviar a la PC, bloquear) confirma lo que pasó con una respuesta táctil, un temporizador visible o un indicador. Nada ocurre en silencio.
7. **Segundos, no minutos.** El flujo diario (huella, buscar, enviar) no tiene pantallas ni animaciones de más.

---

## 2. Color

### 2.1 Superficies y bordes

| Token | Valor | Uso |
| --- | --- | --- |
| `bgCanvas` | `#070709` | Fondo detrás del contenido (ventanas de escritorio, márgenes) |
| `bgBase` | `#0A0A0C` | Fondo de pantalla (`scaffoldBackgroundColor`) |
| `surface1` | `#121218` | Campos de texto, contenedores de inputs, bloques de código |
| `surface2` | `#14141B` | Tarjetas de entrada en la lista (`VaultCard`) |
| `surfaceElevated` | `#1D1D27` | Estados activos, botones secundarios, diálogos y hojas |
| `iconBox` | `#0D0D12` | Caja de la inicial del servicio |
| `borderSubtle` | `#22222C` | Divisores internos |
| `borderCard` | `#242430` | Borde de tarjetas, barras flotantes y diálogos |
| `borderInteractive` | `#282836` | Borde de contenedores activos |
| `outline` | `#71717A` | Borde de campos y controles (necesario para el 3:1, ver 2.5) |
| `lockScrim` | `#09090C` | Fondo de la pantalla de desbloqueo |

### 2.2 Texto

| Token | Valor | Uso |
| --- | --- | --- |
| `textPrimary` | `#F4F4F5` | Títulos, servicio, letras de una contraseña |
| `textBody` | `#D4D4D8` | Párrafos y notas |
| `textSecondary` | `#A1A1AA` | Usuario, URL, textos de ayuda, pistas de campos |
| `textDisabled` | `#71717A` | Solo controles deshabilitados (exentos de contraste) |

### 2.3 Acentos (temas seleccionables)

Cuatro acentos intercambiables en tiempo real. Cada uno define el mismo juego de tokens; el resto de la paleta no cambia.

| Tema | `accent` | `accentDim` (fondo, 14 %) | `accentBorder` (38 %) | `onAccent` |
| --- | --- | --- | --- | --- |
| **Cyber Lime** (por omisión) | `#B5FF00` | `rgba(181, 255, 0, 0.14)` | `rgba(181, 255, 0, 0.38)` | `#08080A` |
| **Electric Emerald** | `#00F59B` | `rgba(0, 245, 155, 0.14)` | `rgba(0, 245, 155, 0.38)` | `#08080A` |
| **Solar Amber** | `#FF9F1C` | `rgba(255, 159, 28, 0.14)` | `rgba(255, 159, 28, 0.38)` | `#08080A` |
| **Neon Crimson** | `#FF2A85` | `rgba(255, 42, 133, 0.15)` | `rgba(255, 42, 133, 0.42)` | `#08080A` |

- Neon Crimson usa `#08080A` sobre el acento, no blanco: el blanco da 3.55:1 y no pasa ni AA.
- Con Solar Amber o Neon Crimson el acento se parece a `warning` o a `error`. Por eso los estados nunca dependen solo del color (principio 3).
- `accentGlow`: el acento al 28 %, solo para el resplandor de la acción principal (ver 4.4).

### 2.4 Estados y sintaxis de contraseñas

| Token | Valor | Uso |
| --- | --- | --- |
| `error` | `#F87171` | Texto e íconos de error: contraseña incorrecta, entradas dañadas, eliminar |
| `errorStrong` | `#EF4444` | Rellenos: barra del temporizador en sus últimos 5 s, cápsula "BLOQUEADA", segmento de "Muy corta" |
| `errorContainer` | `#450A0A` al 20 % sobre `surface2` | Fondo de avisos de error (`DamagedEntriesBanner`) |
| `warning` | `#FBBF24` | Contraseña aceptable, advertencias no bloqueantes |
| `success` | `#4ADE80` | Enviado a la PC, respaldo exportado, contraseña fuerte |
| `lan` | `#F59E0B` | Estado de la PC emparejada (etapa 3) |
| `syntaxLetter` | `#F4F4F5` | `A-Z`, `a-z` y cualquier otra letra |
| `syntaxDigit` | `#FBBF24` | `0-9` |
| `syntaxSymbol` | `#EC4899` | Todo lo que no es letra, dígito ni espacio |
| `syntaxSpace` | `#A1A1AA` | Un espacio se dibuja como `␣` para que se vea |

`warning`, `success` y `lan` tienen también contenedor: el mismo tono al 14 % sobre `surface2`, con el texto en el color sólido.

### 2.5 Contraste comprobado (WCAG 2.2)

| Par | Contraste |
| --- | --- |
| `textPrimary` / `bgBase` | 18.00 |
| `textBody` / `bgBase` | 13.38 |
| `textSecondary` / `surface2` | 7.15 |
| `onAccent` / `accent` (Lime · Emerald · Amber · Crimson) | 16.46 · 13.85 · 9.75 · 5.64 |
| `accent` / `surface2` (Lime · Emerald · Amber · Crimson) | 15.07 · 12.69 · 8.93 · 5.17 |
| `error` / `surfaceElevated` | 6.04 |
| `error` / `errorContainer` | 6.57 |
| `warning` / `surfaceElevated` | 10.01 |
| `success` / `surfaceElevated` | 9.59 |
| `syntaxDigit` / `surface1` | 11.18 |
| `syntaxSymbol` / `surfaceElevated` | 4.74 |
| `outline` / `surface1` (no texto) | 3.86 |

Reglas:

- Todo texto cumple 4.5:1 y todo borde que identifica un control, 3:1. Tres de los cuatro acentos llegan a AAA (7:1) para texto sobre el acento; Neon Crimson se queda en AA.
- Los bordes `borderCard` y `borderInteractive` (1.2–1.4:1) son decorativos: separan tarjetas, no identifican controles. Un campo de texto lleva `outline` en reposo y `accent` de 2 px con foco.
- `errorStrong` (`#EF4444`) no se usa para texto pequeño sobre `surfaceElevated` (4.44:1); para texto va `error`.
- Cualquier color nuevo se agrega a esta tabla con su contraste medido.

---

## 3. Tipografía

| Rol | Familia | Por qué |
| --- | --- | --- |
| Interfaz y títulos | **Plus Jakarta Sans** | Geométrica humanista: compacta y legible en tamaños pequeños |
| Secretos, métricas, identificadores, insignias | **JetBrains Mono** | Distingue `0`/`O` y `1`/`l`/`I`; da el tono técnico |

- Las fuentes se empaquetan en cada app (`mobile/assets/fonts/`, `desktop/src/assets/fonts/`); nunca se descargan en tiempo de ejecución (nada de `google_fonts` ni CDN). Ambas son OFL 1.1 y su licencia se registra en `LicenseRegistry`.
- Números tabulares (`FontFeature.tabularFigures()`) en contadores y temporizadores, para que no bailen.
- Todos los tamaños escalan con el tamaño de texto del sistema.

| Estilo | Familia | Tamaño / peso | Tracking | Uso |
| --- | --- | --- | --- | --- |
| `brand` | Plus Jakarta Sans | 16 / 800 | -0.02em | Marca en el encabezado |
| `headlineSmall` | Plus Jakarta Sans | 22 / 800 | -0.02em | Títulos de primer uso y desbloqueo |
| `titleLarge` | Plus Jakarta Sans | 18 / 700 | -0.01em | Servicio en el detalle |
| `titleMedium` | Plus Jakarta Sans | 14 / 700 | 0 | Servicio en la tarjeta, títulos de sección |
| `bodyLarge` | Plus Jakarta Sans | 16 / 400 | 0 | Texto que se escribe en los campos |
| `bodyMedium` | Plus Jakarta Sans | 13 / 400, interlineado 1.5 | 0 | Párrafos y notas reveladas |
| `bodySmall` | Plus Jakarta Sans | 12 / 400, interlineado 1.5 | 0 | Usuario, URL y textos de ayuda |
| `labelLarge` | Plus Jakarta Sans | 14 / 700 | 0 | Botones |
| `secret` | JetBrains Mono | 17 / 700 | 0.05em | Contraseña revelada o generada, con salto en cualquier carácter |
| `metric` | JetBrains Mono | 15 / 800 | 0.15em | Valores grandes: bits de entropía, temporizador del QR |
| `meta` | JetBrains Mono | 10–11 / 500–700, MAYÚSCULAS | 0.08em | Insignias, etiquetas, temporizadores, estado del encabezado |

`brand`, `secret`, `metric` y `meta` viven en un `ThemeExtension<BovedaText>`; el resto, en el `TextTheme` de Material.

---

## 4. Forma, espacio y profundidad

### 4.1 Espaciado

Escala base de 4: `4, 8, 10, 12, 14, 16, 24, 32, 48`. Margen lateral de pantalla 16; separación entre tarjetas 10; relleno interno de tarjetas 14.

### 4.2 Radios

| Radio | Uso |
| --- | --- |
| 16 | Tarjetas, diálogos, hojas inferiores |
| 12 | Botones, campos de texto, contenedores internos |
| 6–8 | Insignias, chips de etiqueta |
| completo | Cápsula de estado, barras de progreso, barra de navegación, indicadores |

### 4.3 Profundidad

Sin sombras grises difusas. La profundidad sale de:

- Bordes de 1 px bien definidos (`borderCard`).
- Desenfoque de fondo (`BackdropFilter`, sigma 24, equivalente a `backdrop-blur-xl`) solo en barras flotantes.
- Resplandor direccional (ver 4.4) solo en la acción principal.

### 4.4 Resplandor

`BoxShadow(color: accentGlow, blurRadius: 25)` en el botón principal de cada pantalla, el botón "+" y el indicador de la pestaña activa. En nada más: si dos cosas brillan, ninguna es la principal.

### 4.5 Maquetas

Las maquetas usan un marco de 390 × 820 con esquinas de 54 y pantalla interna de 44. Es solo el marco de presentación: la app se diseña y se prueba desde 360 dp de ancho.

---

## 5. Base técnica

### 5.1 Android (Flutter)

- Material 3 con un `ColorScheme` oscuro armado a mano con los tokens. No se usa `ColorScheme.fromSeed` ni color dinámico.
- Un solo tema: `darkTheme` y `themeMode: ThemeMode.dark`.
- Todo vive en `lib/theme/`:
  - `colors.dart`: los tokens de la sección 2 como constantes (`BovedaColors`).
  - `accents.dart`: `enum BovedaAccent { cyberLime, electricEmerald, solarAmber, neonCrimson }` con sus cuatro tokens.
  - `theme.dart`: `buildTheme(BovedaAccent)` arma el `ThemeData`.
  - `extensions.dart`: `ThemeExtension<StatusColors>` (`success`, `warning`, `lan`, `errorStrong` y contenedores), `ThemeExtension<SyntaxColors>` y `ThemeExtension<BovedaText>`.
  - `spacing.dart`: la escala (`Space.xs = 4` … `Space.xxl = 48`) y los radios (`Radii.card = 16`…).
- Los widgets leen todo del tema (`Theme.of(context).colorScheme`, `textTheme`, `extension<…>()`). Nunca un `Color(0x…)` ni un `TextStyle` con tamaño fijo fuera de `lib/theme/`.
- Íconos: Material Symbols Outlined, trazo uniforme, el mismo ícono para la misma acción en toda la app.

### 5.2 Windows (Tauri)

- Tokens como variables CSS en `src/styles/tokens.css` (`--bg-base`, `--accent`…), con los mismos nombres. La PC usa Cyber Lime fijo: el acento elegido en el celular no viaja a la PC.
- Ningún color ni tamaño literal fuera de `tokens.css`. La CSP de `tauri.conf.json` no permite orígenes externos.
- Íconos: Fluent System Icons (SVG empaquetados).

---

## 6. Componentes (Android)

### 6.1 Encabezado y cápsula de estado (`CryptoHeader`)

- Fila superior fija en `bgBase`, alto 56, margen 16.
- **Izquierda:** marca `BÓVEDA` en `brand` y, debajo, `XCHACHA20-POLY1305 • ARGON2ID` en `meta`, `textSecondary`.
- **Cápsula de estado:** píldora `#000000` con borde `#24242F` y un LED de 6 px:
  - Bóveda abierta: LED en `accent` con pulso y texto `CONOCIMIENTO CERO` en `accent`.
  - Bloqueada: LED en `errorStrong` fijo y texto `BLOQUEADA` en `error`.
  - Android dibuja su propia barra de estado; la cápsula es parte de la app, no imita la Dynamic Island.
- **Derecha (etapa 3):** botón de estado de la PC: ícono de escritorio, `PC` y la última dirección conocida o `SIN PC` en `meta`, color `lan`. Toca para ir a la pestaña PC. El punto de pulso solo aparece mientras un envío está en curso: el celular no mantiene una conexión abierta con la PC (la busca al enviar), así que no hay latencia en vivo que mostrar.
- Hasta la etapa 3, en esa esquina va el ícono de Ajustes.

### 6.2 Tarjeta de entrada (`VaultCard`)

- Contenedor `surface2`, borde `borderCard`, radio 16, relleno 14, separación 10. Presionada: borde `#52525B` y escala 0.98.
- **Izquierda:** caja `iconBox` de 40 × 40, radio 12, con la inicial del servicio en `metric` y `accent`. Sin favicons ni íconos de categoría: descargarlos revelaría a qué servicios tienes cuenta.
- **Centro:**
  - Línea 1: servicio en `titleMedium`, truncado.
  - Línea 2: hasta dos etiquetas como insignias (`meta`, fondo `accentDim`, borde `accentBorder`, texto `accent`, radio 6), separadas por ` // `; el resto, "+N".
  - Línea 3: usuario o, si no hay, la URL tal como está guardada, en `bodySmall` y `textSecondary`.
- **Derecha (etapa 3):** acción rápida "Enviar a PC" (ícono, 40 × 40, `surfaceElevated`, `active` a escala 0.90). Pide huella como todo envío.
- Deslizar a la izquierda envía a la PC (etapa 4): fondo `accent` con el ícono de enviar en `onAccent`.
- Presionar la tarjeta abre el detalle.

### 6.3 Contraseña con sintaxis (`SecretText`)

- Oculta: siempre `••••••••••••` (12 puntos fijos, no revelan el largo) en `secret` y `textSecondary`. Lector de pantalla: "Contraseña oculta".
- Revelada: `Text.rich` con un `TextSpan` por tramo de caracteres del mismo tipo, en `secret` y los colores de 2.4, con salto en cualquier carácter. Nunca `SelectableText`: seleccionar permitiría copiar sin la marca de sensible.
- Fondo `surface1`, radio 12, relleno 14.

### 6.4 `PasswordField` (existe)

- Fondo `surface1`, radio 12, borde `outline` de 1 px en reposo y `accent` de 2 px con foco.
- Texto en `secret` (sin colores de sintaxis mientras se escribe), ojo al final para mostrar u ocultar, empieza oculto.
- Conserva lo que ya hace: no deja copiar ni cortar, sin sugerencias ni aprendizaje del teclado.

### 6.5 `StrengthIndicator` (existe)

Barra de tres segmentos (alto 4, radio completo, separación 4) y un texto en `meta`. El nivel viene de `password_strength` del núcleo.

| `PasswordStrength` | Segmentos | Color | Texto |
| --- | --- | --- | --- |
| `TooShort` | 1 | `errorStrong` | `MUY CORTA: MÍNIMO 12 CARACTERES` |
| `Acceptable` | 2 | `warning` | `ACEPTABLE` |
| `Strong` | 3 | `success` | `FUERTE` |

Con el campo vacío, los tres segmentos en `borderInteractive` y sin texto.

### 6.6 `DamagedEntriesBanner` (existe)

Tarjeta `errorContainer`, borde `error` al 38 %, radio 16, ícono de advertencia en `error`, texto "N entradas no se pudieron leer" en `textPrimary` y botón de cerrar. Una vez por desbloqueo, sin bloquear nada.

### 6.7 Barra de búsqueda

- `surface1`, borde `outline`, radio 12, alto 52; lupa al inicio y botón de borrar con texto. Con foco: borde `accent` de 2 px.
- Pista: "Buscar servicio, usuario o etiqueta".
- No se enfoca al llegar: el teclado sale cuando el usuario la toca (especificación, #40).

### 6.8 Botones

- **Principal:** `FilledButton` en `accent` con texto `onAccent`, alto 52, radio 12, resplandor (4.4), ancho completo en formularios. Uno por pantalla.
- **Secundario:** fondo `surfaceElevated`, borde `borderInteractive`, texto `textPrimary`.
- **Destructivo:** `TextButton` en `error`; la confirmación es un diálogo cuyo botón destructivo también va en `error`.
- **Botón "+":** 56 × 56, radio 16, `accent` con resplandor, sobre la barra de navegación a la derecha.
- Todos responden al toque con escala 0.95 (0.90 los de ícono) en 100 ms.

### 6.9 Aviso de portapapeles (`ClipboardToast`)

- Aparece al copiar una contraseña, 16 px sobre la barra de navegación (o sobre el borde inferior si no hay barra).
- Fondo `surfaceElevated` al 85 % con desenfoque, borde `borderCard`, radio 16.
- Texto "Contraseña copiada" y, en `meta` con números tabulares, `SE BORRA EN 30 S`, que baja cada segundo. Debajo, una barra de progreso de radio completo en `accent` que pasa a `errorStrong` con 5 s o menos.
- Botón `BORRAR AHORA` en `accent`: borra el portapapeles solo si el token del clip sigue siendo el de Bóveda, igual que el borrado automático.
- Nunca muestra la contraseña. Desaparece al llegar a 0, al borrar o al bloquear.
- Hasta Android 12L es el aviso de copia de la app. Desde Android 13 el sistema ya avisa; ver la decisión pendiente D4.

### 6.10 Barra de navegación flotante (`FloatingGlassNav`)

- Cápsula a 12 px del borde inferior, alto 64, radio completo, `#12121A` al 85 % con desenfoque y borde `borderCard`.
- Pestañas: **Bóveda**, **Generador** (etapa 4), **PC** (etapa 3) y **Ajustes**. Cada una con ícono y etiqueta en `meta`; la inactiva en `textSecondary`, la activa en `accent`.
- Indicador activo: línea de 24 × 2 en el borde superior de la pestaña, en `accent` con resplandor.
- Las pestañas viven en un `IndexedStack` dentro de la misma ruta, sin `Navigator` anidado: el bloqueo vacía el `Navigator` raíz y uno anidado no se limpiaría.
- La barra entra cuando haya al menos tres destinos (etapa 3). Hasta entonces, Ajustes sigue en el encabezado.

### 6.11 Diálogos, hojas y avisos

- Diálogos en `surfaceElevated`, borde `borderCard`, radio 16, título en pregunta ("¿Eliminar Gmail?") y una línea de consecuencia ("No se puede deshacer.").
- `SnackBar` flotante en `surfaceElevated` con borde `borderCard`, 4 s. Nunca contiene un secreto.

---

## 7. Pantallas (Android)

### 7.1 Primer uso (`first_run_screen.dart`)

- De arriba abajo: cápsula de estado, título "Crea tu contraseña maestra", párrafo corto, `PasswordField`, `StrengthIndicator`, confirmación, advertencia y el botón "Crear bóveda".
- La advertencia va en contenedor de `warning` con ícono: "Si la olvidas, no hay forma de recuperarla ni de abrir tus contraseñas." Se ve sin desplazarse en 360 × 640 dp con el teclado cerrado.
- "Crear bóveda" se habilita con `Acceptable` o `Strong` y la confirmación igual. Mientras se calibra Argon2id: indicador en el botón, "Creando bóveda…" y campos deshabilitados.

### 7.2 Oferta de huella (`biometric_offer_screen.dart`)

Ícono de huella de 64 en `accent` dentro de un círculo `accentDim`, título "¿Desbloquear con huella?", una línea de explicación, "Activar huella" (principal) y "Ahora no" (`TextButton`).

### 7.3 Desbloqueo (`unlock_screen.dart`, `LockOverlay`)

- Pantalla completa en `lockScrim` sólido. No es un velo desenfocado sobre la bóveda: al bloquear, el contenido sale del árbol sin animación para que no se vea en la transición.
- Centro: caja de 72 × 72 radio 16 con un candado en `accent`, título "Bóveda bloqueada" y la cápsula `BLOQUEADA`.
- `PasswordField` y botón "Desbloquear". Debajo, el botón de huella: 64 × 64, radio completo, `surfaceElevated` con borde `accentBorder`, ícono de huella en `accent` y la etiqueta `HUELLA • KEYSTORE` en `meta`.
- Casilla "Desbloquear con huella la próxima vez" cuando el dispositivo lo admite y no está activa.
- Con la huella activa, el diálogo del sistema aparece al abrir; si se cancela, queda esta pantalla.
- Errores debajo del campo, en `error`, con ícono: "Contraseña incorrecta" o "No se pudo desbloquear la bóveda. Intenta de nuevo.".
- Huella desactivada: contenedor de `warning`, "La huella se desactivó. Desbloquea con tu contraseña para volver a activarla.".
- Avisos de un solo uso (`UnlockNotice`): contenedor de `error` o `warning` según el aviso.
- Mientras corre Argon2id: indicador en el botón, sin tapar la pantalla.

### 7.4 Bóveda (pantalla principal, `search_screen.dart`)

- `CryptoHeader`, barra de búsqueda fija, aviso de dañadas si aplica y la lista de `VaultCard`.
- Sin texto: primero las usadas recientemente (orden del núcleo).
- Sin resultados: lupa en `textSecondary` y "Nada coincide con «texto»".
- Bóveda vacía: "Todavía no hay contraseñas" y "Agregar la primera".
- Botón "+" para crear.

### 7.5 Detalle de entrada (`entry_detail_screen.dart`)

- Encabezado: caja de inicial de 56, servicio en `titleLarge`, etiquetas como insignias y dominio en `bodySmall`.
- Filas en tarjetas `surface2`: Usuario, Contraseña, URL, Notas. Etiqueta arriba en `meta` y `textSecondary`, valor abajo, acción a la derecha.
- Contraseña: `SecretText` oculto, con "Mostrar" y "Copiar". Notas: "Mostrar notas" (sin notas, "Sin notas"). Revelar no se oculta por tiempo; sigue visible hasta ocultar o salir.
- Acción principal al pie: "Copiar contraseña"; desde la etapa 3, "Enviar a PC" y copiar pasa a la fila.
- Editar en la barra superior.
- Secreto dañado: tarjeta de `errorContainer` con "La contraseña y las notas de esta entrada están dañadas" y las acciones "Reparar" y "Eliminar".

### 7.6 Crear y editar (`entry_form_screen.dart`)

- Campos en `surface1` con borde `outline`, en este orden: Servicio, Usuario, Contraseña, URL, Etiquetas, Notas. Los obligatorios dicen "Obligatorio" en el texto de ayuda.
- Etiquetas como insignias (6.2) con botón de quitar; Intro o coma agregan.
- Al editar, la contraseña empieza cerrada con "Cambiar" y las notas con "Editar notas".
- El error del núcleo se muestra en su campo, en `error`.
- Guardar con el botón principal; salir con cambios pregunta "¿Descartar cambios?". Eliminar, con confirmación, dentro de este formulario.
- Etapa 4: botón del generador junto al campo de contraseña, que abre `EntropyGenerator` en una hoja.

### 7.7 Generador (`EntropyGenerator`, etapa 4)

- **Selector doble** (control segmentado, `surfaceElevated`, segmento activo en `accent`): *Caracteres* (8–64) y *Frase* (3–8 palabras).
- **Visor:** la contraseña generada en `SecretText` revelado, sobre `surface1`, con botón de regenerar que gira 180° en 200 ms.
- **Controles:** deslizador de longitud o de palabras y casillas de tipos de carácter (mayúsculas, minúsculas, dígitos, símbolos).
- **Barra de entropía:** bits en `metric` (`86 BITS`), barra de radio completo de 52 a 142+ bits en `errorStrong` (menos de 60), `warning` (60–79) y `accent` (80 o más), y una línea con el tiempo estimado de fuerza bruta en `meta`.
- "Usar esta contraseña" como principal cuando se abre desde el formulario; "Copiar" cuando se abre desde la pestaña.
- La generación y el cálculo de bits salen del núcleo (CSPRNG y fórmula a partir de los parámetros), no de Dart.

### 7.8 PC (etapa 3)

- Tarjeta de la PC emparejada: nombre, `EMPAREJADA EL 8 NOV 2026` y `ÚLTIMO ENVÍO HACE 3 MIN` en `meta`, color `lan` en el ícono.
- "Emparejar PC" (principal) abre el escáner del QR dentro de `runWithoutAutoLock`.
- Errores fijos: "No se encontró la PC. ¿Están en la misma red?" con "Copiar en este celular".

### 7.9 Ajustes

- Secciones con encabezado en `meta` y `accent`: **Seguridad** (bloqueo automático, huella, cambiar contraseña maestra), **Respaldos** (exportar, importar), **Apariencia** (acento) y, desde la etapa 3, **Dispositivos**.
- Acento: cuatro muestras circulares de 40 con el nombre del tema; la elegida lleva anillo de 2 px en `textPrimary`. Se guarda fuera de la bóveda (no es secreto) y se aplica antes del desbloqueo.
- Importar avisa antes: "Esto reemplazará tu bóveda actual por la del respaldo.".

---

## 8. App de Windows (satélite de escritorio)

Mismos tokens, en oscuro y con Cyber Lime fijo. Casi nunca muestra una ventana.

- **Bandeja:** ícono monocromo de 16 px que sigue la barra de tareas; con una contraseña en el portapapeles, un punto en `accent` en la esquina, que desaparece al borrarse a los 30 s. Menú nativo: Emparejar celular, Dispositivos emparejados, Iniciar con Windows (con marca) y, tras un separador, Salir. Clic izquierdo abre "Dispositivos emparejados".
- **Emparejar celular** (360 × 480, fija): instrucción, el QR, el temporizador en `metric` ("CADUCA EN 1:45", pasa a `warning` en los últimos 20 s) y "Cancelar". El QR es siempre de módulos `#0F1720` sobre blanco puro, 240 × 240 con zona de silencio de 4 módulos, dentro de una tarjeta blanca de radio 16: un QR invertido falla en muchos lectores. Al caducar se atenúa al 20 % y aparece "Generar otro código".
- **Confirmación** (360 × 200): "¿Emparejar con [celular]?", la huella corta de la llave en `secret`, "Emparejar" (principal) y "Rechazar", que tiene el foco por defecto.
- **Dispositivos emparejados** (420 × 360): una tarjeta `surface2` por celular con nombre, fechas en `meta` y "Quitar" en `error`, con confirmación. Vacía: "Todavía no hay celulares emparejados".
- **Notificación del sistema** al recibir: "Contraseña lista para pegar", "Se borrará del portapapeles en 30 s" y el nombre del celular. Nunca la contraseña ni el servicio.
- **Teclado:** todo se usa con teclado, Esc cierra, foco visible con anillo de 2 px en `accent`. Respeta `prefers-reduced-motion` y `forced-colors`. Objetivos de clic de 32 × 32 o más.

---

## 9. Movimiento y respuesta táctil

| Qué | Duración | Detalle |
| --- | --- | --- |
| Presionar | 100 ms | Escala 0.95 (0.90 en botones de ícono) |
| Mostrar u ocultar, regenerar | 100–200 ms | Giro de 180° del botón de regenerar |
| Cambiar de pantalla o pestaña, abrir una hoja | 200 ms | Curva estándar, sin rebotes ni resortes |
| Cambiar de acento | 500–700 ms | Interpolación de color del acento y su resplandor |
| Bloquear | 0 ms | Corte directo al desbloqueo |

- **Indicadores vivos:** los temporizadores (portapapeles, QR) se actualizan cada 1000 ms. El pulso del LED de la cápsula es continuo con la bóveda abierta; el de la PC, solo durante un envío.
- **Respuesta háptica:** `HapticFeedback.lightImpact` al copiar y al confirmar un envío; `HapticFeedback.heavyImpact` en un error de desbloqueo.
- **Reducir movimiento:** con `MediaQuery.disableAnimations`, sin pulsos ni escalas, cambio de acento instantáneo y transiciones como fundidos de 100 ms.

---

## 10. Textos

- Español neutro, tuteo, frases cortas en voz activa. Las insignias y metadatos van en MAYÚSCULAS y en `meta`; los botones y párrafos, en minúsculas normales.
- El botón dice la acción ("Enviar a PC", "Desbloquear", "Exportar respaldo"), no "Aceptar".
- Los textos fijos de la especificación se respetan tal cual: "Contraseña incorrecta", "No se pudo desbloquear la bóveda. Intenta de nuevo.", "N entradas no se pudieron leer", "La contraseña y las notas de esta entrada están dañadas", "No se encontró la PC. ¿Están en la misma red?", "Contraseña lista para pegar", "No se pudo copiar. Intenta de nuevo.", "¿Descartar cambios?" y los errores de validación del formulario.
- Sin signos de exclamación salvo en celebraciones contadas (primer respaldo hecho).

---

## 11. Seguridad visible

- `FLAG_SECURE` en toda la app: sin capturas y contenido oculto en apps recientes.
- Ninguna pantalla muestra un secreto al abrirse; los revelados no son seleccionables.
- Al copiar, el clip lleva la marca de sensible y vence a los 30 s (ver 6.9).
- Al bloquear, se vuelve al desbloqueo sin animación.
- Sin favicons ni peticiones de red que no sean el envío a la PC.

---

## 12. Accesibilidad

- Contraste según 2.5.
- Áreas táctiles de 48 × 48 dp o más, aunque el ícono sea menor.
- Todo funciona con el texto del sistema al 200 %: las líneas largas se envuelven o se truncan, nunca los botones. Con texto grande, la barra de navegación oculta las etiquetas y deja los íconos.
- `Semantics` en tarjetas (servicio, usuario, "doble toque para abrir"), botones de ícono, cápsula de estado ("Bóveda abierta" o "Bóveda bloqueada"), indicador de fortaleza ("Fortaleza: aceptable") y temporizador del portapapeles (anuncia solo al aparecer y al borrarse, no cada segundo).
- Un secreto oculto se anuncia como "Contraseña oculta"; nunca se lee sin que el usuario lo muestre.
- Probar con TalkBack, fuente al 200 % y un teléfono de 360 dp.

---

## 13. Pruebas de diseño

- Pruebas de widgets para cada estado descrito (vacío, sin resultados, error, cargando, dañadas) con los cuatro acentos.
- Una prueba que recorra los cuatro `ThemeData` y compruebe que ningún par de 2.5 baje de su mínimo.
- Una prueba de `SecretText` con letras, dígitos, símbolos, espacios y caracteres fuera de ASCII.

---

## 14. Fuera del MVP

Partes del sistema de referencia que no entran porque la especificación las deja fuera o no las cubre. Se diseñan aquí cuando se decidan.

- **TOTP** (código de 6 dígitos con barra del ciclo de 30 s en la tarjeta).
- **Auditoría (Watchtower):** índice de salud y aviso de contraseñas débiles. Medir la entropía de las contraseñas guardadas exige descifrarlas todas.
- **PIN de 4 dígitos:** el desbloqueo es con contraseña maestra o huella (`BIOMETRIC_STRONG`, sin credencial del dispositivo).

---

## 15. Decisiones que cambian la especificación

Este diseño da por hechas estas decisiones. Hay que registrarlas en `docs/especificacion.md` (o descartarlas y ajustar este archivo) antes de implementarlas.

- **D1. Solo tema oscuro.** Antes se seguía el tema del sistema.
- **D2. Acento seleccionable** en Ajustes (cuatro temas, guardado fuera de la bóveda). Antes era un color fijo.
- **D3. Barra de navegación con pestañas** desde la etapa 3, con una pestaña propia para el generador.
- **D4. Aviso de portapapeles con cuenta regresiva y "Borrar ahora".** Desde Android 13 la especificación usa solo el aviso del sistema para no duplicarlo. Opciones: mostrar el `ClipboardToast` en todas las versiones (y en 13+ sin la frase "Contraseña copiada", solo el temporizador), o dejarlo solo hasta 12L. "Borrar ahora" necesita un método nuevo en el canal `boveda/clipboard`.
- **D5. Generador con modo frase** (3–8 palabras en español). Necesita una lista de palabras en el núcleo y decidir cuál.
- **D6. Acción rápida "Enviar a PC" en la tarjeta**, además de deslizar.
