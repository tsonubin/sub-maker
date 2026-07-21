use std::path::PathBuf;

use crate::config::{self, SetupConfig};

pub fn config_path() -> PathBuf {
    PathBuf::from(config::etc_base())
        .join("sub-maker")
        .join("config.yaml")
}

pub fn nodes_path() -> PathBuf {
    PathBuf::from(config::etc_base())
        .join("sub-maker")
        .join("nodes.txt")
}

pub fn load_config() -> anyhow::Result<SetupConfig> {
    let data = std::fs::read_to_string(config_path())?;
    let cfg: SetupConfig = serde_yaml::from_str(&data)?;
    Ok(cfg)
}
