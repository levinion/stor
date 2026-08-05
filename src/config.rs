use anyhow::Result;
use colored::Colorize;
use fs_extra::file::read_to_string;
use log::info;
use std::path::{Path, PathBuf};

#[derive(Default, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub ignore: Vec<String>,
}

impl Config {
    // global config (~/.config/stor/stor.toml) merged with project config (<module>/stor.toml)
    pub fn from(module: &Path) -> Result<Self> {
        let mut config = Self::read(&global_config_path())?;
        let project = Self::read(&module.join("stor.toml"))?;
        config.ignore.extend(project.ignore);
        Ok(config)
    }

    fn read(path: &Path) -> Result<Self> {
        if path.is_file() {
            info!("{}", format!("Found config: {}", path.display()).cyan());
            let s = read_to_string(path)?;
            Ok(toml::from_str(&s)?)
        } else {
            Ok(Self::default())
        }
    }
}

// follow XDG: use $XDG_CONFIG_HOME if set, otherwise ~/.config
fn global_config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home::home_dir().unwrap().join(".config"))
        .join("stor/stor.toml")
}
