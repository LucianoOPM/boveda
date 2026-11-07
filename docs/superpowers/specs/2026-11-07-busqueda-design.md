# Diseño: búsqueda tolerante a errores (issue #8)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/8-busqueda`

## Objetivo

Agregar a `vault-core` la búsqueda que usará la pantalla principal de la app Android: la lista se filtra mientras se escribe, tolera errores y busca en servicio, dominio de la URL, usuario y etiquetas. Sin texto, muestra primero las entradas usadas recientemente (`docs/especificacion.md`, "Experiencia de uso → Pantalla principal"). El caso modelo es escribir "gm" y obtener Gmail.

La búsqueda trabaja solo sobre la caché de datos de búsqueda que `unlock()` ya descifró (`Unlocked.entries`, issue #7). No lee el disco ni descifra secretos.

### Fuera de alcance

- Búsqueda desde la PC (fase futura).
- Puntajes, campo coincidente o posiciones para resaltar en la interfaz.
- Normalización NFC, punycode (`xn--`) y quitar `www.` del dominio.
- Índices o cachés de búsqueda: con cientos de entradas basta recorrer la lista.

### Criterios de aceptación (de la issue)

- "gm" encuentra "Gmail".
- "gmial" encuentra "Gmail" mediante el respaldo.
- Busca en el dominio de la URL, no en la ruta completa.
- Encuentra entradas por usuario y por etiqueta.
- Sin texto, devuelve primero las usadas recientemente.
- No devuelve entradas eliminadas.
- Con la bóveda bloqueada devuelve `Error::Locked`.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

## Decisiones

### API

- Una sola función pública nueva, `Vault::search(&self, query: &str) -> Result<Vec<&EntrySummary>>`, ya ordenada. Toma prestado de la caché, igual que `entries()`: no copia texto plano. La fachada de `mobile/rust` convierte el resultado al cruzar a Dart.
- No expone puntajes: hoy ninguna pantalla los necesita.
- Con la bóveda bloqueada → `Error::Locked`. No hay otros errores, porque no toca el disco ni descifra nada. Ninguna variante nueva de `Error`.
- La lógica vive en `core/src/search.rs`, privado al crate, como función pura `search::search(entries: &[EntrySummary], query: &str) -> Vec<&EntrySummary>`. Se prueba sin archivos ni Argon2. Lo que usan las apps sale desde `vault` (`core/CLAUDE.md`).
- "No devuelve eliminadas" se cumple por construcción: las lápidas y las entradas dañadas nunca llegan a `Unlocked.entries` (#7). Aun así lleva su prueba.

### Campos

Cada entrada se compara contra estos textos:

- El servicio.
- El dominio de la URL, si la hay (ver abajo).
- El usuario, si lo hay.
- Cada etiqueta, por separado.

Nunca contra el secreto.

### Dominio de la URL (a mano)

`fn domain(url: &str) -> Option<&str>` devuelve un trozo de la URL original, sin reservar memoria:

1. Si contiene `://`, se descarta todo hasta ahí, incluido.
2. Se corta en el primer `/`, `?` o `#`.
3. Si queda un `@`, se conserva solo lo que va después del último. Así `usuario:clave@` nunca entra en la búsqueda.
4. Se quita el puerto: si empieza con `[` (IPv6), se conserva hasta el `]` incluido; si no, se corta en el último `:`.
5. Si el resultado está vacío, `None`.

Sin dependencia nueva: el crate `url` rechaza las URLs sin esquema (`google.com/mail`), que los usuarios escriben a menudo. No se pasa a minúsculas: nucleo ignora mayúsculas y el respaldo pliega por su cuenta.

### Sin texto

Si `query.trim()` está vacío, se devuelven todas las entradas ordenadas así:

1. `last_used_at` de mayor a menor. Las `None` (nunca usadas) van al final.
2. Empate: servicio sin distinguir mayúsculas (`to_lowercase`).

### Buscador principal: nucleo-matcher

- `Pattern::new(query, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy)`. Se usa `new` y no `parse`: con `parse`, `^ ! ' $` son operadores y escribir "!" invertiría la búsqueda. Con `new` son caracteres literales.
- `Pattern::new` parte la consulta en palabras por los espacios. Cada palabra se compara contra cada campo (`Pattern` de una sola palabra o `Atom::score` por palabra; se confirma con la documentación de la 0.3.1 al escribir el plan).
- Una entrada coincide si **cada** palabra coincide con al menos un campo. Su puntaje es la suma del mejor puntaje de cada palabra. Así "gm trab" encuentra la entrada con servicio Gmail y etiqueta "trabajo".
- `Normalization::Smart`: "telefono" encuentra "Teléfono", pero "teléfono" no encuentra "Telefono". Es el comportamiento documentado de nucleo y se acepta.
- Orden: puntaje de mayor a menor; empate por `last_used_at` (como sin texto) y luego por servicio.
- Un `Matcher::new(Config::DEFAULT)` por búsqueda, que se suelta al terminar.
- Si devuelve al menos una entrada, ese es el resultado y el respaldo no se ejecuta.

### Respaldo: strsim (`osa_distance`)

Solo si el buscador principal no encontró nada.

- **Plegado:** cada carácter pasa por `nucleo_matcher::chars::normalize` (quita acentos latinos) y luego por `to_lowercase`. Se pliegan la consulta y los campos, cada uno en un `Zeroizing<String>`.
- **Palabras de los campos:** cada campo plegado se parte por todo carácter que no sea alfanumérico (`"accounts.google.com"` da `accounts`, `google` y `com`).
- **Cada palabra de la consulta** (plegada y partida por espacios), con `n` = su número de caracteres:
  - Si `n ≥ 3`, su distancia es el mínimo, sobre todas las palabras de los campos, de `osa_distance(consulta, palabra)` y `osa_distance(consulta, inicio de palabra con n caracteres)`. El inicio cubre lo que se está escribiendo ("gmia" contra "gmai").
  - Coincide si la distancia es ≤ 1 con `3 ≤ n ≤ 5`, o ≤ 2 con `n ≥ 6`.
  - Si `n < 3`, coincide solo si nucleo la encuentra en algún campo, con distancia 0. Así "gm gmial" funciona y "gm" sola nunca activa el respaldo por sí misma.
- La entrada coincide si todas las palabras coinciden. Orden: suma de distancias de menor a mayor; empate por `last_used_at` y luego por servicio.
- OSA permite intercambiar dos letras vecinas, que es el error de "gmial".

Ejemplos (ninguno es subsecuencia del campo, así que llegan al respaldo): "gmial" → `gmail` (1); "gmia" → inicio `gmai` (1); "googel" → `google` del dominio (1); "face" contra `gmail` (4) no coincide.

Ojo al elegir ejemplos para las pruebas: un error por letra faltante ("gogle", "amzon") es subsecuencia del campo y lo encuentra nucleo, no el respaldo. Los casos del respaldo necesitan letras intercambiadas o sobrantes.

### Memoria

`ARCHITECTURE.md` pide que el texto plano dentro del núcleo viva en búferes que se borran. En la búsqueda:

- **Búferes propios, en `Zeroizing`:**
  - Las cadenas plegadas del respaldo (`Zeroizing<String>`).
  - El `Vec<char>` que pide `Utf32Str::new`, en un `Zeroizing<Vec<char>>` reservado una sola vez con la capacidad del campo más largo (en caracteres). `Utf32Str::new` hace `clear` + `extend`, y con esa capacidad nunca reasigna ni deja copias.
  - Que `char` implemente `Zeroize` se confirma al escribir el plan.
- **Limitación conocida, aceptada:** las copias internas de las librerías no se pueden borrar. Son la consulta dentro de `Pattern`, los textos que el `Matcher` copia en su búfer y los vectores que reserva `osa_distance`. Se acepta porque son datos de búsqueda, no secretos, y los mismos textos ya cruzan a Dart, donde el borrado es de mejor esfuerzo. Queda documentado en un comentario de `search.rs`, en la especificación y en `ARCHITECTURE.md`.
- La búsqueda nunca accede a `Secret`.

## Componentes

```
core/Cargo.toml              # nucleo-matcher y strsim
core/src/lib.rs              # mod search; (privado)
core/src/search.rs           # search(), domain(), plegado, nucleo y respaldo
core/src/search/tests.rs     # pruebas unitarias
core/src/vault/mod.rs        # Vault::search
core/src/vault/tests.rs      # Locked, lápidas y orden tras reveal
core/tests/vault.rs          # prueba de punta a punta
```

### Firmas

```rust
// core/src/vault/mod.rs
impl Vault {
    /// Entradas que coinciden con `query`, la mejor primero. Sin texto
    /// (vacío o solo espacios), todas: las usadas más recientemente primero.
    pub fn search(&self, query: &str) -> Result<Vec<&EntrySummary>>;
}

// core/src/search.rs (privado al crate)
pub(crate) fn search<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary>;
fn domain(url: &str) -> Option<&str>;
fn by_recent(entries: &[EntrySummary]) -> Vec<&EntrySummary>;
fn fuzzy<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary>;
fn typo_fallback<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary>;
fn fold(text: &str) -> Zeroizing<String>;
fn max_distance(chars: usize) -> Option<usize>; // None con menos de 3
```

Los nombres internos pueden ajustarse en el plan; lo fijo es la firma pública y el comportamiento.

## Pruebas

### Unitarias (`core/src/search/tests.rs`)

Sobre listas de `EntrySummary` armadas en memoria (constructor de prueba `pub(crate)` o `#[cfg(test)]`, a definir en el plan).

| Criterio | Prueba |
| --- | --- |
| Subsecuencia | "gm" encuentra "Gmail" |
| Respaldo | "gmial" y "gmia" encuentran "Gmail". Con una entrada que nucleo sí encuentra, el respaldo no agrega otras |
| Dominio | "google" encuentra `https://accounts.google.com/signin`; "signin" no |
| `domain()` | Con esquema, sin esquema, `usuario:clave@`, puerto, IPv6 con puerto, `?` y `#` sin ruta, y cadena vacía o solo esquema → `None` |
| Usuario y etiqueta | Se encuentra por cada uno; "gm trab" cruza servicio y etiqueta |
| Sin texto | `""` y `"   "`: más reciente primero, `None` al final, empate por servicio |
| Orden con texto | Igual puntaje: la usada más recientemente primero |
| Acentos | "telefono" encuentra "Teléfono" con nucleo; en el respaldo, "telefnoo" también (plegado + OSA 1) |
| Literales | "!gm" y "^gm" no fallan y no invierten la búsqueda |
| Umbrales | `n < 3` sin respaldo; distancia 1 con 5 caracteres pasa y 2 no; distancia 2 con 6 pasa y 3 no |
| No toca secretos | La contraseña de una entrada como consulta no la encuentra (prueba en `vault/tests.rs`, donde existe el secreto) |
| Robustez | `proptest`: consulta y servicio arbitrarios nunca producen panic |

### En `core/src/vault/tests.rs`

- Bloqueada: `search` → `Locked`.
- Una entrada eliminada no aparece en `search("")` ni buscándola por nombre.
- Tras `reveal` de una entrada, `search("")` la devuelve primero.
- La contraseña de una entrada como consulta no la encuentra.

### De punta a punta (`core/tests/vault.rs`)

Crear, agregar "Gmail" y otra entrada, y `search("gm")` devuelve Gmail, solo con la API pública.

## Dependencias nuevas

No son criptográficas ni de red.

- `nucleo-matcher = "0.3.1"` (MPL-2.0, ya permitida en `deny.toml`), con sus funciones predeterminadas (`unicode-normalization`, `unicode-casefold`).
- `strsim = "0.11.1"` (MIT).

Antes del commit se pasan `cargo deny check` y `cargo audit`.

## Documentación

- `docs/especificacion.md`: reemplazar "Búsqueda (pospuesta hasta su punto del roadmap)" por las reglas de este diseño: campos, dominio, nucleo, respaldo y umbrales, orden y la limitación de memoria.
- `ARCHITECTURE.md`: en la decisión "Borrado de memoria garantizado solo dentro de `vault-core`", anotar la excepción de las copias internas de la búsqueda.
- `docs/roadmap.md`: marcar la búsqueda como terminada.

## Commits previstos

1. `docs:` diseño de la búsqueda
2. `chore:` agregar `nucleo-matcher` y `strsim`
3. `feat:` extracción del dominio de la URL
4. `feat:` búsqueda sin texto por uso reciente
5. `feat:` búsqueda principal con nucleo
6. `feat:` respaldo con OSA
7. `feat:` `Vault::search` y prueba de punta a punta
8. `docs:` especificación, arquitectura y roadmap
