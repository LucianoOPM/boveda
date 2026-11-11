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
    session
        .edit(id.clone(), input("Banco 2"), None, None)
        .unwrap();
    assert_eq!(session.reveal_password(id), Ok("clave-minima".into()));
}

/// Notas de "Correo Electrónico" en la bóveda de ejemplo.
const MAIL_NOTES: &str = "Línea uno\nLínea dos";

#[test]
fn edit_new_password_only_keeps_notes() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Correo Electrónico");
    session
        .edit(id.clone(), input("Correo"), Some("nueva".into()), None)
        .unwrap();
    assert_eq!(session.reveal_password(id.clone()), Ok("nueva".into()));
    assert_eq!(session.reveal_notes(id), Ok(Some(MAIL_NOTES.into())));
}

#[test]
fn edit_new_notes_only_keeps_password() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Banco");
    session
        .edit(id.clone(), input("Banco"), None, Some("PIN aparte".into()))
        .unwrap();
    assert_eq!(
        session.reveal_password(id.clone()),
        Ok("clave-minima".into())
    );
    assert_eq!(session.reveal_notes(id), Ok(Some("PIN aparte".into())));
}

#[test]
fn edit_empty_notes_removes_them() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Correo Electrónico");
    session
        .edit(id.clone(), input("Correo"), None, Some(String::new()))
        .unwrap();
    assert_eq!(
        session.reveal_password(id.clone()),
        Ok("clave-completa-ñ".into())
    );
    assert_eq!(session.reveal_notes(id), Ok(None));
}

#[test]
fn edit_both_replaces_secret() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Correo Electrónico");
    session
        .edit(
            id.clone(),
            input("Correo"),
            Some("nueva".into()),
            Some("otras".into()),
        )
        .unwrap();
    assert_eq!(session.reveal_password(id.clone()), Ok("nueva".into()));
    assert_eq!(session.reveal_notes(id), Ok(Some("otras".into())));
}

#[test]
fn entry_follows_add_edit_delete() {
    let (_dir, session) = unlocked_fixture();
    let mut new = input("  Foro  ");
    new.tags = vec!["a".into(), "A".into(), " b ".into()];
    let id = session.add(new, secret("clave")).unwrap();

    let added = session.entry(id.clone()).unwrap();
    assert_eq!(added.id, id);
    assert_eq!(added.service, "Foro");
    assert_eq!(added.tags, ["a", "b"]);
    assert_eq!(added.last_used_at, None);

    session
        .edit(id.clone(), input("Foro 2"), None, None)
        .unwrap();
    let edited = session.entry(id.clone()).unwrap();
    assert_eq!(edited.service, "Foro 2");
    assert!(edited.tags.is_empty());

    session.reveal_password(id.clone()).unwrap();
    assert!(session.entry(id.clone()).unwrap().last_used_at.is_some());

    session.delete(id.clone()).unwrap();
    assert_eq!(session.entry(id), Err(VaultError::EntryNotFound));
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
            session.entry(id.clone()).map(drop),
            Err(VaultError::InvalidEntryId)
        );
        assert_eq!(
            session.edit(id, input("X"), None, None),
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

// El secreto se convierte (y el `String` recibido se borra) antes de
// cualquier validación que pueda fallar: con datos y secreto inválidos, el
// error que sale es el del secreto.
#[test]
fn add_converts_secret_before_validating_search_data() {
    let (_dir, session) = unlocked_fixture();
    assert_eq!(
        session.add(input("   "), secret("")),
        Err(VaultError::InvalidEntryPasswordEmpty)
    );
}

#[test]
fn edit_converts_secret_before_reading_id() {
    let (_dir, session) = unlocked_fixture();
    assert_eq!(
        session.edit(
            "no-es-un-id".into(),
            input("   "),
            Some(String::new()),
            Some(String::new())
        ),
        Err(VaultError::InvalidEntryPasswordEmpty)
    );
}

// Con una sola parte, la validación del valor nuevo la hace el núcleo
// dentro de `edit`, después del id y de los datos de búsqueda.
#[test]
fn edit_single_part_reports_invalid_value() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Banco");
    assert_eq!(
        session.edit(id.clone(), input("Banco"), Some(String::new()), None),
        Err(VaultError::InvalidEntryPasswordEmpty)
    );
    assert_eq!(
        session.edit(id, input("Banco"), None, Some("n".repeat(16385))),
        Err(VaultError::InvalidEntryNotesTooLong)
    );
}

/// Cada operación que necesita la bóveda abierta y desbloqueada, con un id
/// válido del fixture.
fn every_operation(
    session: &VaultSession,
    id: &str,
) -> Vec<(&'static str, Result<(), VaultError>)> {
    let dest = std::env::temp_dir().join("boveda-no-se-crea.db");
    vec![
        ("search", session.search(String::new()).map(drop)),
        ("add", session.add(input("X"), secret("clave")).map(drop)),
        ("edit", session.edit(id.into(), input("X"), None, None)),
        ("entry", session.entry(id.into()).map(drop)),
        ("delete", session.delete(id.into())),
        (
            "reveal_password",
            session.reveal_password(id.into()).map(drop),
        ),
        ("reveal_notes", session.reveal_notes(id.into()).map(drop)),
        (
            "export_backup",
            session.export_backup(dest.to_str().unwrap().into()),
        ),
    ]
}

#[test]
fn every_operation_without_vault_gives_no_vault() {
    let (_dir, unlocked) = unlocked_fixture();
    let id = id_of(&unlocked, "Banco");
    let (_empty_dir, path) = crate::test_support::temp_path();
    let session = VaultSession::new(path_string(&path));
    for (name, result) in every_operation(&session, &id) {
        assert_eq!(result, Err(VaultError::NoVault), "{name}");
    }
}

#[test]
fn every_operation_after_lock_gives_locked() {
    let (_dir, session) = unlocked_fixture();
    let id = id_of(&session, "Banco");
    session.lock().unwrap();
    for (name, result) in every_operation(&session, &id) {
        assert_eq!(result, Err(VaultError::Locked), "{name}");
    }
}
