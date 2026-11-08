# Plan de implementación: cifrado por entrada (issue #5)

> **Para agentes:** SUB-SKILL REQUERIDA: usa superpowers:subagent-driven-development (recomendada) o superpowers:executing-plans para implementar este plan tarea por tarea. Los pasos usan casillas (`- [ ]`) para el seguimiento.

**Objetivo:** cifrar y descifrar cada parte de una entrada con XChaCha20-Poly1305. El dato autenticado de 28 bytes liga cada parte a su entrada, a su versión de estructura y a su tipo.

**Arquitectura:**
- `crypto/aead.rs` arma el dato autenticado (función privada, en la pila) y expone `seal_part`/`open_part` sobre bytes. No conoce el modelo de entrada.
- `vault/entry/sealed.rs` define `SealedEntry`: une `SearchData`/`Secret` con `aead` y convierte cualquier fallo en `CorruptEntry`.

**Tecnologías:** Rust 2024, `chacha20poly1305` 0.11 (`aead` 0.6.1), `zeroize` 1.9, `secrecy` 0.10, `proptest` (desarrollo). Sin dependencias nuevas.

**Especificación:** `docs/superpowers/specs/2026-11-07-cifrado-entrada-design.md`. Léela antes de empezar: este plan no repite sus tablas ni su flujo de datos.

## Restricciones globales

- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- Pruebas unitarias en archivo aparte: `aead.rs` → `aead/tests.rs` y `sealed.rs` → `sealed/tests.rs`, con `#[cfg(test)] mod tests;` y `use super::*;`.
- Todo es `pub(crate)`. Ningún error nuevo: los fallos de lectura son `Error::CorruptEntry` y los del generador, `Error::Random`.
- La API no hace panic: nada de `unwrap`, `expect` ni indexación que pueda fallar fuera de las pruebas.
- El texto en claro solo vive en `Zeroizing<Vec<u8>>` con la capacidad reservada de antemano; nunca en un `Vec` normal, ni siquiera en una ruta de error.
- Nonce aleatorio nuevo de 24 bytes en cada cifrado (`XNonce::try_generate()`). Nunca se reutiliza ni se deriva.
- No usar `encrypt_in_place_detached` ni `decrypt_in_place_detached` (deprecadas en `aead 0.6.1`). Usar `encrypt_inout_detached` para cifrar y `decrypt_in_place` para descifrar.
- No usar `postcard::to_extend`.
- `EncryptedPart` y `SealedEntry` no derivan `Debug`: lo implementan sin mostrar los bytes cifrados.
- Antes de cada commit: `cargo test -p vault-core` y `cargo clippy --workspace --all-targets -- -D warnings`. `cargo fmt --all` siempre.

## Foco de revisión

1. Una parte con texto en claro vacío (0 bytes) cifra y descifra bien, y mide exactamente 40 bytes. → Task 2.
2. Texto en claro grande (16384 + 64 bytes, más que unas notas al límite): la parte mide exactamente `24 + len + 16`, sin bytes de relleno. → Task 2.
3. Una parte truncada (sin su último byte) o con un byte extra al final da `CorruptEntry`, no panic. → Task 3.
4. `from_parts` con versión `0` o `u16::MAX` da `CorruptEntry` al abrir, no panic. → Task 4.
5. Abrir una entrada con la llave de otra bóveda da `CorruptEntry` en ambas partes. → Task 4.

---

### Task 1: dato autenticado y `Part`

**Archivos:**
- Crear: `core/src/crypto/aead.rs`, `core/src/crypto/aead/tests.rs`
- Modificar: `core/src/crypto/mod.rs` (agregar `pub(crate) mod aead;` y mencionar el cifrado de entradas en el comentario del módulo)

**Interfaces:**
- Produce: `pub(crate) const AAD_LEN: usize = 28`, `pub(crate) const ENTRY_ID_LEN: usize = 16`, `const AAD_LABEL: &[u8; 9] = b"boveda-v1"`, `pub(crate) enum Part { Search = 1, Secret = 2 }` (`#[repr(u8)]`, `Clone, Copy, PartialEq, Eq, Debug`), `fn associated_data(id: &[u8; ENTRY_ID_LEN], entry_schema_version: u16, part: Part) -> [u8; AAD_LEN]` (privada).

- [ ] **Paso 1: escribir las pruebas que fallan**

```rust
#[test]
fn associated_data_matches_known_vector() {
    let id: [u8; 16] = core::array::from_fn(|i| i as u8); // 00..0f
    let expected: [u8; 28] = [
        b'b', b'o', b'v', b'e', b'd', b'a', b'-', b'v', b'1',
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x01, 0x00, // versión 1, little-endian
        0x02,       // secreto
    ];
    assert_eq!(associated_data(&id, 1, Part::Secret), expected);
}

#[test]
fn associated_data_encodes_version_little_endian() {
    let ad = associated_data(&[0; 16], 0x0102, Part::Search);
    assert_eq!(&ad[25..27], &[0x02, 0x01]);
    assert_eq!(ad[27], 1);
}

#[test]
fn part_values_are_fixed() {
    assert_eq!(Part::Search as u8, 1);
    assert_eq!(Part::Secret as u8, 2);
}
```

Escribe el arreglo `expected` a mano, como arriba (desactiva `rustfmt` en esa constante con `#[rustfmt::skip]` si lo reorganiza). No lo construyas con la misma lógica que `associated_data`.

- [ ] **Paso 2: verificar que fallan**

Run: `cargo test -p vault-core crypto::aead`
Expected: error de compilación (no existen `associated_data` ni `Part`).

- [ ] **Paso 3: implementar en `core/src/crypto/aead.rs`**

- Comentario del módulo: qué hace, la tabla de offsets del dato autenticado (0–8, 9–24, 25–26, 27) y el párrafo de que el rollback de una entrada a una versión anterior queda fuera del modelo de amenazas de la v1.
- `associated_data` copia los campos en un `[u8; AAD_LEN]`. La versión va con `to_le_bytes()` y la parte con `part as u8`.

- [ ] **Paso 4: verificar que pasan**

Run: `cargo test -p vault-core crypto::aead`
Expected: 3 pruebas en verde. Clippy limpio.

- [ ] **Paso 5: commit**

```bash
git add core/src/crypto/mod.rs core/src/crypto/aead.rs core/src/crypto/aead/tests.rs
git commit -m "feat: agregar el dato autenticado de las partes de una entrada"
```

---

### Task 2: cifrar y descifrar una parte

**Archivos:**
- Modificar: `core/src/crypto/aead.rs`, `core/src/crypto/aead/tests.rs`
- Modificar: `core/src/crypto/keys.rs` (`TAG_LEN` pasa a `pub(crate)`)

**Interfaces:**
- Consume: `VaultKey`, `NONCE_LEN` y `TAG_LEN` de `crypto::keys`; de la Task 1, `associated_data` y `Part`.
- Produce:
  - `pub(crate) const MIN_SEALED_LEN: usize = NONCE_LEN + TAG_LEN` (40).
  - `pub(crate) struct EncryptedPart(Vec<u8>)` (`Clone, PartialEq, Eq`, con `Debug` propio), con `from_bytes(Vec<u8>) -> Self` y `as_bytes(&self) -> &[u8]`.
  - `pub(crate) fn seal_part(key: &VaultKey, id: &[u8; ENTRY_ID_LEN], entry_schema_version: u16, part: Part, plaintext: &[u8]) -> Result<EncryptedPart>`.
  - `pub(crate) fn open_part(key: &VaultKey, id: &[u8; ENTRY_ID_LEN], entry_schema_version: u16, part: Part, sealed: &EncryptedPart) -> Result<Zeroizing<Vec<u8>>>`.

- [ ] **Paso 1: escribir las pruebas que fallan**

Ayudante en `aead/tests.rs`: `fn key(byte: u8) -> VaultKey { VaultKey::new(Box::new([byte; 32])) }`. Constantes `ID: [u8; 16] = [7; 16]` y `V: u16 = 1`.

| Prueba | Afirmación |
| --- | --- |
| `open_returns_the_same_plaintext` | `seal_part(&key(1), &ID, V, Part::Secret, b"hola")` y luego `open_part` con los mismos datos da `*plain == b"hola"` |
| `empty_plaintext_round_trips_and_is_40_bytes` | Con `b""`: `as_bytes().len() == 40` y al abrir queda vacío |
| `sealed_length_is_exact` | Con 16448 bytes: `as_bytes().len() == 24 + 16448 + 16` |
| `ciphertext_does_not_contain_plaintext` | Con 64 × `b'A'`: `as_bytes()[24..]` no contiene la ventana `[b'A'; 16]` |
| `two_seals_differ` | Mismo contenido dos veces: los `[..24]` son distintos y las partes son distintas |
| `open_rejects_other_id` / `_other_version` / `_other_part` / `_other_key` | Abrir cambiando uno solo de los cuatro da `Err(Error::CorruptEntry)` |
| `open_rejects_short_parts` | Para longitudes `0, 1, 39, 40` (ceros): `open_part(... &EncryptedPart::from_bytes(vec![0; n]))` da `Err(Error::CorruptEntry)` |
| `debug_hides_bytes` | `format!("{:?}", part)` no contiene la representación de los primeros bytes e incluye la longitud |

- [ ] **Paso 2: verificar que fallan**

Run: `cargo test -p vault-core crypto::aead`
Expected: error de compilación (no existen `seal_part`, `open_part` ni `EncryptedPart`).

- [ ] **Paso 3: implementar en `aead.rs`**

Sigue el flujo de "Cifrar una parte" y "Descifrar una parte" de la especificación al pie de la letra:
- **`seal_part`:** usa `Zeroizing::new(Vec::with_capacity(NONCE_LEN + len + TAG_LEN))` y `encrypt_inout_detached` sobre `buf[NONCE_LEN..]`. Al final, `EncryptedPart(core::mem::take(&mut *buf))`.
- **`open_part`:** comprueba la longitud con `MIN_SEALED_LEN`, copia `[NONCE_LEN..]` a un `Zeroizing<Vec<u8>>` y aplica `decrypt_in_place`.
- **Cifrador:** se crea con `XChaCha20Poly1305::new_from_slice(key.expose_secret())`. Su error (imposible con 32 bytes) se convierte en `CorruptEntry`.
- **Errores:** cualquier error de `aead` se convierte en `CorruptEntry`, y el del nonce en `Random`.
- **`Debug`:** muestra `EncryptedPart { len: N }`.

- [ ] **Paso 4: verificar que pasan**

Run: `cargo test -p vault-core crypto`
Expected: todo en verde, incluidas las pruebas existentes de `keys`. Clippy limpio, sin avisos de funciones deprecadas.

- [ ] **Paso 5: commit**

```bash
git add core/src/crypto/
git commit -m "feat: cifrar y descifrar una parte con XChaCha20-Poly1305"
```

---

### Task 3: pruebas de manipulación y propiedades de `aead`

**Archivos:**
- Modificar: `core/src/crypto/aead/tests.rs` (módulo `props`, como en `vault/entry/tests.rs`)

**Interfaces:**
- Consume: `seal_part`, `open_part` y `EncryptedPart` (Task 2).

- [ ] **Paso 1: escribir las pruebas**

| Prueba | Afirmación |
| --- | --- |
| `round_trip_prop` | `plaintext in vec(any::<u8>(), 0..4096)`, `id in any::<[u8; 16]>()`, `version in any::<u16>()`, `part in prop_oneof![Just(Part::Search), Just(Part::Secret)]`: abrir lo cifrado devuelve `plaintext` |
| `any_bit_flip_is_detected_prop` | `plaintext in vec(any::<u8>(), 0..256)`, `pos in any::<prop::sample::Index>()`, `bit in 0u8..8`: se invierte el bit `bit` del byte `pos.index(len)` de la parte (cubre nonce, cifrado y etiqueta) y abrir da `Err(Error::CorruptEntry)` |
| `truncated_part_is_rejected` | Se quita el último byte de una parte válida: `CorruptEntry` |
| `extended_part_is_rejected` | Se agrega un `0` al final de una parte válida: `CorruptEntry` |
| `open_never_panics_prop` | `bytes in vec(any::<u8>(), 0..128)`: `open_part` sobre `EncryptedPart::from_bytes(bytes)` devuelve `Err` (solo importa que no haga panic) |

- [ ] **Paso 2: verificar**

Run: `cargo test -p vault-core crypto::aead`
Expected: todo en verde. Si una propiedad falla, es un error de la Task 2: corrígelo ahí, no ajustes la prueba.

- [ ] **Paso 3: commit**

```bash
git add core/src/crypto/aead/tests.rs core/proptest-regressions
git commit -m "test: agregar pruebas de manipulación y propiedades del cifrado de partes"
```

---

### Task 4: `SealedEntry`

**Archivos:**
- Crear: `core/src/vault/entry/sealed.rs`, `core/src/vault/entry/sealed/tests.rs`
- Modificar: `core/src/vault/entry.rs` (agregar `mod sealed;` y `pub(crate) use sealed::SealedEntry;`; actualizar el comentario del módulo, que hoy dice que el cifrado "vive en otro módulo")

**Interfaces:**
- Consume: `seal_part`, `open_part`, `EncryptedPart`, `Part` y `aead::ENTRY_ID_LEN` (Tasks 1–2); `EntryId`, `SearchData`, `Secret`, `ENTRY_SCHEMA_VERSION` y `entry::ENTRY_ID_LEN` (#4); `VaultKey`.
- Produce: `SealedEntry` con las firmas exactas de la sección "`vault/entry/sealed.rs`" de la especificación (`seal`, `from_parts`, `open_search`, `open_secret`, `id`, `entry_schema_version`, `search_bytes`, `secret_bytes`).

- [ ] **Paso 1: escribir las pruebas que fallan**

Ayudantes: `key(byte)` como en la Task 2; `sample_search()` con servicio, URL, usuario y dos etiquetas; `sample_secret()` con contraseña y notas.

| Prueba | Afirmación |
| --- | --- |
| `entry_id_len_matches_aead` | `crate::crypto::aead::ENTRY_ID_LEN == super::super::ENTRY_ID_LEN` |
| `seal_then_open_returns_same_entry` | `open_search` da `== sample_search()`; `open_secret` coincide en `password().expose_secret()` y en las notas; `entry_schema_version() == ENTRY_SCHEMA_VERSION` |
| `secret_moved_to_other_entry_is_rejected` | Se sellan A y B con ids distintos y se arma `from_parts(b.id(), V, b.search_bytes().clone(), a.secret_bytes().clone())`: `open_secret` da `CorruptEntry` y `open_search` da `Ok` |
| `swapped_parts_are_rejected` | `from_parts(id, V, secret.clone(), search.clone())`: ambos `open_*` dan `CorruptEntry` |
| `other_version_is_rejected` | `from_parts` con versión `2`, `0` y `u16::MAX`: ambos `open_*` dan `CorruptEntry` |
| `other_vault_key_is_rejected` | Se sella con `key(1)` y se abre con `key(2)`: ambos `open_*` dan `CorruptEntry` |
| `debug_hides_ciphertext` | `format!("{:?}", sealed)` no contiene el servicio, la contraseña ni los primeros bytes cifrados, y sí contiene la versión |

- [ ] **Paso 2: verificar que fallan**

Run: `cargo test -p vault-core vault::entry::sealed`
Expected: error de compilación (no existe `SealedEntry`).

- [ ] **Paso 3: implementar en `sealed.rs`**

- **Cifrar:** `seal` codifica cada parte con `encode()`, que da un búfer `Zeroizing`, y la cifra con `ENTRY_SCHEMA_VERSION` y su `Part`.
- **Descifrar:** `open_*` llama a `open_part` con `self.entry_schema_version` y luego a `decode(self.entry_schema_version, &plain)`. Cualquier error se convierte en `Error::CorruptEntry`, incluido el improbable de `open_part`.
- **`Debug`:** muestra `id`, `entry_schema_version` y las longitudes de las dos partes.

- [ ] **Paso 4: verificar que pasan**

Run: `cargo test -p vault-core`
Expected: todo en verde. Clippy limpio.

- [ ] **Paso 5: commit**

```bash
git add core/src/vault/entry.rs core/src/vault/entry/sealed.rs core/src/vault/entry/sealed/tests.rs
git commit -m "feat: agregar SealedEntry para cifrar y descifrar una entrada completa"
```

---

### Task 5: vectores de descifrado v1 y propiedades de la entrada

**Archivos:**
- Modificar: `core/src/vault/entry/sealed/tests.rs`

**Interfaces:**
- Consume: `SealedEntry` (Task 4) y `EncryptedPart::from_bytes` (Task 2).

Contenido fijo de los vectores:
- Llave: los bytes `00..1f`.
- Id: los bytes `a0..af`.
- Versión: `1`.
- Datos de búsqueda: servicio `"Correo"`, URL `"https://correo.example"`, usuario `"ana@example.com"`, etiquetas `["Trabajo", "Médico"]`.
- Secreto: contraseña `"contraseña de prueba 1"` y notas `"Notas de prueba"`.

- [ ] **Paso 1: generar los vectores una sola vez**

Agrega una prueba temporal `print_v1_vectors` que selle ese contenido con `SealedEntry::seal` e imprima en hexadecimal (`{:02x}`) `search_bytes().as_bytes()` y `secret_bytes().as_bytes()`. Ejecútala con `cargo test -p vault-core print_v1_vectors -- --nocapture`, copia las dos cadenas y **borra la prueba temporal**.

- [ ] **Paso 2: escribir la prueba de los vectores**

```rust
// Formato v1. No modificar: si esta prueba falla, cambió el formato en disco.
const V1_SEARCH_HEX: &str = "...";  // salida del paso 1
const V1_SECRET_HEX: &str = "...";

#[test]
fn v1_vectors_decrypt_to_known_content() { /* from_parts + open_search/open_secret */ }
```

- Las afirmaciones comparan cada campo con el contenido fijo de arriba. El secreto se compara con `expose_secret()`.
- Agrega en el mismo archivo un ayudante `fn hex(s: &str) -> Vec<u8>` (sin dependencias nuevas).
- `V1_SEARCH_HEX` y `V1_SECRET_HEX` quedan con las cadenas reales: el commit no puede llevar `"..."`.

- [ ] **Paso 3: agregar la propiedad de ida y vuelta**

En un módulo `props` de `sealed/tests.rs`, con su propia estrategia de texto (la de `entry/tests.rs` es privada), está `sealed_round_trip_prop`: para entradas válidas, generadas igual que en `search_data_round_trip_prop` y `secret_round_trip_prop`, `open_search(seal(x)) == x` y el secreto coincide con `expose_secret()`.

- [ ] **Paso 4: verificar**

Run: `cargo test -p vault-core vault::entry::sealed`
Expected: todo en verde. Para comprobar que la prueba de los vectores detecta cambios, cambia un carácter de `V1_SECRET_HEX`: debe fallar. Después revierte el cambio.

- [ ] **Paso 5: commit**

```bash
git add core/src/vault/entry/sealed/tests.rs core/proptest-regressions
git commit -m "test: agregar los vectores de descifrado v1 y propiedades de la entrada cifrada"
```

---

### Task 6: documentación y verificación final

**Archivos:**
- Modificar: `docs/especificacion.md` (sección "Formato de la bóveda → Entradas")
- Modificar: `ARCHITECTURE.md` (tabla de componentes de `core/`)
- Modificar: `docs/roadmap.md` (marcar "Cifrado por entrada")

- [ ] **Paso 1: actualizar la documentación**

Aplica lo que pide la sección "Documentación" de la especificación:
- Columna de offsets en la tabla del dato autenticado.
- Una parte de menos de 40 bytes o que no se autentica cuenta como entrada dañada.
- Las dos partes de una entrada se cifran siempre con la misma versión de estructura.
- Filas de `crypto/` y `vault/` en `ARCHITECTURE.md`.
- La casilla del roadmap.

- [ ] **Paso 2: verificación completa**

Run, desde la raíz:
```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo audit
cargo deny check
```
Expected: los cinco sin errores. No debe quedar ninguna prueba temporal ni un `"..."` en los vectores.

- [ ] **Paso 3: commit**

```bash
git add docs/especificacion.md ARCHITECTURE.md docs/roadmap.md
git commit -m "docs: registrar el cifrado por entrada en la especificación y la arquitectura"
```
