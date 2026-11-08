use super::*;
use crate::api::session::VaultSession;
use crate::test_support::{fixture_copy, path_string, FIXTURE_PASSWORD};

fn unlocked_fixture() -> (tempfile::TempDir, VaultSession) {
    let (dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    (dir, session)
}

fn input(service: &str) -> EntryInput {
    EntryInput {
        service: service.into(),
        url: None,
        username: None,
        tags: vec![],
    }
}

fn secret(password: &str) -> NewSecret {
    NewSecret {
        password: password.into(),
        notes: None,
    }
}

fn id_of(session: &VaultSession, service: &str) -> String {
    session
        .search(String::new())
        .unwrap()
        .into_iter()
        .find(|e| e.service == service)
        .unwrap()
        .id
}

fn is_hex_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[test]
fn search_empty_lists_live_entries_recent_first() {
    let (_dir, session) = unlocked_fixture();
    let all = session.search(String::new()).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].service, "Nube");
    assert!(all[0].last_used_at.is_some());
    let mail = all
        .iter()
        .find(|e| e.service == "Correo Electrónico")
        .unwrap();
    assert_eq!(
        mail.url.as_deref(),
        Some("https://correo.example.com/entrar")
    );
    assert_eq!(mail.username.as_deref(), Some("ana@example.com"));
    assert_eq!(mail.tags, ["Trabajo", "Médico", "Año"]);
    assert!(all.iter().all(|e| is_hex_id(&e.id)));
}

#[test]
fn search_finds_by_text() {
    let (_dir, session) = unlocked_fixture();
    assert_eq!(session.search("banco".into()).unwrap()[0].service, "Banco");
}

#[test]
fn add_returns_hex_id_found_by_search() {
    let (_dir, session) = unlocked_fixture();
    let id = session.add(input("Gmail"), secret("clave")).unwrap();
    assert!(is_hex_id(&id));
    let found = session.search("gmail".into()).unwrap();
    assert_eq!(found[0].id, id);
}

#[test]
fn add_returns_normalized_data() {
    let (_dir, session) = unlocked_fixture();
    let mut data = input("  Gmail  ");
    data.tags = vec!["a".into(), "A".into()];
    session.add(data, secret("clave")).unwrap();
    let found = &session.search("gmail".into()).unwrap()[0];
    assert_eq!(found.service, "Gmail");
    assert_eq!(found.tags, ["a"]);
}

#[test]
fn add_with_empty_service_fails() {
    let (_dir, session) = unlocked_fixture();
    assert_eq!(
        session.add(input("   "), secret("clave")),
        Err(VaultError::InvalidEntryServiceEmpty)
    );
}

#[test]
fn edit_keep_preserves_password() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Banco");
    session.edit(id.clone(), input("Banco 2"), None).unwrap();
    assert_eq!(session.reveal_password(id), Ok("clave-minima".into()));
}

#[test]
fn edit_replace_changes_password() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Correo Electrónico");
    session
        .edit(id.clone(), input("Correo"), Some(secret("nueva")))
        .unwrap();
    assert_eq!(session.reveal_password(id.clone()), Ok("nueva".into()));
    assert_eq!(session.reveal_notes(id), Ok(None));
}

#[test]
fn delete_removes_entry() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Banco");
    session.delete(id.clone()).unwrap();
    assert!(session
        .search("banco".into())
        .unwrap()
        .iter()
        .all(|e| e.service != "Banco"));
    assert_eq!(session.reveal_password(id), Err(VaultError::EntryNotFound));
}

#[test]
fn reveal_returns_fixture_values() {
    let (_dir, session) = unlocked_fixture();
    let mail = id_of(&session, "Correo Electrónico");
    assert_eq!(
        session.reveal_password(mail.clone()),
        Ok("clave-completa-ñ".into())
    );
    assert_eq!(
        session.reveal_notes(mail),
        Ok(Some("Línea uno\nLínea dos".into()))
    );
    let bank = id_of(&session, "Banco");
    assert_eq!(session.reveal_notes(bank), Ok(None));
}

#[test]
fn invalid_ids_are_rejected() {
    let (_dir, session) = unlocked_fixture();
    let valid = id_of(&session, "Banco");
    let invalid = [
        String::new(),
        valid[..31].to_owned(),
        format!("{valid}0"),
        valid.to_uppercase(),
        format!("{}g", &valid[..31]),
        "é".repeat(16),
    ];
    for id in invalid {
        assert_eq!(
            session.reveal_password(id.clone()),
            Err(VaultError::InvalidEntryId),
            "{id:?}"
        );
        assert_eq!(session.delete(id.clone()), Err(VaultError::InvalidEntryId));
        assert_eq!(
            session.edit(id, input("X"), None),
            Err(VaultError::InvalidEntryId)
        );
    }
}

#[test]
fn locked_session_gives_locked() {
    let (_dir, session) = unlocked_fixture();
    session.lock().unwrap();
    assert_eq!(session.search(String::new()), Err(VaultError::Locked));
}
