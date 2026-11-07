//! Almacenamiento SQLite de la bóveda: una tabla de cabecera con una sola
//! fila y una tabla de entradas.
//!
//! Este módulo solo mueve bytes y fechas: no cifra, no descifra y no conoce
//! la llave. Tampoco lee el reloj: cada escritura recibe la hora (`now`, en
//! milisegundos Unix) de quien llama.
//!
//! Los errores de SQLite se traducen en [`map_sqlite_error`] y se descartan:
//! pueden traer SQL, rutas o valores de columnas.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior, params};

use super::entry::sealed::SealedEntry;
use super::entry::{ENTRY_ID_LEN, EntryId};
use super::header::{FORMAT_VERSION, VaultHeader};
use crate::crypto::aead::EncryptedPart;
use crate::crypto::kdf::{KdfParams, SALT_LEN};
use crate::crypto::keys::{ENCRYPTED_KEY_LEN, EncryptedKey, NONCE_LEN};
use crate::error::{Error, Result, StorageErrorKind};

/// Identifica el archivo como bóveda ("BVLT"). Fijo para siempre, aunque
/// cambie el formato: la versión es solo `format_version` de la cabecera.
///
/// No es un control de seguridad, porque no está autenticado.
pub(crate) const APPLICATION_ID: i32 = 0x4256_4C54;

/// Esquema de la versión de formato 1.
const SCHEMA: &str = "
CREATE TABLE header (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    format_version  INTEGER NOT NULL,
    kdf_algorithm   INTEGER NOT NULL,
    kdf_memory_kib  INTEGER NOT NULL,
    kdf_iterations  INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    salt            BLOB NOT NULL CHECK (length(salt) = 16),
    key_nonce       BLOB NOT NULL CHECK (length(key_nonce) = 24),
    encrypted_key   BLOB NOT NULL CHECK (length(encrypted_key) = 48)
) STRICT;

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
";

/// Conexión a la bóveda. No deriva `Debug`.
pub(crate) struct Storage {
    conn: Connection,
}

impl Storage {
    /// Crea un archivo de bóveda nuevo con su esquema y su cabecera.
    ///
    /// Falla con `Error::AlreadyExists` si la ruta ya existe, sin tocarla.
    /// Si algo falla después de crear el archivo, lo borra.
    pub(crate) fn create(path: &Path, header: &VaultHeader) -> Result<Self> {
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
        let params = header.params();
        let key = header.encrypted_key();
        tx.execute(
            "INSERT INTO header (id, format_version, kdf_algorithm, kdf_memory_kib, \
             kdf_iterations, kdf_parallelism, salt, key_nonce, encrypted_key) \
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                header.format_version(),
                header.algorithm() as u8,
                params.memory_kib,
                params.iterations,
                params.parallelism,
                header.salt().as_slice(),
                key.nonce.as_slice(),
                key.ciphertext.as_slice(),
            ],
        )
        .map_err(map_sqlite_error)?;
        tx.commit().map_err(map_sqlite_error)?;

        Ok(Self { conn })
    }

    /// Abre una bóveda existente en lectura y escritura. Nunca crea el
    /// archivo: una ruta inexistente da `Storage(Io)`.
    ///
    /// Orden: `application_id` (primero en los bytes del archivo, después
    /// con SQLite) → pragmas → cabecera. Así un archivo ajeno se rechaza
    /// antes de que SQLite lo toque.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        check_file_header(path)?;
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(map_sqlite_error)?;
        check_application_id(&conn)?;
        apply_pragmas(&conn)?;
        let storage = Self { conn };
        storage.read_header()?;
        Ok(storage)
    }

    /// Abre una bóveda existente en solo lectura, para validar un respaldo.
    ///
    /// No fija pragmas (cambiar `journal_mode` es escribir): solo comprueba
    /// que el modo sea `delete`. Un archivo en WAL o con un `-journal`
    /// caliente se rechaza y nunca se repara.
    pub(crate) fn open_read_only(path: &Path) -> Result<Self> {
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
        if version != FORMAT_VERSION {
            return Err(Error::UnsupportedFormatVersion(version));
        }

        let raw = self
            .conn
            .query_row(
                "SELECT format_version, kdf_algorithm, kdf_memory_kib, kdf_iterations, \
                 kdf_parallelism, salt, key_nonce, encrypted_key FROM header WHERE id = 1",
                [],
                |row| {
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
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::Storage(StorageErrorKind::Corrupt),
                other => map_sqlite_error(other),
            })?;

        let corrupt = |_| Error::Storage(StorageErrorKind::Corrupt);
        let salt = <[u8; SALT_LEN]>::try_from(raw.salt).map_err(corrupt)?;
        let nonce = <[u8; NONCE_LEN]>::try_from(raw.nonce).map_err(corrupt)?;
        let ciphertext = <[u8; ENCRYPTED_KEY_LEN]>::try_from(raw.ciphertext).map_err(corrupt)?;
        VaultHeader::from_parts(
            raw.format_version,
            raw.algorithm,
            raw.params,
            salt,
            EncryptedKey { nonce, ciphertext },
        )
    }

    /// Reemplaza la única fila de la cabecera (al cambiar la contraseña
    /// maestra).
    pub(crate) fn replace_header(&mut self, header: &VaultHeader) -> Result<()> {
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
                 salt = ?6, key_nonce = ?7, encrypted_key = ?8 WHERE id = 1",
                params![
                    header.format_version(),
                    header.algorithm() as u8,
                    params.memory_kib,
                    params.iterations,
                    params.parallelism,
                    header.salt().as_slice(),
                    key.nonce.as_slice(),
                    key.ciphertext.as_slice(),
                ],
            )
            .map_err(map_sqlite_error)?;
        if changed != 1 {
            return Err(Error::Storage(StorageErrorKind::Corrupt));
        }
        tx.commit().map_err(map_sqlite_error)
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
