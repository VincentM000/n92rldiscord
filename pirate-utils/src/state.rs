use serde::de::DeserializeOwned;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves a state file path, honoring the `STATE_DIR` environment variable when set.
pub fn resolve_state_path(filename: &str) -> PathBuf {
    if let Ok(dir) = env::var("STATE_DIR") {
        Path::new(&dir).join(filename)
    } else {
        Path::new(filename).to_path_buf()
    }
}

/// Loads raw text state from disk, returning an empty string if the file does not exist.
pub fn load_state(filename: &str) -> String {
    let path = resolve_state_path(filename);
    if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    }
}

/// Saves raw text state to disk, creating parent directories if needed.
pub fn save_state(filename: &str, content: &str) -> std::io::Result<()> {
    let path = resolve_state_path(filename);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::write(path, content)
}

/// Loads typed JSON state from disk.
/// Returns `(state, is_first_run)` where `is_first_run` is `true` if the state file did not exist or failed to parse.
pub fn load_json_state<T: DeserializeOwned + Default>(filename: &str) -> (T, bool) {
    let path = resolve_state_path(filename);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<T>(&content) {
                return (parsed, false);
            }
        }
    }
    (T::default(), true)
}

/// Saves typed JSON state to disk in pretty-printed JSON format.
pub fn save_json_state<T: Serialize>(
    filename: &str,
    state: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = serde_json::to_string_pretty(state)?;
    save_state(filename, &content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct DummyState {
        counter: u32,
    }

    #[test]
    fn test_json_state_roundtrip() {
        let temp_file = "test_pirate_state_roundtrip.json";
        let _ = fs::remove_file(resolve_state_path(temp_file));

        let (initial, is_first) = load_json_state::<DummyState>(temp_file);
        assert!(is_first);
        assert_eq!(initial.counter, 0);

        let updated = DummyState { counter: 42 };
        save_json_state(temp_file, &updated).expect("Failed to save JSON state");

        let (reloaded, is_first_after) = load_json_state::<DummyState>(temp_file);
        assert!(!is_first_after);
        assert_eq!(reloaded, updated);

        let _ = fs::remove_file(resolve_state_path(temp_file));
    }
}
