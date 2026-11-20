//! Comprobación manual de la identidad y la lista de la PC (#99).
//!
//! No es parte de la app. Sirve para comprobar que los archivos protegidos con
//! DPAPI **no** se pueden abrir desde otra cuenta de Windows: se copia
//! `identity.bin` y `paired-devices.bin` a una carpeta de esa cuenta y se corre
//! este ejemplo apuntando a ella. Solo lee: no crea, no repara ni sobrescribe
//! nada, y nunca imprime llaves ni nombres.
//!
//! Uso: `cargo run -p boveda-desktop --example identity_probe -- <carpeta>`.
//! Imprime `identidad=…` y `lista=…`: `ok`, `ausente` o el tipo de error
//! (`Protection` si DPAPI no pudo descifrar, que es lo esperado en otra cuenta).

#[cfg(windows)]
fn main() {
    use boveda_desktop_lib::{identity::IdentityStore, paired_devices::PairedDeviceStore};

    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("uso: identity_probe <carpeta con identity.bin y paired-devices.bin>");
        std::process::exit(2);
    };
    let dir = std::path::PathBuf::from(dir);

    let identity = dir.join("identity.bin");
    if identity.exists() {
        // El archivo existe, así que `load_or_create` solo lee: nunca genera.
        match IdentityStore::new(identity).load_or_create() {
            Ok(_) => println!("identidad=ok"),
            Err(err) => println!("identidad={err:?}"),
        }
    } else {
        println!("identidad=ausente");
    }

    let list = dir.join("paired-devices.bin");
    if list.exists() {
        match PairedDeviceStore::open(list) {
            Ok(store) => println!("lista=ok ({} celulares)", store.list().len()),
            Err(err) => println!("lista={err:?}"),
        }
    } else {
        println!("lista=ausente");
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("identity_probe solo funciona en Windows");
}
