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

    // `load` es de solo lectura: nunca crea ni modifica nada.
    match IdentityStore::new(dir.join("identity.bin")).load() {
        Ok(Some(_)) => println!("identidad=ok"),
        Ok(None) => println!("identidad=ausente"),
        Err(err) => println!("identidad={err:?}"),
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
