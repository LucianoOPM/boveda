# Diseño: normalizar a NFC la contraseña maestra (issue #57)

Fecha: 2026-11-18 · Etapa 2 · Rama `fix/57-nfc-contrasena-maestra`

## Objetivo

Que una misma contraseña maestra abra la bóveda sin importar cómo la escribió el teclado: `é` compuesta (U+00E9) o `e` + acento combinante (U+0065 U+0301). Hoy Argon2id recibe los bytes UTF-8 tal cual, así que cada forma deriva otra llave y una bóveda podría no abrir tras restaurar un respaldo en un celular nuevo o, en la etapa 3, desde la PC. También hace que el mínimo de 12 caracteres de `password_strength` no dependa de la forma.

La issue es prioritaria: debe resolverse antes de guardar las contraseñas reales, para no migrar bóvedas reales después.

### Fuera de alcance

- NFC de las etiquetas y demás datos de búsqueda (limitación documentada de la versión de estructura 1).
- NFKC: no se usa; cambia el significado de caracteres (por ejemplo `ﬁ` por `fi`).
- Cambiar `format_version`.

## Lo que confirma la documentación

Verificado en docs.rs y en el código fuente de la crate (Context7 no la tiene; dos búsquedas sin resultado):

- `unicode-normalization` **0.1.26** (2026-10-16), licencia `(MIT OR Apache-2.0) AND Unicode-3.0`, MSRV 1.36. `deny.toml` ya permite las tres licencias. Su única dependencia es `tinyvec` 1; las dos son nuevas en `Cargo.lock`.
- `UnicodeNormalization::nfc()` está implementado para `&str` y para cualquier `Iterator<Item = char>`; `is_nfc(&str)` existe.
- El iterador `nfc()` guarda un `TinyVec<[(u8, char); 4]>` (descomposición) y un `TinyVec<[char; 4]>` (recomposición) en la pila, y pasa a heap si un grupo de caracteres combinantes pasa de 4. No hay forma de borrarlos desde fuera.
- `is_nfc` usa el iterador solo cuando la comprobación rápida da `Maybe` (caracteres con propiedad NFC_QC=Maybe: marcas combinantes y algunos jamo coreanos). ASCII y letras precompuestas dan `Yes` sin iterador.

## Aprobado por el usuario

- Crate aprobada, fijada como `=0.1.26`.
- NFC, no NFKC.
- Un solo ayudante, usado en `kdf::derive_key` y en `password_strength`.
- Ruta rápida con `is_nfc`: una contraseña que ya está en NFC se usa sin copiarla.
- Sin `unlock` con dos intentos (NFC y luego la original): no existen bóvedas reales con contraseñas descompuestas.
- Sin cambio de `format_version`: solo cambian los bytes que recibe Argon2id para contraseñas que no están en NFC. Se registra en la especificación.
- Excepción de borrado documentada (opción 1 de la conversación): ver abajo.

## Approaches

1. **Un ayudante en `kdf::derive_key` y `password_strength`** (elegida). `derive_key` es el único punto donde la contraseña entra a Argon2id; `seal_vault_key`, `open_vault_key` y `open_vault_key_keeping_derived` pasan por él, y de ahí `create`, `unlock`, `change_master_password` (actual y nueva), `rekey` e `import`. Ninguna función de `vault/mod.rs` puede olvidarlo.
2. **Normalizar en cada función de `vault/mod.rs`.** Cinco sitios; olvidar uno deja la bóveda inconsistente. Descartada.
3. **Normalizar en Dart o en la fachada.** Rompe la regla de que la criptografía vive en `core/`. Descartada.

Para el buffer interno del iterador, las opciones eran: (1) aceptar y documentar la excepción (elegida); (2) escribir la composición NFC a mano, que es una primitiva propia con tablas Unicode, prohibida por `core/CLAUDE.md`; (3) no normalizar, que deja la #57 sin resolver.

## Diseño

`core/src/crypto/normalize.rs` (nuevo, privado al crate):

```rust
pub(crate) enum NormalizedPassword<'a> {
    Borrowed(&'a str),           // ya estaba en NFC: sin copia
    Owned(Zeroizing<String>),    // copia normalizada, se borra al soltarla
}
impl Deref for NormalizedPassword<'_> { type Target = str; }

pub(crate) fn normalize_nfc(password: &SecretString) -> NormalizedPassword<'_>;
```

- Si `is_nfc(texto)`, devuelve `Borrowed`: sin copia y, para ASCII, bytes idénticos a los de antes.
- Si no, recorre `texto.nfc()` una vez para sumar `len_utf8` y crea `String::with_capacity(total)` dentro de `Zeroizing`; una segunda pasada la llena con `extend`. Capacidad exacta, sin reasignaciones: no queda ninguna copia del secreto en un búfer anterior del `String`.
- `derive_key` usa `normalize_nfc(password)` y pasa `.as_bytes()` a Argon2id. `password_strength` cuenta `chars()` sobre la forma normalizada.
- `Debug`: `NormalizedPassword` no deriva `Debug` ni `Display`.

### Excepción de borrado (para `ARCHITECTURE.md` y `docs/especificacion.md`, junto a la de `nucleo-matcher`)

- **Qué queda:** los búferes internos del iterador `nfc()` (de 4 elementos en la pila; en el heap si un grupo combinante es más largo). La revisión final comprobó en `tinyvec` que no borra los `char` al vaciarlos, así que pueden quedar restos de varios grupos anteriores, no solo del último, y en varias zonas de la pila (cada llamada crea hasta tres iteradores). Son unos pocos caracteres, nunca la contraseña entera.
- **Cuándo:** solo con contraseñas que no están en NFC o que contienen caracteres con NFC_QC=Maybe (marcas combinantes, algunos jamo). ASCII y letras precompuestas no pasan por el iterador.
- **Por qué se acepta:** las alternativas eran una primitiva propia con tablas Unicode o dejar la #57 sin resolver.
- **Revisión:** si `unicode-normalization` ofrece borrar ese buffer, la excepción se revisa.

### Compatibilidad

`vault-v1.db` y `vault-v2.db` usan `prueba-bóveda-v1` y `prueba-bóveda-v2` con la "ó" compuesta (bytes `c3 b3`, comprobado): ya están en NFC, así que NFC no las cambia y siguen abriendo sin modificarse. Las pruebas existentes que las abren cubren esto.

## Criterios de aceptación

- [ ] Una bóveda creada con una contraseña compuesta abre con la misma escrita descompuesta, y al revés (`create`, `unlock`, `change_master_password`, `import`).
- [ ] `password_strength` da lo mismo para ambas formas.
- [ ] Una contraseña ASCII deriva la misma llave que antes del cambio (vector fijo, calculado con el código anterior y verificado con el Argon2 de Node).
- [ ] `vault-v1.db` y `vault-v2.db` siguen abriendo, sin modificarse.
- [ ] En la copia normalizada, `capacity() == len()`; ningún error ni `Debug` la muestra.
- [ ] `unicode-normalization = "=0.1.26"`; `cargo deny check` pasa con `tinyvec`.
- [ ] Decisión y excepción registradas en la especificación y en `ARCHITECTURE.md`.
- [ ] `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check` pasan; CI en verde.
