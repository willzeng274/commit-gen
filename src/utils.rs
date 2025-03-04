use std::path::PathBuf;
use anyhow::Result;
use crate::config::Config;

pub fn load_config(config_path: Option<PathBuf>) -> Result<Config> {
    if let Some(path) = config_path {
        let file = std::fs::read_to_string(path)?;
        return Ok(toml::from_str(&file)?);
    }
    // it doesn't make sense to use a macro here
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

    // find the config file with 4 path options
    let config_paths = [
        PathBuf::from("config/default.toml"),
        home.join(".config/commit-gen/config.toml"),
        home.join(".commit-gen/config.toml"),
        home.join(".commit-gen.toml"),
    ];
    // prefers .config/ over .commit-gen/ over .commit-gen.toml
    for path in &config_paths {
        if path.exists() {
            let file = std::fs::read_to_string(path)?;
            return Ok(toml::from_str(&file)?);
        }
    }
    // must have a config file to use commit-gen... specify root_dir
    Err(anyhow::anyhow!("Cannot find a config file"))
}