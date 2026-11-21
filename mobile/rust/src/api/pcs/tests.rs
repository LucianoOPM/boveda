use vault_core::protocol::PublicKey;

use super::*;
use crate::test_support::{
    fixture_copy, fixture_v3_copy, path_string, FIXTURE_PASSWORD, V3_PASSWORD,
};

fn v3_unlocked() -> (tempfile::TempDir, VaultSession) {
    let (dir, path) = fixture_v3_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(V3_PASSWORD.into()).unwrap();
    (dir, session)
}

fn fingerprint_of(byte: u8) -> String {
    PublicKey::from_bytes([byte; 32]).fingerprint().to_string()
}

fn pc_named<'a>(list: &'a PairedPcList, name: &str) -> &'a PairedPcSummary {
    list.pcs.iter().find(|pc| pc.name == name).unwrap()
}

#[test]
fn paired_pcs_lists_the_two_fixture_pcs_without_keys() {
    let (_dir, session) = v3_unlocked();
    let list = session.paired_pcs().unwrap();
    assert_eq!(list.pcs.len(), 2);
    assert!(list.corrupt.is_empty());
    assert_eq!(list.unidentified_corrupt, 0);

    let desk = pc_named(&list, "Escritorio");
    assert_eq!(desk.fingerprint, fingerprint_of(0x11));
    assert_eq!(desk.last_ip.as_deref(), Some("192.168.1.50"));
    assert!(desk.last_seen_at.is_some());
    assert_eq!(desk.id.len(), 32);
    assert!(desk
        .id
        .bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));

    let laptop = pc_named(&list, "Portátil");
    assert_eq!(laptop.fingerprint, fingerprint_of(0x22));
    assert_eq!(laptop.last_ip, None);
    assert_eq!(laptop.last_seen_at, None);

    // Ningún texto del resumen es la llave en hexadecimal.
    let key_hex = "11".repeat(32);
    for pc in &list.pcs {
        for text in [&pc.id, &pc.name, &pc.fingerprint] {
            assert!(!text.contains(&key_hex));
        }
    }
}

#[test]
fn ensure_identity_returns_the_fixture_fingerprint() {
    let (_dir, session) = v3_unlocked();
    let expected = fingerprint_of_private(7);
    assert_eq!(session.ensure_identity().unwrap().fingerprint, expected);
    assert_eq!(session.ensure_identity().unwrap().fingerprint, expected);
}

fn fingerprint_of_private(byte: u8) -> String {
    use secrecy::SecretBox;
    let private = SecretBox::new(Box::new([byte; 32]));
    PublicKey::from_private(&private).fingerprint().to_string()
}

#[test]
fn ensure_identity_creates_it_lazily_on_a_vault_without_one() {
    let (_dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let first = session.ensure_identity().unwrap();
    assert_eq!(first.fingerprint.len(), 19);
    assert_eq!(session.ensure_identity().unwrap(), first);
}

#[test]
fn remove_paired_pc_removes_it_and_unknown_id_is_not_found() {
    let (_dir, session) = v3_unlocked();
    let list = session.paired_pcs().unwrap();
    let id = pc_named(&list, "Portátil").id.clone();
    session.remove_paired_pc(id.clone()).unwrap();
    let after = session.paired_pcs().unwrap();
    assert_eq!(after.pcs.len(), 1);
    assert_eq!(after.pcs[0].name, "Escritorio");
    assert_eq!(
        session.remove_paired_pc(id),
        Err(VaultError::PairedPcNotFound)
    );
}

#[test]
fn invalid_pc_id_is_rejected() {
    let (_dir, session) = v3_unlocked();
    for bad in ["", "abc", &"A".repeat(32), &"g".repeat(32), &"a".repeat(31)] {
        assert_eq!(
            session.remove_paired_pc(bad.to_owned()),
            Err(VaultError::InvalidPairedPcId),
            "id {bad:?}"
        );
    }
}

#[test]
fn locked_or_missing_vault() {
    let (_dir, session) = v3_unlocked();
    session.lock().unwrap();
    assert_eq!(session.paired_pcs().unwrap_err(), VaultError::Locked);
    assert_eq!(session.ensure_identity().unwrap_err(), VaultError::Locked);
    assert_eq!(
        session.remove_paired_pc("a".repeat(32)),
        Err(VaultError::Locked)
    );

    let empty = VaultSession::new(path_string(
        &tempfile::tempdir().unwrap().path().join("x.db"),
    ));
    assert_eq!(empty.paired_pcs().unwrap_err(), VaultError::NoVault);
    assert_eq!(empty.ensure_identity().unwrap_err(), VaultError::NoVault);
    assert_eq!(
        empty.remove_paired_pc("a".repeat(32)),
        Err(VaultError::NoVault)
    );
}

#[test]
fn hex_helpers_roundtrip_and_validate() {
    let bytes = [
        0x00, 0x0f, 0xa0, 0xff, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    ];
    let hex = bytes16_to_hex(bytes);
    assert_eq!(hex, "000fa0ff0102030405060708090a0b0c");
    assert_eq!(hex_to_bytes16(&hex), Ok(bytes));
    assert_eq!(
        hex_to_bytes16("é".repeat(16).as_str()),
        Err(VaultError::InvalidEntryId)
    );
}
