# Diseño: la llave derivada vive en el heap (issue #54)

Fecha: 2026-11-18 · Etapa 2 · Rama `fix/54-llave-derivada-en-heap`

## Objetivo

Que la llave derivada de la contraseña maestra no deje copias sin borrar por los movimientos de nuestro código. Hoy `DerivedKey = Zeroizing<[u8; 32]>` es un arreglo en la pila: `Zeroizing` solo borra la ubicación final, y cada movimiento por valor (`derive_key` la devuelve, `open_vault_key_keeping_derived` la mete en una tupla, `unlock_upgrading` la recibe) puede dejar una copia en el marco anterior. Eso contradice la regla de `core/CLAUDE.md` y de `ARCHITECTURE.md` (evitar reasignaciones y movimientos que dejen copias de un secreto).

### Fuera de alcance

- Cambiar el formato, la derivación o el comportamiento: solo cambian los tipos y dónde viven los búferes.
- Una primitiva propia que evite las copias internas de las dependencias (prohibida por `core/CLAUDE.md`).

## Lo que confirma la documentación y el código

Context7 (`secrecy` 0.10.3, `argon2`) y lectura del código de `argon2` 0.6.0 y `chacha20poly1305` 0.11.0:

- `SecretBox<S>`: `Default` crea `Box::<S>::default()` (en el heap, con ceros: no es un secreto), `init_with_mut(|&mut S| ...)` inicializa en el sitio y `expose_secret_mut()` da `&mut S`. Mover un `SecretBox` copia un puntero. No tiene `Deref` ni `Clone`; su `Debug` no muestra el valor.
- `Argon2::hash_password_into(pwd, salt, out: &mut [u8])` escribe el resultado directamente en `out`. Con la feature `zeroize` (activa en `core/Cargo.toml`) borra `initial_hash` y los bloques de trabajo. **Pero** `blake2b_long` (`argon2-0.6.0/src/blake2b_long.rs:31-34`) arma el hash de 64 bytes en un arreglo de la pila, `full_out`, y copia los 32 primeros a `out`; `full_out` no se borra.
- `ChaChaPoly1305` guarda la llave por valor dentro de la estructura (`key: Key`; `new` hace `*key`) y se borra al soltarse (feature `zeroize`, activa). Mover el cifrador puede dejar copias.

## Decisiones (aprobadas por el usuario)

- `DerivedKey = SecretBox<[u8; 32]>`, igual que `VaultKey`. `Zeroizing<Box<[u8; 32]>>` se descartó: su `Deref` permite `*key` y copiar el arreglo sin darse cuenta.
- `derive_key` inicializa con `SecretBox::init_with_mut` y escribe la salida de Argon2id directamente en el heap. `init_with_mut` no devuelve `Result`: el error de `hash_password_into` se captura en una variable externa y se devuelve después; el `SecretBox` se suelta (y se borra) sin copiar nada a otro lugar.
- `encrypt_vault_key`: el búfer de trabajo, que recibe la llave de bóveda en claro y se cifra en el sitio, pasa a `Zeroizing<[u8; 48]>`. Solo el texto cifrado se copia al `EncryptedKey`. Así, un fallo del cifrado (inalcanzable en la práctica) no deja la llave en claro.
- Se eliminan los ayudantes que devuelven el cifrador por valor (`cipher` y `vault_key_cipher` en `keys.rs`, `cipher` en `aead.rs`): el cifrador se crea y se usa en la misma función y se suelta ahí. Cada sitio lleva una línea con su propio error (`WrongPassword`, `WrongVaultKey`, `CorruptEntry`), así que no hace falta un ayudante con closure. **Esfuerzo razonable, sin garantía**: el compilador decide los movimientos; no se agregan pruebas que pretendan verificarlo.
- Prueba determinista `derived_and_vault_keys_are_pointer_sized`: `size_of::<DerivedKey>()` y `size_of::<VaultKey>()` valen `size_of::<usize>()`. Garantiza que el tipo es un puntero y que mover no copia la llave; **no prueba que no haya copias en la pila**, que se sigue cubriendo con la revisión de código.
- Las features `zeroize` de `argon2` y `chacha20poly1305` ya están activas en `core/Cargo.toml`: no hay que cambiarlas.

## Excepción documentada (`ARCHITECTURE.md` y `docs/especificacion.md`, junto a las de `nucleo-matcher` y `nfc()`)

- **`argon2` 0.6.0:** `full_out`, un arreglo de 64 bytes en la pila de `blake2b_long`, contiene el hash completo (los 32 primeros bytes son la llave derivada) y no se borra. Con una salida de 32 bytes no hay ruta de la API que lo evite.
- **`chacha20poly1305` 0.11.0:** `ChaChaPoly1305` guarda la llave por valor dentro de la estructura; se borra al soltarla, pero un movimiento del cifrador puede dejar copias. Reducimos los movimientos nuestros, sin garantía.
- **Cuándo se revisa:** al actualizar cualquiera de las dos versiones.
- **Por qué no se evita:** solo se cierra con una primitiva propia (Argon2id o ChaCha20-Poly1305 con búferes que se borran), y `core/CLAUDE.md` la prohíbe.

## Approaches

1. **`SecretBox<[u8; 32]>`** (elegida): mismo tipo que `VaultKey`, un movimiento copia un puntero, el acceso es explícito.
2. `Zeroizing<Box<[u8; 32]>>`: la misma garantía de heap, pero el `Deref` facilita copiar el arreglo por error.
3. Dejarlo como está y documentarlo: no cumple el objetivo de la issue.

## Criterios de aceptación

- [ ] `DerivedKey` vive en el heap y Argon2id escribe directamente en ese búfer; la copia interna de `argon2` queda documentada.
- [ ] Revisión de código: ningún secreto de tamaño fijo se mueve por valor como arreglo en la pila dentro de `crypto/` (con las excepciones documentadas de las dependencias).
- [ ] Sin ayudantes que devuelvan el cifrador por valor.
- [ ] Las pruebas existentes de `kdf`, `keys`, `aead` y la cabecera pasan sin cambios de comportamiento; los vectores de `derive_key` (ASCII y `café`) no cambian.
- [ ] Prueba de tamaño de puntero para `DerivedKey` y `VaultKey`.
- [ ] `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check` pasan; CI en verde.
