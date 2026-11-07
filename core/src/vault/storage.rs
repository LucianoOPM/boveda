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

use super::header::VaultHeader;
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
    /// Falla con `Storage(AlreadyExists)` si la ruta ya existe, sin tocarla.
    /// Si algo falla después de crear el archivo, lo borra.
    pub(crate) fn create(path: &Path, header: &VaultHeader) -> Result<Self> {
        // `create_new` falla de forma atómica si la ruta ya existe.
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| match e.kind() {
                io::ErrorKind::AlreadyExists => Error::Storage(StorageErrorKind::AlreadyExists),
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
    /// Orden: `application_id` → pragmas → cabecera. Así un archivo ajeno se
    /// rechaza antes de escribir nada en él.
    pub(crate) fn open(path: &Path) -> Result<Self> {
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

    /// Cierra la conexión sin perder un error al cerrar.
    pub(crate) fn close(self) -> Result<()> {
        self.conn.close().map_err(|(_, e)| map_sqlite_error(e))
    }

    /// Lee la cabecera y la valida con [`VaultHeader::from_parts`].
    ///
    /// Una columna fuera de rango, una longitud incorrecta o la falta de la
    /// fila dan `Storage(Corrupt)`: sin cabecera no hay bóveda.
    pub(crate) fn read_header(&self) -> Result<VaultHeader> {
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
