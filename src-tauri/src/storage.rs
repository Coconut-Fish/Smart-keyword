use std::fs;
use std::path::PathBuf;

use crate::models::PersistedState;

pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new() -> Result<Self, String> {
        let base_dir = dirs::config_dir()
            .ok_or_else(|| "Unable to resolve the user config directory.".to_string())?
            .join("smart-keyword");

        fs::create_dir_all(&base_dir)
            .map_err(|err| format!("Unable to create config directory: {err}"))?;

        Ok(Self {
            path: base_dir.join("state.json"),
        })
    }

    pub fn from_path(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Unable to create storage directory: {err}"))?;
        }

        Ok(Self { path })
    }

    pub fn load(&self) -> Result<PersistedState, String> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }

        let content = fs::read_to_string(&self.path)
            .map_err(|err| format!("Unable to read settings file: {err}"))?;
        serde_json::from_str(&content)
            .map_err(|err| format!("Unable to parse settings file: {err}"))
    }

    pub fn save(&self, state: &PersistedState) -> Result<(), String> {
        let content = serde_json::to_string_pretty(state)
            .map_err(|err| format!("Unable to serialize settings: {err}"))?;
        fs::write(&self.path, content)
            .map_err(|err| format!("Unable to write settings file: {err}"))
    }
}
