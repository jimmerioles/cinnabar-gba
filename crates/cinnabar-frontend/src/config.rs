//! Optional TOML configuration.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub scale: Option<u32>,
    pub shader: Option<String>,
    pub volume: Option<f32>,
    pub library: Option<PathBuf>,
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

pub fn load(explicit: Option<&Path>) -> Result<Config> {
    let Some(path) = explicit.map(Path::to_path_buf).or_else(default_path) else {
        return Ok(Config::default());
    };
    if !path.exists() {
        if explicit.is_some() {
            bail!("config file not found: {}", path.display());
        }
        return Ok(Config::default());
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse config {}", path.display()))
}

pub fn default_path() -> Option<PathBuf> {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("cinnabar/config.toml"));
    }
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("cinnabar")
            .join("config.toml")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_documented_shape() {
        let config: Config = toml::from_str(
            r#"
scale = 3
shader = "lcd,color"
volume = 0.8

[keys]
A = "a, z"
L = "q"
"#,
        )
        .expect("valid config");
        assert_eq!(config.scale, Some(3));
        assert_eq!(config.shader.as_deref(), Some("lcd,color"));
        assert_eq!(config.volume, Some(0.8));
        assert_eq!(config.keys.get("A").map(String::as_str), Some("a, z"));
    }

    #[test]
    fn rejects_misspelled_fields() {
        assert!(toml::from_str::<Config>("scalee = 3").is_err());
    }
}
