//! Almacenamiento SQLite de la bóveda: una tabla de cabecera con una sola
//! fila y una tabla de entradas.
//!
//! Este módulo solo mueve bytes y fechas: no cifra, no descifra y no conoce
//! la llave. Tampoco lee el reloj: cada escritura recibe la hora (`now`, en
//! milisegundos Unix) de quien llama.
//!
//! Los errores de SQLite se traducen en [`map_sqlite_error`] y se descartan:
//! pueden traer SQL, rutas o valores de columnas.
//!
//! Todas las rutas deben ser absolutas ([`require_absolute`]): el SQLite
//! empaquetado se compila con `SQLITE_USE_URI`, así que una ruta que empiece
//! con `file:` se interpretaría como URI (con parámetros como `?mode=` o
//! `?vfs=`), aunque la conexión no use `SQLITE_OPEN_URI`. Las comprobaciones
//! previas de Rust tocarían el archivo literal y SQLite abriría otro. Una
//! ruta absoluta no puede empezar con `file:`.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior, params};

use super::entry::sealed::SealedEntry;
use super::entry::{ENTRY_ID_LEN, EntryId};
use super::header::{FORMAT_VERSION, FORMAT_VERSION_V1, FORMAT_VERSION_V2, VaultHeader};
use super::paired_pc::{PAIRED_PC_ID_LEN, PairedPcId};
use crate::crypto::aead::EncryptedPart;
use crate::crypto::kdf::{KdfParams, SALT_LEN};
use crate::crypto::keys::{ENCRYPTED_KEY_LEN, EncryptedKey, KEY_CHECK_LEN, NONCE_LEN};
use crate::error::{Error, Result, StorageErrorKind};

/// Identifica el archivo como bóveda ("BVLT"). Fijo para siempre, aunque
/// cambie el formato: la versión es solo `format_version` de la cabecera.
///
/// No es un control de seguridad, porque no está autenticado.
pub(crate) const APPLICATION_ID: i32 = 0x4256_4C54;

/// Columnas de la tabla `header` de la versión de formato 2. Se escriben una
/// sola vez porque las usan `SCHEMA` y la migración desde la versión 1
/// (que crea `header_v2` y la renombra): así el texto de ambas es el mismo.
macro_rules! header_v2_columns {
    () => {
        "(
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    format_version  INTEGER NOT NULL,
    kdf_algorithm   INTEGER NOT NULL,
    kdf_memory_kib  INTEGER NOT NULL,
    kdf_iterations  INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    salt            BLOB NOT NULL CHECK (length(salt) = 16),
    key_nonce       BLOB NOT NULL CHECK (length(key_nonce) = 24),
    encrypted_key   BLOB NOT NULL CHECK (length(encrypted_key) = 48),
    key_check       BLOB NOT NULL CHECK (length(key_check) = 40)
) STRICT"
    };
}

/// Columnas de la tabla `identity` de la versión de formato 3. Las usan
/// `SCHEMA` y la migración desde la versión 2, así que el texto es el mismo.
macro_rules! identity_columns {
    () => {
        "(
    id                      INTEGER PRIMARY KEY CHECK (id = 1),
    identity_schema_version INTEGER NOT NULL,
    data                    BLOB NOT NULL CHECK (length(data) = 72)
) STRICT"
    };
}

/// Columnas de la tabla `paired_pcs` de la versión de formato 3. Ver
/// [`identity_columns`].
macro_rules! paired_pcs_columns {
    () => {
        "(
    id                BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
    pc_schema_version INTEGER NOT NULL,
    data              BLOB NOT NULL CHECK (length(data) >= 40)
) STRICT, WITHOUT ROWID"
    };
}

/// Las dos tablas que agrega la versión de formato 3. Las crean `SCHEMA` y
/// la migración desde la versión 2.
macro_rules! pairing_tables {
    () => {
        concat!(
            "CREATE TABLE identity ",
            identity_columns!(),
            ";
CREATE TABLE paired_pcs ",
            paired_pcs_columns!(),
            ";
",
        )
    };
}

/// Esquema de la versión de formato 3. El de la versión 1 (sin `key_check`)
/// está en la especificación y en `tests/fixtures/vault-v1.db`; la versión 2
/// es la 3 sin `identity` ni `paired_pcs`.
///
/// `"header"` va entre comillas para coincidir con lo que deja
/// `ALTER TABLE ... RENAME` en la migración desde la versión 1: SQLite
/// reescribe el nombre entre comillas en `sqlite_schema`. La prueba de
/// igualdad de `sqlite_schema` entre una bóveda migrada y una creada lo
/// comprueba.
const SCHEMA: &str = concat!(
    "CREATE TABLE \"header\" ",
    header_v2_columns!(),
    ";\n",
    "
CREATE TABLE entries (
    id                   BLOB PRIMARY KEY NOT NULL,
    entry_schema_version INTEGER NOT NULL,
    search               BLOB,
    secret               BLOB,
    modified_at          INTEGER NOT NULL,
    last_used_at         INTEGER,
    deleted              INTEGER NOT NULL DEFAULT 0,
    CHECK ((deleted = 0 AND search IS NOT NULL AND secret IS NOT NULL)
        OR (deleted = 1 AND search IS NULL AND secret IS NULL))
) STRICT, WITHOUT ROWID;
",
    pairing_tables!(),
);

/// Resultado de [`Storage::migrate_header`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Migration {
    /// La cabecera era de la versión 1 y quedó en la 2.
    Done,
    /// La cabecera ya no era de la versión 1 al abrir la transacción (otra
    /// conexión la migró): no se tocó nada.
    NotNeeded,
}

/// Conexión a la bóveda. No deriva `Debug`.
pub(crate) struct Storage {
    conn: Connection,
}

impl Storage {
    /// Crea un archivo de bóveda nuevo con su esquema y su cabecera.
    ///
    /// Falla con `Error::AlreadyExists` si la ruta ya existe, sin tocarla.
    /// Si algo falla después de crear el archivo, lo borra. Una ruta
    /// relativa da `Storage(Io)` sin crear nada.
    pub(crate) fn create(path: &Path, header: &VaultHeader) -> Result<Self> {
        require_absolute(path)?;
        // `create_new` falla de forma atómica si la ruta ya existe.
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| match e.kind() {
                io::ErrorKind::AlreadyExists => Error::AlreadyExists,
                _ => Error::Storage(StorageErrorKind::Io),
            })?;

        match Self::initialize(path, header) {
            Ok(storage) => Ok(storage),
            Err(e) => {
                // La conexión ya se soltó dentro de `initialize`: en Windows
                // no se puede borrar un archivo abierto.
                let _ = fs::remove_file(path);
                Err(e)
            }
        }
    }

    /// Abre el archivo recién creado (vacío), fija los pragmas y escribe el
    /// esquema y la cabecera en una sola transacción.
    fn initialize(path: &Path, header: &VaultHeader) -> Result<Self> {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sqlite_error)?;
        apply_pragmas(&conn)?;

        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)
            .map_err(map_sqlite_error)?;
        tx.execute_batch(SCHEMA).map_err(map_sqlite_error)?;
        insert_header(&tx, HeaderTable::Header, header)?;
        tx.commit().map_err(map_sqlite_error)?;

        Ok(Self { conn })
    }

    /// Abre una bóveda existente en lectura y escritura. Nunca crea el
    /// archivo: una ruta inexistente o relativa da `Storage(Io)`.
    ///
    /// Orden: `application_id` (primero en los bytes del archivo, después
    /// con SQLite) → cabecera → pragmas. Así un archivo ajeno se rechaza
    /// antes de que SQLite lo toque, y una versión futura antes de fijar
    /// `journal_mode`, que persiste en el archivo.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        require_absolute(path)?;
        check_file_header(path)?;
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sqlite_error)?;
        check_application_id(&conn)?;
        let storage = Self { conn };
        storage.read_header()?;
        apply_pragmas(&storage.conn)?;
        Ok(storage)
    }

    /// Abre una bóveda existente en solo lectura, para validar un respaldo.
    ///
    /// No fija pragmas (cambiar `journal_mode` es escribir): solo comprueba
    /// que el modo sea `delete`. Un archivo en WAL o con un `-journal`
    /// caliente se rechaza y nunca se repara. Una ruta relativa da
    /// `Storage(Io)`.
    pub(crate) fn open_read_only(path: &Path) -> Result<Self> {
        require_absolute(path)?;
        // Leer una base en WAL, aun en solo lectura, crea los archivos `-wal`
        // y `-shm`. Por eso el modo se revisa en la cabecera del archivo
        // antes de abrirlo con SQLite.
        if check_file_header(path)? {
            return Err(Error::Storage(StorageErrorKind::JournalMode));
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sqlite_error)?;
        // Primera lectura: con un `-journal` caliente falla aquí con
        // `SQLITE_READONLY_ROLLBACK`.
        check_application_id(&conn)?;
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .map_err(map_sqlite_error)?;
        if !mode.eq_ignore_ascii_case("delete") {
            return Err(Error::Storage(StorageErrorKind::JournalMode));
        }
        let storage = Self { conn };
        storage.read_header()?;
        Ok(storage)
    }

    /// Cierra la conexión sin perder un error al cerrar.
    pub(crate) fn close(self) -> Result<()> {
        self.conn.close().map_err(|(_, e)| map_sqlite_error(e))
    }

    /// Lee la cabecera y la valida con [`VaultHeader::from_parts`].
    ///
    /// Una columna fuera de rango, una longitud incorrecta o la falta de la
    /// fila dan `Storage(Corrupt)`: sin cabecera no hay bóveda.
    ///
    /// `format_version` se lee y se compara primero, sola: una versión
    /// futura puede tener otras columnas, y debe dar
    /// [`Error::UnsupportedFormatVersion`], no un error de lectura.
    pub(crate) fn read_header(&self) -> Result<VaultHeader> {
        let version: i64 = self
            .conn
            .query_row(
                "SELECT format_version FROM header WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::Storage(StorageErrorKind::Corrupt),
                other => map_sqlite_error(other),
            })?;
        let version =
            u16::try_from(version).map_err(|_| Error::Storage(StorageErrorKind::Corrupt))?;
        // La versión 1 no tiene la columna `key_check`: cada versión conocida
        // tiene su consulta. La tabla `header` es la misma en la 2 y la 3.
        let sql = match version {
            FORMAT_VERSION | FORMAT_VERSION_V2 => {
                "SELECT format_version, kdf_algorithm, kdf_memory_kib, kdf_iterations, \
                 kdf_parallelism, salt, key_nonce, encrypted_key, key_check \
                 FROM header WHERE id = 1"
            }
            FORMAT_VERSION_V1 => {
                "SELECT format_version, kdf_algorithm, kdf_memory_kib, kdf_iterations, \
                 kdf_parallelism, salt, key_nonce, encrypted_key, NULL \
                 FROM header WHERE id = 1"
            }
            other => return Err(Error::UnsupportedFormatVersion(other)),
        };

        let raw = self
            .conn
            .query_row(sql, [], |row| {
                Ok(RawHeader {
                    format_version: row.get(0)?,
                    algorithm: row.get(1)?,
                    params: KdfParams {
                        memory_kib: row.get(2)?,
                        iterations: row.get(3)?,
                        parallelism: row.get(4)?,
                    },
                    salt: row.get(5)?,
                    nonce: row.get(6)?,
                    ciphertext: row.get(7)?,
                    key_check: row.get(8)?,
                })
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::Storage(StorageErrorKind::Corrupt),
                other => map_sqlite_error(other),
            })?;

        let corrupt = |_| Error::Storage(StorageErrorKind::Corrupt);
        let salt = <[u8; SALT_LEN]>::try_from(raw.salt).map_err(corrupt)?;
        let nonce = <[u8; NONCE_LEN]>::try_from(raw.nonce).map_err(corrupt)?;
        let ciphertext = <[u8; ENCRYPTED_KEY_LEN]>::try_from(raw.ciphertext).map_err(corrupt)?;
        let key_check = raw
            .key_check
            .map(<[u8; KEY_CHECK_LEN]>::try_from)
            .transpose()
            .map_err(corrupt)?;
        VaultHeader::from_parts(
            raw.format_version,
            raw.algorithm,
            raw.params,
            salt,
            EncryptedKey { nonce, ciphertext },
            key_check,
        )
    }

    /// Reemplaza la única fila de la cabecera (al cambiar la contraseña
    /// maestra). La cabecera debe tener `key_check` (versión 2 o 3).
    pub(crate) fn replace_header(&mut self, header: &VaultHeader) -> Result<()> {
        let key_check = current_key_check(header)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        let params = header.params();
        let key = header.encrypted_key();
        let changed = tx
            .execute(
                "UPDATE header SET format_version = ?1, kdf_algorithm = ?2, \
                 kdf_memory_kib = ?3, kdf_iterations = ?4, kdf_parallelism = ?5, \
                 salt = ?6, key_nonce = ?7, encrypted_key = ?8, key_check = ?9 \
                 WHERE id = 1",
                params![
                    header.format_version(),
                    header.algorithm() as u8,
                    params.memory_kib,
                    params.iterations,
                    params.parallelism,
                    header.salt().as_slice(),
                    key.nonce.as_slice(),
                    key.ciphertext.as_slice(),
                    key_check.as_slice(),
                ],
            )
            .map_err(map_sqlite_error)?;
        if changed != 1 {
            return Err(Error::Storage(StorageErrorKind::Corrupt));
        }
        tx.commit().map_err(map_sqlite_error)
    }

    /// Reemplaza solo `key_check` (al repararlo en
    /// `Vault::export_vault_key`). Si no cambia exactamente una fila, da
    /// `Storage(Corrupt)`.
    pub(crate) fn replace_key_check(&mut self, key_check: &[u8; KEY_CHECK_LEN]) -> Result<()> {
        let changed = self.write(
            "UPDATE header SET key_check = ?1 WHERE id = 1",
            params![key_check.as_slice()],
        )?;
        if changed != 1 {
            return Err(Error::Storage(StorageErrorKind::Corrupt));
        }
        Ok(())
    }

    /// Migra la tabla `header` de la versión 1 a la 2 con `header`, ya
    /// armada por [`VaultHeader::unlock_upgrading`].
    ///
    /// Sigue el procedimiento que documenta SQLite para cambios de esquema
    /// ("Making Other Kinds Of Table Schema Changes", `lang_altertable.html`
    /// §8), en una sola transacción: crear `header_v2`, insertar la
    /// cabecera, borrar `header`, renombrar `header_v2` a `header` y
    /// verificar. Se renombra la tabla nueva, no la vieja: la documentación
    /// advierte que renombrar primero la vieja puede dañar referencias.
    /// `header` no tiene índices, triggers, vistas ni llaves foráneas que
    /// reconstruir. Si algo falla, la transacción se revierte y el archivo
    /// sigue siendo de la versión 1.
    ///
    /// Antes de tocar nada, ya con el candado de escritura, vuelve a leer
    /// `format_version`: entre la lectura de la cabecera v1 y esta
    /// transacción (Argon2id tarda) otra conexión pudo migrarla y cambiar la
    /// contraseña. Si ya no es la versión 1, no escribe nada y devuelve
    /// [`Migration::NotNeeded`]; quien llama decide cómo seguir. Sobrescribir
    /// revertiría ese cambio de contraseña sin aviso.
    pub(crate) fn migrate_header(&mut self, header: &VaultHeader) -> Result<Migration> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        let current = stored_format_version(&tx)?;
        if current != i64::from(FORMAT_VERSION_V1) {
            // Al soltar `tx` sin confirmar, SQLite revierte (no hubo cambios).
            return Ok(Migration::NotNeeded);
        }
        tx.execute_batch(concat!(
            "CREATE TABLE header_v2 ",
            header_v2_columns!(),
            ";"
        ))
        .map_err(map_sqlite_error)?;
        insert_header(&tx, HeaderTable::HeaderV2, header)?;
        tx.execute_batch("DROP TABLE header; ALTER TABLE header_v2 RENAME TO header;")
            .map_err(map_sqlite_error)?;

        let (version, key_check_len): (i64, i64) = tx
            .query_row(
                "SELECT format_version, length(key_check) FROM header WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(map_sqlite_error)?;
        if (version, key_check_len) != (i64::from(FORMAT_VERSION_V2), KEY_CHECK_LEN as i64) {
            return Err(Error::Storage(StorageErrorKind::Corrupt));
        }
        tx.commit().map_err(map_sqlite_error)?;
        Ok(Migration::Done)
    }

    /// Migra la bóveda de la versión 2 a la 3 (#93): crea `identity` y
    /// `paired_pcs` y deja `format_version = 3` con el `key_check` de la
    /// versión 3 (`key_check`, que arma quien llama con la llave de bóveda).
    ///
    /// Una sola transacción `IMMEDIATE`. Nunca toca `encrypted_key`, la sal ni
    /// los parámetros: la tabla `header` no cambia, así que no hay que
    /// volver a cifrar nada y la migración sirve también cuando solo se tiene
    /// la llave de bóveda. Tampoco puede revertir un cambio de contraseña de
    /// otra conexión.
    ///
    /// Antes de tocar nada, ya con el candado de escritura, vuelve a leer
    /// `format_version`: si ya es la 3 (otra conexión migró), no escribe y
    /// devuelve [`Migration::NotNeeded`]; si no es la 2 ni la 3, o ya existe
    /// alguna de las dos tablas (una v3 degradada a mano), da
    /// `Storage(Corrupt)` y se revierte sin escribir.
    pub(crate) fn migrate_v2_to_v3(
        &mut self,
        key_check: &[u8; KEY_CHECK_LEN],
    ) -> Result<Migration> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        let current = stored_format_version(&tx)?;
        if current == i64::from(FORMAT_VERSION) {
            // Al soltar `tx` sin confirmar, SQLite revierte (no hubo cambios).
            return Ok(Migration::NotNeeded);
        }
        if current != i64::from(FORMAT_VERSION_V2) {
            return Err(Error::Storage(StorageErrorKind::Corrupt));
        }
        upgrade_v2_to_v3(&tx, key_check)?;
        tx.commit().map_err(map_sqlite_error)?;
        Ok(Migration::Done)
    }

    /// Prepara la copia temporal de una importación (#93), en una sola
    /// transacción `IMMEDIATE`: la migra a la versión 3 si hace falta y le
    /// borra la identidad y las PCs emparejadas.
    ///
    /// `from_v2` es el `key_check` de la versión 3 si la copia validada era de
    /// la versión 1 o 2 (la 1 ya pasó a la 2): entonces espera la versión 2 y
    /// la migra. `None` si era de la versión 3: espera la 3. Si la versión que
    /// lee dentro de la transacción no es la esperada, la copia cambió después
    /// de validarla y da `Storage(Corrupt)` sin escribir nada: no se instala
    /// algo distinto de lo validado.
    ///
    /// La limpieza es lógica: la bóveda instalada no tiene filas en
    /// `identity` ni en `paired_pcs`. No promete que los bytes cifrados
    /// desaparezcan del archivo. Si algo falla, la transacción se revierte.
    pub(crate) fn prepare_import_copy(
        &mut self,
        from_v2: Option<&[u8; KEY_CHECK_LEN]>,
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        let current = stored_format_version(&tx)?;
        match from_v2 {
            Some(key_check) => {
                if current != i64::from(FORMAT_VERSION_V2) {
                    return Err(Error::Storage(StorageErrorKind::Corrupt));
                }
                upgrade_v2_to_v3(&tx, key_check)?;
            }
            None => {
                if current != i64::from(FORMAT_VERSION) {
                    return Err(Error::Storage(StorageErrorKind::Corrupt));
                }
            }
        }
        tx.execute_batch("DELETE FROM identity; DELETE FROM paired_pcs;")
            .map_err(map_sqlite_error)?;
        // Un respaldo manipulado puede traer un `TRIGGER ... RAISE(IGNORE)`
        // que hace que el `DELETE` no borre nada y tampoco falle: se comprueba
        // que de verdad quedaron vacías, y si no, no se instala nada.
        let remaining: i64 = tx
            .query_row(
                "SELECT (SELECT count(*) FROM identity) + (SELECT count(*) FROM paired_pcs)",
                [],
                |row| row.get(0),
            )
            .map_err(map_sqlite_error)?;
        if remaining != 0 {
            return Err(Error::Storage(StorageErrorKind::Corrupt));
        }
        tx.commit().map_err(map_sqlite_error)
    }

    /// La identidad guardada, o `None` si no hay fila (identidad ausente).
    ///
    /// Una versión de estructura fuera de `u16` o una columna con otro tipo
    /// dan [`Error::CorruptIdentity`]: una fila que existe pero no se puede
    /// leer no es una identidad ausente. Descifrarla corresponde a quien
    /// llama, que tiene la llave.
    pub(crate) fn get_identity(&self) -> Result<Option<StoredIdentity>> {
        let row = self.conn.query_row(
            "SELECT identity_schema_version, data FROM identity WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        );
        match row {
            Ok((version, data)) => Ok(Some(StoredIdentity {
                version: u16::try_from(version).map_err(|_| Error::CorruptIdentity)?,
                sealed: EncryptedPart::from_bytes(data),
            })),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) if is_conversion_error(&e) => Err(Error::CorruptIdentity),
            Err(e) => Err(map_sqlite_error(e)),
        }
    }

    /// Guarda la identidad si todavía no hay ninguna. Devuelve `false` y no
    /// escribe nada si ya existía una fila (válida o dañada): nunca la
    /// sobrescribe, ni siquiera si otra conexión la creó entre la lectura y
    /// esta escritura.
    pub(crate) fn insert_identity(&mut self, version: u16, data: &EncryptedPart) -> Result<bool> {
        let changed = self.write(
            "INSERT OR IGNORE INTO identity (id, identity_schema_version, data) \
             VALUES (1, ?1, ?2)",
            params![version, data.as_bytes()],
        )?;
        Ok(changed == 1)
    }

    /// Todas las filas de `paired_pcs`: las legibles (todavía cifradas) y las
    /// dañadas, por separado. Una fila con un id que no mide 16 bytes o con
    /// columnas de otro tipo no impide leer las demás.
    pub(crate) fn list_paired_pcs(&self) -> Result<PcListing> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, pc_schema_version, data FROM paired_pcs ORDER BY id")
            .map_err(map_sqlite_error)?;
        let rows = stmt.query_map([], read_pc_row).map_err(map_sqlite_error)?;

        let mut listing = PcListing {
            rows: Vec::new(),
            corrupt: Vec::new(),
            unidentified_corrupt: 0,
        };
        for outcome in rows {
            match outcome.map_err(map_sqlite_error)? {
                PcRowOutcome::Valid(pc) => listing.rows.push(pc),
                PcRowOutcome::Corrupt(id) => listing.corrupt.push(id),
                PcRowOutcome::Unidentified => listing.unidentified_corrupt += 1,
            }
        }
        Ok(listing)
    }

    /// La PC `id`, o `None` si no hay fila. Una fila que existe pero tiene
    /// columnas ilegibles da [`Error::CorruptPairedPc`], no `None`.
    pub(crate) fn get_paired_pc(&self, id: PairedPcId) -> Result<Option<StoredPc>> {
        let row = self.conn.query_row(
            "SELECT id, pc_schema_version, data FROM paired_pcs WHERE id = ?1",
            [id.as_bytes().as_slice()],
            read_pc_row,
        );
        match row {
            Ok(PcRowOutcome::Valid(pc)) => Ok(Some(pc)),
            Ok(PcRowOutcome::Corrupt(_) | PcRowOutcome::Unidentified) => {
                Err(Error::CorruptPairedPc)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_sqlite_error(e)),
        }
    }

    /// Agrega una PC. Un id ya usado es un error de almacenamiento y no
    /// escribe nada.
    pub(crate) fn insert_paired_pc(
        &mut self,
        id: PairedPcId,
        version: u16,
        data: &EncryptedPart,
    ) -> Result<()> {
        self.write(
            "INSERT INTO paired_pcs (id, pc_schema_version, data) VALUES (?1, ?2, ?3)",
            params![id.as_bytes().as_slice(), version, data.as_bytes()],
        )
        .map(|_| ())
    }

    /// Reemplaza los datos de una PC. Sin fila con ese id, da
    /// [`Error::PairedPcNotFound`].
    pub(crate) fn update_paired_pc(
        &mut self,
        id: PairedPcId,
        version: u16,
        data: &EncryptedPart,
    ) -> Result<()> {
        match self.write(
            "UPDATE paired_pcs SET pc_schema_version = ?1, data = ?2 WHERE id = ?3",
            params![version, data.as_bytes(), id.as_bytes().as_slice()],
        )? {
            0 => Err(Error::PairedPcNotFound),
            _ => Ok(()),
        }
    }

    /// Quita una PC (también una fila dañada). Un `DELETE` real: sin lápida.
    /// Sin fila con ese id, da [`Error::PairedPcNotFound`].
    pub(crate) fn delete_paired_pc(&mut self, id: PairedPcId) -> Result<()> {
        match self.write(
            "DELETE FROM paired_pcs WHERE id = ?1",
            params![id.as_bytes().as_slice()],
        )? {
            0 => Err(Error::PairedPcNotFound),
            _ => Ok(()),
        }
    }

    /// Copia consistente y compacta de la bóveda en `dest` con `VACUUM INTO`.
    /// `dest` debe no existir o estar vacío. La ruta va como parámetro,
    /// nunca dentro del SQL; una que no es UTF-8 o relativa da
    /// `Storage(Io)` (`VACUUM INTO` también acepta URIs).
    pub(crate) fn vacuum_into(&self, dest: &Path) -> Result<()> {
        require_absolute(dest)?;
        let dest = dest.to_str().ok_or(Error::Storage(StorageErrorKind::Io))?;
        self.conn
            .execute("VACUUM INTO ?1", [dest])
            .map_err(map_sqlite_error)?;
        Ok(())
    }

    /// `PRAGMA integrity_check`: páginas, registros, `CHECK`, `NOT NULL` y
    /// unicidad de la llave primaria. Con `(1)` se detiene en el primer
    /// problema; sin problemas devuelve una sola fila `ok`. El mensaje de
    /// error se descarta sin mostrarlo: cualquier otra respuesta da
    /// `Storage(Corrupt)`.
    pub(crate) fn check_integrity(&self) -> Result<()> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
            .map_err(map_sqlite_error)?;
        if result == "ok" {
            Ok(())
        } else {
            Err(Error::Storage(StorageErrorKind::Corrupt))
        }
    }

    /// Guarda una entrada nueva con `modified_at = now` y sin uso registrado.
    pub(crate) fn insert_entry(&mut self, entry: &SealedEntry, now: i64) -> Result<()> {
        self.write(
            "INSERT INTO entries (id, entry_schema_version, search, secret, modified_at, \
             last_used_at, deleted) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0)",
            params![
                entry.id().as_bytes().as_slice(),
                entry.entry_schema_version(),
                entry.search_bytes().as_bytes(),
                entry.secret_bytes().as_bytes(),
                now,
            ],
        )
        .map(|_| ())
    }

    /// Reemplaza la versión y las partes de una entrada viva y fija
    /// `modified_at = now`. No toca `last_used_at`.
    pub(crate) fn update_entry(&mut self, entry: &SealedEntry, now: i64) -> Result<()> {
        self.write_existing(
            "UPDATE entries SET entry_schema_version = ?2, search = ?3, secret = ?4, \
             modified_at = ?5 WHERE id = ?1 AND deleted = 0",
            params![
                entry.id().as_bytes().as_slice(),
                entry.entry_schema_version(),
                entry.search_bytes().as_bytes(),
                entry.secret_bytes().as_bytes(),
                now,
            ],
        )
    }

    /// Convierte una entrada viva en lápida: sin partes cifradas y con
    /// `modified_at = now`, en un solo `UPDATE`.
    pub(crate) fn delete_entry(&mut self, id: EntryId, now: i64) -> Result<()> {
        self.write_existing(
            "UPDATE entries SET deleted = 1, search = NULL, secret = NULL, modified_at = ?2 \
             WHERE id = ?1 AND deleted = 0",
            params![id.as_bytes().as_slice(), now],
        )
    }

    /// Registra el uso de una entrada viva. No cambia `modified_at`.
    pub(crate) fn touch_entry(&mut self, id: EntryId, now: i64) -> Result<()> {
        self.write_existing(
            "UPDATE entries SET last_used_at = ?2 WHERE id = ?1 AND deleted = 0",
            params![id.as_bytes().as_slice(), now],
        )
    }

    /// Lee una entrada viva. Una lápida o un id inexistente dan
    /// [`Error::EntryNotFound`]; una fila mal formada, [`Error::CorruptEntry`].
    pub(crate) fn get_entry(&self, id: EntryId) -> Result<StoredEntry> {
        let outcome = self
            .conn
            .query_row(
                &format!("{SELECT_ENTRY} WHERE id = ?1 AND deleted = 0"),
                [id.as_bytes().as_slice()],
                read_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::EntryNotFound,
                other => map_sqlite_error(other),
            })?;
        match outcome {
            RowOutcome::Valid(entry) => Ok(entry),
            RowOutcome::Corrupt(_) | RowOutcome::Unidentified => Err(Error::CorruptEntry),
        }
    }

    /// Lee solo `last_used_at` de una entrada viva, aunque las demás columnas
    /// de la fila no se puedan leer (para reparar sin perder el último uso).
    /// Una lápida o un id inexistente dan [`Error::EntryNotFound`].
    pub(crate) fn get_last_used_at(&self, id: EntryId) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT last_used_at FROM entries WHERE id = ?1 AND deleted = 0",
                [id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::EntryNotFound,
                other => map_sqlite_error(other),
            })
    }

    /// Lista las entradas vivas. Una fila mal formada no aborta el listado;
    /// un error de SQLite (por ejemplo, el archivo dañado) sí.
    pub(crate) fn list_entries(&self) -> Result<EntryListing> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SELECT_ENTRY} WHERE deleted = 0"))
            .map_err(map_sqlite_error)?;
        let rows = stmt.query_map([], read_row).map_err(map_sqlite_error)?;

        let mut listing = EntryListing {
            entries: Vec::new(),
            corrupt: Vec::new(),
            unidentified_corrupt: 0,
        };
        for outcome in rows {
            match outcome.map_err(map_sqlite_error)? {
                RowOutcome::Valid(entry) => listing.entries.push(entry),
                RowOutcome::Corrupt(id) => listing.corrupt.push(id),
                RowOutcome::Unidentified => listing.unidentified_corrupt += 1,
            }
        }
        Ok(listing)
    }

    /// Ejecuta una escritura en su propia transacción `IMMEDIATE` y devuelve
    /// cuántas filas cambió.
    /// Solo para pruebas: hace que toda escritura posterior falle.
    #[cfg(test)]
    pub(crate) fn deny_writes_for_test(&self) {
        self.conn
            .pragma_update(None, "query_only", true)
            .expect("query_only");
    }

    fn write(&mut self, sql: &str, params: impl rusqlite::Params) -> Result<usize> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite_error)?;
        let changed = tx.execute(sql, params).map_err(map_sqlite_error)?;
        tx.commit().map_err(map_sqlite_error)?;
        Ok(changed)
    }

    /// Como [`Self::write`], pero sobre una entrada viva: si no cambió
    /// ninguna fila, devuelve [`Error::EntryNotFound`].
    fn write_existing(&mut self, sql: &str, params: impl rusqlite::Params) -> Result<()> {
        match self.write(sql, params)? {
            0 => Err(Error::EntryNotFound),
            _ => Ok(()),
        }
    }
}

/// Identidad leída del archivo, todavía cifrada.
pub(crate) struct StoredIdentity {
    pub(crate) version: u16,
    pub(crate) sealed: EncryptedPart,
}

/// PC emparejada leída del archivo, todavía cifrada.
pub(crate) struct StoredPc {
    pub(crate) id: PairedPcId,
    pub(crate) version: u16,
    pub(crate) sealed: EncryptedPart,
}

/// Resultado de listar las PCs: las filas legibles y las dañadas.
pub(crate) struct PcListing {
    pub(crate) rows: Vec<StoredPc>,
    /// Filas dañadas con un id válido de 16 bytes.
    pub(crate) corrupt: Vec<PairedPcId>,
    /// Filas dañadas sin un id válido.
    pub(crate) unidentified_corrupt: usize,
}

/// Lo que se obtuvo de una fila de `paired_pcs`.
enum PcRowOutcome {
    Valid(StoredPc),
    Corrupt(PairedPcId),
    Unidentified,
}

/// Lee una fila de `paired_pcs` (`id`, versión, `data`). Los errores de
/// conversión son daños de la fila, no del archivo: se convierten en
/// `Corrupt` o `Unidentified` y su valor se descarta. Los demás errores se
/// propagan.
fn read_pc_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PcRowOutcome> {
    let id = match row.get::<_, Vec<u8>>(0) {
        Ok(bytes) => match <[u8; PAIRED_PC_ID_LEN]>::try_from(bytes) {
            Ok(bytes) => PairedPcId::from_bytes(bytes),
            Err(_) => return Ok(PcRowOutcome::Unidentified),
        },
        Err(e) if is_conversion_error(&e) => return Ok(PcRowOutcome::Unidentified),
        Err(e) => return Err(e),
    };
    let fields = (|| Ok((row.get::<_, u16>(1)?, row.get::<_, Vec<u8>>(2)?)))();
    match fields {
        Ok((version, data)) => Ok(PcRowOutcome::Valid(StoredPc {
            id,
            version,
            sealed: EncryptedPart::from_bytes(data),
        })),
        Err(e) if is_conversion_error(&e) => Ok(PcRowOutcome::Corrupt(id)),
        Err(e) => Err(e),
    }
}

/// Entrada viva leída del archivo.
#[derive(Debug)]
pub(crate) struct StoredEntry {
    pub(crate) sealed: SealedEntry,
    /// Milisegundos Unix de la última creación o edición.
    pub(crate) modified_at: i64,
    /// Milisegundos Unix del último uso; `None` si nunca se usó.
    pub(crate) last_used_at: Option<i64>,
}

/// Resultado de listar: las filas válidas y las dañadas, por separado.
#[derive(Debug)]
pub(crate) struct EntryListing {
    pub(crate) entries: Vec<StoredEntry>,
    /// Filas dañadas con un id válido de 16 bytes.
    pub(crate) corrupt: Vec<EntryId>,
    /// Filas dañadas cuyo id no mide 16 bytes: no hay un `EntryId` que
    /// reportar.
    pub(crate) unidentified_corrupt: usize,
}

const SELECT_ENTRY: &str = "SELECT id, entry_schema_version, search, secret, modified_at, \
                            last_used_at FROM entries";

/// Lo que se obtuvo de una fila de `entries`.
enum RowOutcome {
    Valid(StoredEntry),
    Corrupt(EntryId),
    Unidentified,
}

/// Lee una fila de [`SELECT_ENTRY`]. Los errores de conversión son daños de
/// la fila, no del archivo: se convierten en `Corrupt` o `Unidentified` y su
/// valor se descarta. Los demás errores se propagan.
fn read_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RowOutcome> {
    let id = match row.get::<_, Vec<u8>>(0) {
        Ok(bytes) => match <[u8; ENTRY_ID_LEN]>::try_from(bytes) {
            Ok(bytes) => EntryId::from_bytes(bytes),
            Err(_) => return Ok(RowOutcome::Unidentified),
        },
        Err(e) if is_conversion_error(&e) => return Ok(RowOutcome::Unidentified),
        Err(e) => return Err(e),
    };

    let fields = (|| {
        Ok((
            row.get::<_, u16>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })();
    match fields {
        Ok((version, search, secret, modified_at, last_used_at)) => {
            Ok(RowOutcome::Valid(StoredEntry {
                sealed: SealedEntry::from_parts(
                    id,
                    version,
                    EncryptedPart::from_bytes(search),
                    EncryptedPart::from_bytes(secret),
                ),
                modified_at,
                last_used_at,
            }))
        }
        Err(e) if is_conversion_error(&e) => Ok(RowOutcome::Corrupt(id)),
        Err(e) => Err(e),
    }
}

fn is_conversion_error(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::IntegralValueOutOfRange(..)
            | rusqlite::Error::FromSqlConversionFailure(..)
            | rusqlite::Error::InvalidColumnType(..)
    )
}

/// Columnas de la cabecera tal como salen de SQLite.
struct RawHeader {
    format_version: u16,
    algorithm: u8,
    params: KdfParams,
    salt: Vec<u8>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    /// `None` en la versión 1, que no tiene la columna.
    key_check: Option<Vec<u8>>,
}

/// Tabla donde se inserta la cabecera: la definitiva o la temporal de la
/// migración desde la versión 1. Los nombres son fijos, nunca datos.
#[derive(Clone, Copy)]
enum HeaderTable {
    Header,
    HeaderV2,
}

impl HeaderTable {
    fn insert_sql(self) -> &'static str {
        match self {
            Self::Header => {
                "INSERT INTO header (id, format_version, kdf_algorithm, kdf_memory_kib, \
                 kdf_iterations, kdf_parallelism, salt, key_nonce, encrypted_key, key_check) \
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            }
            Self::HeaderV2 => {
                "INSERT INTO header_v2 (id, format_version, kdf_algorithm, kdf_memory_kib, \
                 kdf_iterations, kdf_parallelism, salt, key_nonce, encrypted_key, key_check) \
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            }
        }
    }
}

/// `format_version` guardada en la cabecera. Sin fila no hay bóveda: da
/// `Storage(Corrupt)`, como `read_header`.
fn stored_format_version(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT format_version FROM header WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Error::Storage(StorageErrorKind::Corrupt),
        other => map_sqlite_error(other),
    })
}

/// Pasos de la migración de la versión 2 a la 3 dentro de la transacción de
/// quien llama: crea las dos tablas (que no deben existir) y sube
/// `format_version` con el nuevo `key_check`. La versión de origen ya la
/// comprobó quien llama.
fn upgrade_v2_to_v3(conn: &Connection, key_check: &[u8; KEY_CHECK_LEN]) -> Result<()> {
    const COUNT_PAIRING_TABLES: &str = "SELECT count(*) FROM sqlite_schema \
         WHERE type = 'table' AND name IN ('identity', 'paired_pcs')";
    let corrupt = || Error::Storage(StorageErrorKind::Corrupt);

    let existing: i64 = conn
        .query_row(COUNT_PAIRING_TABLES, [], |row| row.get(0))
        .map_err(map_sqlite_error)?;
    if existing != 0 {
        return Err(corrupt());
    }
    conn.execute_batch(pairing_tables!())
        .map_err(map_sqlite_error)?;
    let changed = conn
        .execute(
            "UPDATE header SET format_version = ?1, key_check = ?2 \
             WHERE id = 1 AND format_version = ?3",
            params![FORMAT_VERSION, key_check.as_slice(), FORMAT_VERSION_V2],
        )
        .map_err(map_sqlite_error)?;
    if changed != 1 {
        return Err(corrupt());
    }

    let (version, key_check_len): (i64, i64) = conn
        .query_row(
            "SELECT format_version, length(key_check) FROM header WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_error)?;
    let tables: i64 = conn
        .query_row(COUNT_PAIRING_TABLES, [], |row| row.get(0))
        .map_err(map_sqlite_error)?;
    if (version, key_check_len, tables) != (i64::from(FORMAT_VERSION), KEY_CHECK_LEN as i64, 2) {
        return Err(corrupt());
    }
    Ok(())
}

/// `key_check` de una cabecera de la versión 2 o 3. Una cabecera sin él
/// (de la versión 1) no se puede escribir: da `Storage(Corrupt)`.
fn current_key_check(header: &VaultHeader) -> Result<&[u8; KEY_CHECK_LEN]> {
    match header.key_check() {
        Some(key_check)
            if matches!(header.format_version(), FORMAT_VERSION_V2 | FORMAT_VERSION) =>
        {
            Ok(key_check)
        }
        _ => Err(Error::Storage(StorageErrorKind::Corrupt)),
    }
}

/// Inserta la única fila de la cabecera en `table`, dentro de la
/// transacción de quien llama.
fn insert_header(conn: &Connection, table: HeaderTable, header: &VaultHeader) -> Result<()> {
    let key_check = current_key_check(header)?;
    let params = header.params();
    let key = header.encrypted_key();
    conn.execute(
        table.insert_sql(),
        params![
            header.format_version(),
            header.algorithm() as u8,
            params.memory_kib,
            params.iterations,
            params.parallelism,
            header.salt().as_slice(),
            key.nonce.as_slice(),
            key.ciphertext.as_slice(),
            key_check.as_slice(),
        ],
    )
    .map_err(map_sqlite_error)?;
    Ok(())
}

/// Fija los pragmas de cada apertura de lectura y escritura. Ninguno de los
/// tres persiste en el archivo salvo `journal_mode`.
fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "secure_delete", "ON")
        .map_err(map_sqlite_error)?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(map_sqlite_error)?;
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0))
        .map_err(map_sqlite_error)?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err(Error::Storage(StorageErrorKind::JournalMode));
    }
    Ok(())
}

/// Revisa la cabecera del archivo (los primeros 100 bytes) sin pasar por
/// SQLite, para que SQLite nunca abra un archivo ajeno: abrirlo puede
/// revertir su `-journal` o integrar su `-wal`, es decir, modificarlo.
///
/// Devuelve si el archivo está en modo WAL (bytes 18 y 19 iguales a 2).
/// Un archivo inexistente o ilegible da `Storage(Io)`; uno sin el magic de
/// SQLite, más corto que la cabecera o con otro `application_id` (bytes
/// 68..72, big-endian) da [`Error::NotAVault`].
fn check_file_header(path: &Path) -> Result<bool> {
    const MAGIC: &[u8; 16] = b"SQLite format 3\0";
    let mut head = [0_u8; 100];
    let mut file = fs::File::open(path).map_err(|_| Error::Storage(StorageErrorKind::Io))?;
    io::Read::read_exact(&mut file, &mut head).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => Error::NotAVault,
        _ => Error::Storage(StorageErrorKind::Io),
    })?;
    let application_id = i32::from_be_bytes([head[68], head[69], head[70], head[71]]);
    if !head.starts_with(MAGIC) || application_id != APPLICATION_ID {
        return Err(Error::NotAVault);
    }
    Ok(head[18] == 2 || head[19] == 2)
}

/// Primera consulta tras abrir: como `rusqlite` abre de forma perezosa, un
/// archivo que no es SQLite falla aquí con `SQLITE_NOTADB`.
fn check_application_id(conn: &Connection) -> Result<()> {
    let id: i32 = conn
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(map_sqlite_error)?;
    if id != APPLICATION_ID {
        return Err(Error::NotAVault);
    }
    Ok(())
}

/// Rechaza una ruta relativa con `Storage(Io)` (ver el comentario del
/// módulo sobre `SQLITE_USE_URI`).
pub(crate) fn require_absolute(path: &Path) -> Result<()> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(Error::Storage(StorageErrorKind::Io))
    }
}

/// Traduce un error de SQLite a un [`Error`] del núcleo y lo descarta.
fn map_sqlite_error(err: rusqlite::Error) -> Error {
    match err {
        rusqlite::Error::SqliteFailure(e, _) => match e.code {
            ErrorCode::NotADatabase => Error::NotAVault,
            ErrorCode::DatabaseCorrupt => Error::Storage(StorageErrorKind::Corrupt),
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => {
                Error::Storage(StorageErrorKind::Busy)
            }
            ErrorCode::ReadOnly if e.extended_code == rusqlite::ffi::SQLITE_READONLY_ROLLBACK => {
                Error::Storage(StorageErrorKind::HotJournal)
            }
            ErrorCode::ReadOnly => Error::Storage(StorageErrorKind::ReadOnly),
            ErrorCode::CannotOpen
            | ErrorCode::SystemIoFailure
            | ErrorCode::DiskFull
            | ErrorCode::PermissionDenied => Error::Storage(StorageErrorKind::Io),
            _ => Error::Storage(StorageErrorKind::Other),
        },
        rusqlite::Error::IntegralValueOutOfRange(..)
        | rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::InvalidColumnType(..) => Error::Storage(StorageErrorKind::Corrupt),
        _ => Error::Storage(StorageErrorKind::Other),
    }
}

#[cfg(test)]
mod tests;
