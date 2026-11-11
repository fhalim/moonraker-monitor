use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub base_url: Option<String>,
    /// uids of webcams selected for display
    #[serde(default)]
    pub cameras: Vec<String>,
}

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("config.json"))
        .map_err(|e| e.to_string())
}

pub fn load(app: &AppHandle) -> Config {
    path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, config: &Config) -> Result<(), String> {
    let p = path(app)?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| format!("Cannot save config: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_config_without_cameras_still_parses() {
        let c: Config = serde_json::from_str(r#"{"baseUrl":"http://p:7125"}"#).unwrap();
        assert_eq!(c.base_url.as_deref(), Some("http://p:7125"));
        assert!(c.cameras.is_empty());
    }
}
