use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize, Clone)]
pub struct AppConfig {
    pub ontology: Option<PathBuf>,
    pub rules: Option<PathBuf>,
    pub db: Option<PathBuf>,
}

impl AppConfig {
    pub fn load_from_current_dir() -> Option<Self> {
        let path = Path::new("ctxset.toml");
        if !path.exists() {
            return None;
        }
        let raw = fs::read_to_string(path).ok()?;
        toml::from_str::<Self>(&raw).ok()
    }

    
}
