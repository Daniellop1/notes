use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub theme: String,
    pub notes_dir: PathBuf,
    pub default_bits: u16,
}

impl Default for Config {
    fn default() -> Self {
        let mut notes_dir = home::home_dir().expect("Could not find home directory");
        notes_dir.push(".local");
        notes_dir.push("share");
        notes_dir.push("mynotes");

        Self {
            theme: "default".to_string(),
            notes_dir,
            default_bits: 256,
        }
    }
}

impl Config {
    pub fn load_or_default() -> Self {
        let mut config_path = home::home_dir().expect("Could not find home directory");
        config_path.push(".config");
        config_path.push("mynotes");
        config_path.push("config.toml");

        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        
        let config = Self::default();
        let _ = config.save(); // Try to save default if missing
        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_dir = home::home_dir().expect("Could not find home directory");
        config_dir.push(".config");
        config_dir.push("mynotes");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        let mut config_path = config_dir.clone();
        config_path.push("config.toml");

        let toml_str = toml::to_string_pretty(self)?;
        fs::write(config_path, toml_str)?;
        Ok(())
    }
}
