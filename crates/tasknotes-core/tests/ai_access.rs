//! Q47: an AI-facing interface works only after the person has explicitly
//! authorized it. Temporary data directories only.

use tasknotes_core::ai_access::{
    ACCESS_FILE, DATABASE_FILE, is_authorized, open_authorized, set_authorized,
};
use tasknotes_core::audit::Interface;
use tasknotes_core::tasks::TaskError;

#[test]
fn an_unauthorized_interface_is_refused_before_the_database_exists() {
    let dir = tempfile::tempdir().unwrap();
    for interface in [Interface::Cli, Interface::Mcp] {
        let refused = open_authorized(dir.path(), interface);
        assert!(matches!(refused, Err(TaskError::NotAuthorized)));
        assert_eq!(refused.unwrap_err().code(), "not_trusted");
    }
    assert!(!dir.path().join(DATABASE_FILE).exists());
    assert!(!dir.path().join(ACCESS_FILE).exists());
}

#[test]
fn each_interface_is_authorized_and_revoked_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    set_authorized(&data, Interface::Cli, true).unwrap();
    assert!(is_authorized(&data, Interface::Cli));
    assert!(!is_authorized(&data, Interface::Mcp));
    let store = open_authorized(&data, Interface::Cli).unwrap();
    assert_eq!(store.interface(), Interface::Cli);
    assert!(data.join(DATABASE_FILE).exists());

    set_authorized(&data, Interface::Mcp, true).unwrap();
    set_authorized(&data, Interface::Cli, false).unwrap();
    assert!(!is_authorized(&data, Interface::Cli));
    assert!(is_authorized(&data, Interface::Mcp));
    assert!(matches!(
        open_authorized(&data, Interface::Cli),
        Err(TaskError::NotAuthorized)
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(data.join(ACCESS_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "only the person's account can read it");
    }
}

#[test]
fn a_damaged_or_unknown_configuration_authorizes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    for text in [
        "{\"cli\": \"yes\"",
        r#"{"cli": "yes", "schema_version": 1}"#,
        r#"{"cli": true}"#,
        r#"{"cli": true, "schema_version": 2}"#,
        "[]",
    ] {
        std::fs::write(dir.path().join(ACCESS_FILE), text).unwrap();
        assert!(!is_authorized(dir.path(), Interface::Cli), "{text}");
    }
    std::fs::write(
        dir.path().join(ACCESS_FILE),
        r#"{"schema_version": 1, "cli": true}"#,
    )
    .unwrap();
    assert!(is_authorized(dir.path(), Interface::Cli));
    assert!(!is_authorized(dir.path(), Interface::Mcp));
}

#[test]
fn the_desktop_window_is_the_person_and_needs_no_authorization() {
    let dir = tempfile::tempdir().unwrap();
    assert!(is_authorized(dir.path(), Interface::Desktop));
}
