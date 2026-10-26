//! Q47: the CLI works only after the person grants local access.

mod common;

use common::{Cli, error_code};

#[test]
fn an_unconfigured_cli_refuses_every_data_command_without_touching_the_database() {
    let cli = Cli::untrusted();
    for args in [
        vec!["task", "list"],
        vec!["task", "create", "--title", "Plan the garden"],
        vec!["list", "list"],
        vec!["audit", "list"],
    ] {
        let run = cli.run(&args);
        assert_eq!(error_code(&run, 6), "not_trusted", "{args:?}");
    }
    assert!(!cli.db().exists(), "no database is created or migrated");
}

#[test]
fn access_cannot_be_granted_without_an_interactive_person() {
    let cli = Cli::untrusted();
    let run = cli.run(&["access", "grant"]);
    assert_eq!(error_code(&run, 6), "not_trusted");
    assert!(!cli.data_dir().join("ai-access.json").exists());
    assert_eq!(cli.ok(&["access", "status"])["item"]["cli"], false);
}

#[test]
fn a_granted_cli_works_and_revoking_stops_it_again() {
    let cli = Cli::trusted();
    assert_eq!(cli.ok(&["access", "status"])["item"]["cli"], true);
    cli.ok(&["task", "list"]);
    cli.ok(&["access", "revoke"]);
    assert_eq!(error_code(&cli.run(&["task", "list"]), 6), "not_trusted");
}

#[test]
fn a_damaged_trust_file_is_not_trust() {
    let cli = Cli::untrusted();
    std::fs::write(cli.data_dir().join("ai-access.json"), "{\"cli\": \"yes\"").unwrap();
    assert_eq!(error_code(&cli.run(&["task", "list"]), 6), "not_trusted");
    assert!(!cli.db().exists());
}

#[test]
fn a_person_at_a_terminal_who_types_allow_grants_access_privately() {
    use tasknotes_cli::access::{Environment, cli_trusted, grant};
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let terminal = Environment {
        stdin_is_terminal: true,
        ..Environment::default()
    };
    assert!(grant(&data, &terminal, || Ok("no\n".into())).is_err());
    assert!(!cli_trusted(&data));
    grant(&data, &terminal, || Ok("allow\n".into())).unwrap();
    assert!(cli_trusted(&data));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(data.join("ai-access.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
