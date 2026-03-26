use anyhow::Result;
use colored::Colorize;
use fs_extra::file::read_to_string;
use log::info;
use std::path::Path;

#[derive(Default, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub pre_install: Option<String>,
    pub post_install: Option<String>,
    pub pre_uninstall: Option<String>,
    pub post_uninstall: Option<String>,
    pub ignore: Vec<String>,
}

impl Config {
    pub fn from(module: &Path) -> Result<Self> {
        let path = module.join("stor.toml");
        if path.is_file() {
            info!("{}", format!("Found config: {}", path.display()).cyan());
        }
        let s = read_to_string(&path)?;
        let config: Config = toml::from_str(&s)?;
        Ok(config)
    }
}
