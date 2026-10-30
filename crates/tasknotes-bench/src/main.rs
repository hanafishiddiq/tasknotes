//! `tasknotes-bench`: seed a synthetic dataset in a directory of your choice,
//! then time queries against it. It refuses a directory that already holds
//! anything but its own dataset, so it can never touch real data.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use serde_json::{Value, json};
use tasknotes_bench::dataset::{self, FULL_NOTES, FULL_TASKS};
use tasknotes_bench::measure::{self, scenarios};
use tasknotes_core::tasks::TaskStore;

const USAGE: &str = "Usage: tasknotes-bench seed DIR [--tasks N] [--notes N]
       tasknotes-bench measure DIR [--runs N] [--cold-runs N] [--out FILE]";
const MANIFEST: &str = "synthetic-dataset.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["seed", dir, rest @ ..] => seed(Path::new(dir), rest),
        ["measure", dir, rest @ ..] => measure_all(Path::new(dir), rest),
        ["once", dir, name] => once(Path::new(dir), name),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn number(rest: &[&str], flag: &str, default: usize) -> Result<usize, String> {
    match rest.iter().position(|arg| *arg == flag) {
        None => Ok(default),
        Some(at) => rest
            .get(at + 1)
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| format!("{flag} needs a whole number")),
    }
}

fn seed(dir: &Path, rest: &[&str]) -> Result<(), String> {
    if dir.exists() && dir.read_dir().map_err(text)?.next().is_some() {
        return Err(format!(
            "{} is not empty; seed needs a new directory",
            dir.display()
        ));
    }
    let tasks = number(rest, "--tasks", FULL_TASKS)?;
    let notes = number(rest, "--notes", FULL_NOTES)?;
    let started = Instant::now();
    let mut manifest = dataset::seed(dir, tasks, notes).map_err(text)?;
    manifest["seed_seconds"] = json!(started.elapsed().as_secs());
    std::fs::write(dir.join(MANIFEST), manifest.to_string()).map_err(text)?;
    println!("{manifest}");
    Ok(())
}

fn manifest(dir: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(dir.join(MANIFEST))
        .map_err(|_| format!("{} holds no synthetic dataset", dir.display()))?;
    serde_json::from_str(&raw).map_err(text)
}

fn open(dir: &Path) -> Result<TaskStore, String> {
    TaskStore::open(&dir.join("tasknotes.db")).map_err(text)
}

/// A fresh process's first query, store opening included.
fn once(dir: &Path, name: &str) -> Result<(), String> {
    let manifest = manifest(dir)?;
    let all = scenarios(&manifest);
    let scenario = all
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("no scenario {name}"))?;
    let started = Instant::now();
    let store = open(dir)?;
    measure::run(&store, &scenario.query);
    println!("{}", started.elapsed().as_secs_f64() * 1000.0);
    Ok(())
}

fn measure_all(dir: &Path, rest: &[&str]) -> Result<(), String> {
    let manifest = manifest(dir)?;
    let runs = number(rest, "--runs", 200)?;
    let cold_runs = number(rest, "--cold-runs", 20)?;
    let store = open(dir)?;
    let me = std::env::current_exe().map_err(text)?;
    let mut rows = Vec::new();
    for scenario in scenarios(&manifest) {
        let (samples, found) = measure::warm(&store, &scenario, runs);
        let mut cold = Vec::new();
        for _ in 0..cold_runs {
            let output = Command::new(&me)
                .args(["once", &dir.to_string_lossy(), scenario.name])
                .output()
                .map_err(text)?;
            let line = String::from_utf8_lossy(&output.stdout);
            cold.push(line.trim().parse::<f64>().map_err(text)?);
        }
        let row = json!({
            "scenario": scenario.name,
            "search": scenario.search,
            "records_on_page": found,
            "warm": measure::summarize(samples),
            "fresh_process": measure::summarize(cold),
        });
        eprintln!("{row}");
        rows.push(row);
    }
    let report = json!({ "dataset": manifest, "scenarios": rows });
    let out = rest
        .iter()
        .position(|arg| *arg == "--out")
        .and_then(|at| rest.get(at + 1))
        .map(PathBuf::from);
    match out {
        Some(path) => std::fs::write(path, serde_json::to_string_pretty(&report).map_err(text)?)
            .map_err(text)?,
        None => println!("{report}"),
    }
    Ok(())
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
