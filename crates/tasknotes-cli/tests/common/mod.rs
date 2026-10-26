//! Runs the built `tasknotes` binary as a separate process against a
//! temporary data directory. Synthetic data only.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

pub struct Cli {
    pub dir: tempfile::TempDir,
}

pub struct Run {
    pub code: i32,
    pub json: Value,
    pub stderr: String,
}

impl Cli {
    /// A fresh data directory the person has not granted access to yet.
    pub fn untrusted() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// A data directory where the person has granted CLI access.
    pub fn trusted() -> Self {
        let cli = Self::untrusted();
        std::fs::write(
            cli.dir.path().join("ai-access.json"),
            r#"{"schema_version":1,"cli":true}"#,
        )
        .unwrap();
        cli
    }

    pub fn data_dir(&self) -> &Path {
        self.dir.path()
    }

    pub fn db(&self) -> PathBuf {
        self.dir.path().join("tasknotes.db")
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tasknotes"));
        command
            .arg("--data-dir")
            .arg(self.dir.path())
            .args(args)
            // Never reach a real data directory, whatever the environment.
            .env_remove("TASKNOTES_DATA_DIR")
            .env("HOME", self.dir.path().join("home"))
            .stdin(Stdio::null());
        command
    }

    pub fn run(&self, args: &[&str]) -> Run {
        finish(self.command(args).output().unwrap())
    }

    /// Run and require success; returns the envelope.
    pub fn ok(&self, args: &[&str]) -> Value {
        let run = self.run(args);
        assert_eq!(run.code, 0, "{args:?} failed: {}", run.json);
        assert_eq!(run.json["ok"], true);
        run.json
    }
}

pub fn finish(output: std::process::Output) -> Run {
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {stdout:?}"));
    assert_eq!(
        json["schema_version"], 1,
        "envelope carries its schema version"
    );
    Run {
        code: output.status.code().unwrap(),
        json,
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// The error code of a failed run, after checking its exit code.
pub fn error_code(run: &Run, exit: i32) -> String {
    assert_eq!(run.code, exit, "unexpected exit for {}", run.json);
    assert_eq!(run.json["ok"], false);
    assert!(run.stderr.is_empty(), "nothing on stderr: {}", run.stderr);
    run.json["error"]["code"].as_str().unwrap().to_owned()
}
