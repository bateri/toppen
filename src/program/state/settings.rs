//! The settings: `~/.config/toppen/settings.json` (`$XDG_CONFIG_HOME/toppen/` where that is set,
//! `$TOPPEN_CONFIG_DIR` over both), the person's own file and theirs alone.
//!
//! - A file that is not there is the defaults; one that cannot be read (broken JSON, a value that is
//!   not what it should be) is left exactly where it is, the defaults are used, and the error is told
//!   once, at the start. While it is broken nothing is written over it: the person fixes it, or
//!   removes it.
//! - What is not known to this version is kept when the file is written again.
//! - A write is atomic: the new text goes to a file beside it, which then takes the place of the old.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::command_line::{MAX_INTERVAL, MIN_INTERVAL};

const FILE: &str = "settings.json";

/// The modes the table has.
const MODES: [&str; 3] = ["flat", "tree", "group"];

/// The most keys one command is given, and the longest id and key words: a limit against a file
/// that is not meant for this program.
const MOST_KEYS: usize = 6;
const LONGEST: usize = 64;

/// A list of words. The page's empty list may come as an empty object (Lua has one kind of empty
/// table), which is the same list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Words(pub Vec<String>);

impl<'de> Deserialize<'de> for Words {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Array(items) => items
                .into_iter()
                .map(|item| match item {
                    Value::String(word) => Ok(word),
                    other => Err(D::Error::custom(format!("{other} is not a word"))),
                })
                .collect::<Result<_, _>>()
                .map(Self),
            Value::Object(map) if map.is_empty() => Ok(Self::default()),
            other => Err(D::Error::custom(format!("a list of words, not {other}"))),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Seconds between two looks at the machine; the command line's `--interval` is stronger.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<f64>,
    /// The column the table is ordered by, and whether the largest comes first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descending: Option<bool>,
    /// "flat", "tree" or "group".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// The columns that are not shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<Words>,
    /// Whether the panel of a process is open at the start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<bool>,
    /// The machine's and the person's names are not shown.
    pub privacy: bool,
    /// The keys of a command, by its id: they replace the ones it has by default. An empty list
    /// leaves the command without keys.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, Words>,
    /// What this version does not know, kept as it was.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Settings {
    /// Why these settings are not what a run can use, if they are not.
    fn check(&self) -> Result<(), String> {
        if let Some(seconds) = self.interval
            && !(seconds.is_finite() && (MIN_INTERVAL..=MAX_INTERVAL).contains(&seconds))
        {
            return Err(format!(
                "interval is {MIN_INTERVAL} to {MAX_INTERVAL} seconds, not {seconds}"
            ));
        }
        if let Some(mode) = &self.mode
            && !MODES.contains(&mode.as_str())
        {
            return Err(format!("mode is flat, tree or group, not {mode:?}"));
        }
        for (id, keys) in &self.keys {
            if id.is_empty() || id.len() > LONGEST {
                return Err(format!("keys: {id:?} is not the id of a command"));
            }
            if keys.0.len() > MOST_KEYS || keys.0.iter().any(|words| words.len() > LONGEST) {
                return Err(format!("keys: {id} has too many keys, or one too long"));
            }
        }
        Ok(())
    }
}

/// Where the settings live: the directory.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    let from = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    if let Some(dir) = from("TOPPEN_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Some(dir) = from("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("toppen"));
    }
    from("HOME").map(|home| PathBuf::from(home).join(".config").join("toppen"))
}

struct State {
    settings: Settings,
    /// Why the file was not used: it is left as it is while this is so.
    broken: Option<String>,
}

/// The settings of this run, and the file they are kept in (none: kept in memory, as the tests do).
pub struct Store {
    path: Option<PathBuf>,
    state: Mutex<State>,
}

impl Store {
    /// Settings that are never written anywhere.
    #[must_use]
    pub fn memory(settings: Settings) -> Self {
        Self {
            path: None,
            state: Mutex::new(State {
                settings,
                broken: None,
            }),
        }
    }

    /// The settings of the directory `dir`.
    #[must_use]
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(FILE);
        let (settings, broken) = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text)
                .map_err(|why| why.to_string())
                .and_then(|settings| settings.check().map(|()| settings))
            {
                Ok(settings) => (settings, None),
                Err(why) => (
                    Settings::default(),
                    Some(format!("{} is not used: {why}", path.display())),
                ),
            },
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => (Settings::default(), None),
            Err(why) => (
                Settings::default(),
                Some(format!("{} cannot be read: {why}", path.display())),
            ),
        };
        Self {
            path: Some(path),
            state: Mutex::new(State { settings, broken }),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn get(&self) -> Settings {
        self.state().settings.clone()
    }

    /// The file's path, where there is one.
    #[must_use]
    pub fn path(&self) -> Option<String> {
        self.path.as_ref().map(|path| path.display().to_string())
    }

    /// Why the file was not used, if it was not.
    #[must_use]
    pub fn broken(&self) -> Option<String> {
        self.state().broken.clone()
    }

    /// Changes the settings by `patch`, an object: a member in it replaces that setting, `null`
    /// takes it back to the default. The result is written. Nothing is changed where the result is
    /// not valid, or where the file is broken (it is the person's to fix).
    pub fn update(&self, patch: &Map<String, Value>) -> Result<Settings, String> {
        let mut state = self.state();
        if let Some(why) = &state.broken {
            return Err(format!(
                "{why}; fix it or remove it, and toppen will keep your settings again"
            ));
        }
        let Ok(Value::Object(mut whole)) = serde_json::to_value(&state.settings) else {
            return Err("the settings cannot be read".to_owned());
        };
        for (name, value) in patch {
            if value.is_null() {
                whole.remove(name);
            } else {
                whole.insert(name.clone(), value.clone());
            }
        }
        let next: Settings =
            serde_json::from_value(Value::Object(whole)).map_err(|why| why.to_string())?;
        next.check()?;
        if let Some(path) = &self.path {
            write_atomically(path, &next)?;
        }
        state.settings = next.clone();
        Ok(next)
    }
}

/// Writes `settings` to `path` by writing a file beside it and renaming that over it.
fn write_atomically(path: &Path, settings: &Settings) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(settings).map_err(|why| why.to_string())?;
    text.push('\n');
    let dir = path.parent().ok_or("the settings have no folder")?;
    fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    let beside = path.with_extension("json.tmp");
    let written = fs::File::create(&beside)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())
                .and_then(|()| file.sync_all())
        })
        .and_then(|()| fs::rename(&beside, path));
    written.map_err(|why| {
        let _ = fs::remove_file(&beside);
        format!("{}: {why}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn patch(value: &Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn a_folder_with_no_file_is_the_defaults_and_nothing_is_written_until_a_change() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path());
        assert_eq!(store.get(), Settings::default());
        assert!(store.broken().is_none());
        assert!(!dir.path().join("settings.json").exists());
    }

    #[test]
    fn a_change_is_written_whole_and_read_again_by_the_next_run() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("toppen");
        let store = Store::open(&config);
        let made = store
            .update(&patch(&json!({
                "interval": 2.0, "sort": "memory", "descending": true, "mode": "tree",
                "hidden": ["virt"], "privacy": true, "keys": { "pause": ["p"], "mark": [] }
            })))
            .unwrap();
        assert_eq!(made.interval, Some(2.0));
        let again = Store::open(&config).get();
        assert_eq!(again, made);
        assert_eq!(again.keys["pause"].0, ["p"]);
        assert!(again.keys["mark"].0.is_empty());
        assert!(again.privacy);
        // No file is left beside it.
        let names: Vec<_> = fs::read_dir(&config)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings.json"]);
    }

    #[test]
    fn the_pages_empty_list_may_come_as_an_empty_object() {
        let store = Store::memory(Settings::default());
        let made = store
            .update(&patch(&json!({ "hidden": {}, "keys": { "mark": {} } })))
            .unwrap();
        assert_eq!(made.hidden, Some(Words::default()));
        assert!(made.keys["mark"].0.is_empty());
    }

    #[test]
    fn null_takes_a_setting_back_to_its_default_and_a_member_replaces_the_one_before() {
        let store = Store::memory(Settings::default());
        store
            .update(&patch(&json!({ "interval": 5.0, "mode": "group" })))
            .unwrap();
        let made = store.update(&patch(&json!({ "interval": null }))).unwrap();
        assert_eq!(made.interval, None);
        assert_eq!(made.mode.as_deref(), Some("group"));
    }

    #[test]
    fn a_value_that_is_not_valid_changes_nothing() {
        let store = Store::memory(Settings::default());
        for bad in [
            json!({ "interval": 0.1 }),
            json!({ "interval": 60 }),
            json!({ "mode": "sideways" }),
            json!({ "keys": { "pause": "space" } }),
            json!({ "keys": { "pause": [1] } }),
            json!({ "keys": { "pause": ["a", "b", "c", "d", "e", "f", "g"] } }),
            json!({ "privacy": "yes" }),
        ] {
            assert!(store.update(&patch(&bad)).is_err(), "{bad}");
        }
        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn a_broken_file_stays_as_it_is_the_defaults_are_used_and_nothing_is_written_over_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ \"interval\": ").unwrap();
        let store = Store::open(dir.path());
        assert_eq!(store.get(), Settings::default());
        let why = store.broken().unwrap();
        assert!(why.contains("settings.json is not used"), "{why}");
        let refused = store
            .update(&patch(&json!({ "privacy": true })))
            .unwrap_err();
        assert!(refused.contains("fix it or remove it"), "{refused}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ \"interval\": ");
        // A file that parses and is not valid is as broken.
        fs::write(&path, "{ \"mode\": \"sideways\" }").unwrap();
        assert!(Store::open(dir.path()).broken().is_some());
    }

    #[test]
    fn what_this_version_does_not_know_is_kept_when_the_file_is_written() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{ "privacy": true, "future": { "a": 1 } }"#,
        )
        .unwrap();
        let store = Store::open(dir.path());
        store.update(&patch(&json!({ "interval": 2.0 }))).unwrap();
        let text = fs::read_to_string(dir.path().join("settings.json")).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["future"], json!({ "a": 1 }));
        assert_eq!(value["privacy"], json!(true));
        assert_eq!(value["interval"], json!(2.0));
    }
}
