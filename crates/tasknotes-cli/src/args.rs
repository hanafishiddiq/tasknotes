//! A deliberately small argument parser: every mistake becomes an
//! `invalid_input` error in the JSON envelope instead of free-form text.

use crate::contract::CliError;

/// The flags one command accepts.
pub struct Spec<'a> {
    /// `--name value` (or `--name=value`), at most once.
    pub values: &'a [&'a str],
    /// `--name value`, any number of times.
    pub repeated: &'a [&'a str],
    /// `--name`, no value.
    pub switches: &'a [&'a str],
    /// How many positional words (ids) the command takes.
    pub positionals: usize,
}

#[derive(Debug, Default)]
pub struct Parsed {
    positionals: Vec<String>,
    values: Vec<(String, String)>,
    switches: Vec<String>,
}

pub fn parse(words: &[String], spec: &Spec) -> Result<Parsed, CliError> {
    let mut parsed = Parsed::default();
    let mut iter = words.iter();
    while let Some(word) = iter.next() {
        let Some(flag) = word.strip_prefix("--") else {
            parsed.positionals.push(word.clone());
            continue;
        };
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (flag, None),
        };
        if spec.switches.contains(&name) {
            if inline.is_some() {
                return Err(CliError::invalid(format!("--{name} takes no value")));
            }
            if parsed.switches.iter().any(|s| s == name) {
                return Err(CliError::invalid(format!("--{name} is given twice")));
            }
            parsed.switches.push(name.to_owned());
        } else if spec.values.contains(&name) || spec.repeated.contains(&name) {
            let value = match inline {
                Some(value) => value,
                None => iter
                    .next()
                    .cloned()
                    .ok_or_else(|| CliError::invalid(format!("--{name} needs a value")))?,
            };
            if spec.values.contains(&name) && parsed.values.iter().any(|(n, _)| n == name) {
                return Err(CliError::invalid(format!("--{name} is given twice")));
            }
            parsed.values.push((name.to_owned(), value));
        } else {
            return Err(CliError::invalid(format!("Unknown option --{name}")));
        }
    }
    if parsed.positionals.len() != spec.positionals {
        return Err(CliError::invalid(match spec.positionals {
            0 => "This command takes no id".to_owned(),
            n => format!("This command takes exactly {n} id"),
        }));
    }
    Ok(parsed)
}

impl Parsed {
    pub fn id(&self) -> &str {
        &self.positionals[0]
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn all(&self, name: &str) -> Vec<String> {
        self.values
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name) || self.values.iter().any(|(n, _)| n == name)
    }

    pub fn required(&self, name: &str) -> Result<&str, CliError> {
        self.value(name)
            .ok_or_else(|| CliError::invalid(format!("--{name} is required")))
    }

    /// `--revision`: the revision the caller last read, required for changes.
    pub fn revision(&self) -> Result<i64, CliError> {
        self.required("revision")?
            .parse::<i64>()
            .ok()
            .filter(|r| *r >= 1)
            .ok_or_else(|| CliError::invalid("--revision must be a whole number of at least 1"))
    }

    /// `--limit`, 50 when absent; the core checks the upper bound.
    pub fn limit(&self) -> Result<u32, CliError> {
        match self.value("limit") {
            None => Ok(50),
            Some(raw) => raw
                .parse::<u32>()
                .map_err(|_| CliError::invalid("--limit must be a whole number")),
        }
    }

    pub fn number<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, CliError> {
        self.value(name)
            .map(|raw| {
                raw.parse::<T>()
                    .map_err(|_| CliError::invalid(format!("--{name} must be a whole number")))
            })
            .transpose()
    }

    /// A snake_case enum value such as `in_progress` or `very_high`.
    pub fn choice<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
        allowed: &str,
    ) -> Result<Option<T>, CliError> {
        self.value(name)
            .map(|raw| {
                serde_json::from_value(serde_json::Value::String(raw.to_owned()))
                    .map_err(|_| CliError::invalid(format!("--{name} must be one of: {allowed}")))
            })
            .transpose()
    }
}
