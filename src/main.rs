mod cli;
mod config;

use clap::Parser;
use inquire::Confirm;

use std::{
    collections::HashSet,
    fs::read_dir,
    io::Write,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Result, anyhow};
use colored::{ColoredString, Colorize};

use log::{debug, error, info, warn};

use crate::config::Config;

#[derive(Default)]
struct Stor {
    args: cli::Args,
    removed: HashSet<PathBuf>,
}

impl Stor {
    fn new(mut args: cli::Args) -> Stor {
        // handle default values
        if args.target_dir.is_none() {
            args.target_dir = Some(home::home_dir().unwrap().to_str().unwrap().to_string());
        }
        Self {
            args,
            ..Default::default()
        }
    }

    fn print_version(self) {
        let version = env!("CARGO_PKG_VERSION");
        println!("stor version {}", version);
    }

    fn run(mut self) -> Result<()> {
        // show version
        if self.args.version {
            self.print_version();
            return Ok(());
        }
        // range all input modules
        for module in self.args.modules.clone() {
            // check input
            let module = module.parse::<PathBuf>().unwrap();
            if let Err(err) = self.deploy_module(module) {
                error!("{}", err);
            }
        }

        if self.args.simulate {
            warn!(
                "{}",
                "Simulate: in simulation mode so not modifying filesystem.".yellow()
            );
        }
        Ok(())
    }

    fn deploy_module(&mut self, module: PathBuf) -> Result<()> {
        if !module.is_dir() {
            warn!(
                "{}",
                format!(
                    "Skip: module {} doesn't exists or is a file",
                    module.display()
                )
                .yellow()
            );
            return Ok(());
        }
        let target = self
            .args
            .target_dir
            .as_ref()
            .unwrap()
            .parse::<PathBuf>()
            .unwrap();
        if !target.is_dir() {
            warn!(
                "{}",
                format!(
                    "Skip: target {} doesn't exists or is a file",
                    target.display()
                )
                .yellow()
            );
            return Ok(());
        }

        // turn into absolute path
        let module = std::path::absolute(module).unwrap();
        let target = std::path::absolute(target).unwrap();

        // parse config
        let mut config = Config::from(&module).unwrap_or_default();
        config.ignore.push("stor.toml".to_string());
        config.ignore.extend(self.args.ignore.clone());
        debug!(
            "{}",
            format!("ignore: {:?}", config.ignore.clone()).truecolor(150, 150, 150)
        );

        // run commands
        if self.args.delete {
            // delete links and files
            self.unstow(&module, &target, &module, &config)?;
        } else if self.args.restow {
            self.restow(&module, &target, &module, &config)?;
        } else {
            // create links and files
            self.stow(&module, &target, &module, &config)?;
        }

        Ok(())
    }

    fn confirm(&self, message: ColoredString) -> bool {
        if !self.args.interactive {
            return true;
        }
        Confirm::new(&message.to_string())
            .with_default(true)
            .prompt()
            .unwrap_or(true)
    }

    fn copy(&mut self, path: &Path, target: &Path) -> Result<()> {
        let info = format!("Copy: {} -> {}", path.display(), target.display()).cyan();
        if !self.args.interactive {
            info!("{}", info);
        } else if !self.confirm(info) {
            return Ok(());
        }
        if path.is_dir() {
            if !self.args.simulate {
                let options = fs_extra::dir::CopyOptions::default();
                fs_extra::dir::copy(path, target.parent().unwrap(), &options)?;
            }
        } else if path.is_file() {
            #[allow(clippy::collapsible_if)]
            if !self.args.simulate {
                let options = fs_extra::file::CopyOptions::default();
                fs_extra::file::copy(path, target, &options)?;
            }
        }
        if self.removed.contains(target) {
            self.removed.remove(target);
        }
        Ok(())
    }

    fn link(&mut self, path: &Path, target: &Path) -> Result<()> {
        let info = format!("Link: {} -> {}", path.display(), target.display()).cyan();
        if !self.args.interactive {
            info!("{}", info);
        } else if !self.confirm(info) {
            return Ok(());
        }
        if !self.args.simulate {
            if path.is_dir() {
                symlink(path, target).map_err(|err| anyhow!(err))?;
            } else if path.is_file() {
                symlink(path, target).map_err(|err| anyhow!(err))?;
            }
        }
        if self.removed.contains(target) {
            self.removed.remove(target);
        }
        Ok(())
    }

    fn remove(&mut self, target: &Path) -> Result<()> {
        let action = if target.is_symlink() {
            "Unlink"
        } else {
            "Delete"
        };
        let info = format!("{}: {}", action, target.display()).red();
        if !self.args.delete {
            if self.args.overwrite {
                if !self.args.interactive {
                    info!("{}", info);
                } else if !self.confirm(info) {
                    return Ok(());
                }
                self.removed.insert(target.to_path_buf());
                if !self.args.simulate {
                    fs_extra::remove_items(&[&target])?;
                }
            } else if !self.args.interactive {
                warn!(
                    "{}",
                    format!("Skip: {} is not overwritten", target.display()).yellow()
                );
            } else if self.confirm(info) {
                self.removed.insert(target.to_path_buf());
                if !self.args.simulate {
                    fs_extra::remove_items(&[&target])?;
                }
            }
        } else {
            if !self.args.interactive {
                info!("{}", info);
            } else if !self.confirm(info) {
                return Ok(());
            }
            self.removed.insert(target.to_path_buf());
            if !self.args.simulate {
                fs_extra::remove_items(&[&target])?;
            }
        }
        Ok(())
    }

    fn exists(&self, target: &Path) -> bool {
        if !self.args.simulate {
            target.exists()
        } else {
            !self.removed.contains(target) && target.exists()
        }
    }

    fn execute_hook(&self, name: &str, hook: &str) -> Result<()> {
        if !self.args.disable_hooks {
            let info = format!("{}: {}", name, hook).white();
            if !self.args.interactive {
                info!("{}", info);
            } else if !self.confirm(info) {
                return Ok(());
            }
        } else {
            warn!("{}", format!("Skip {}: {}", name, hook).yellow());
        }
        if !self.args.simulate && !self.args.disable_hooks {
            Command::new("sh").args(["-c", hook]).status()?;
        }
        Ok(())
    }

    fn stow(
        &mut self,
        module: &Path,
        target_dir: &Path,
        current: &Path,
        config: &Config,
    ) -> Result<()> {
        // pre-install hook
        if let Some(hook) = &config.pre_install
            && current == module
        {
            self.execute_hook("pre-install", hook)?;
        }

        for entry in read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let target = get_relative_target(path.as_path(), module, target_dir);

            // ignore matching pattern
            let mut matched = false;
            let relative_path = path.strip_prefix(module)?;
            for pattern in &config.ignore {
                if fast_glob::glob_match(pattern, relative_path.to_str().unwrap()) {
                    matched = true;
                    break;
                }
            }
            if matched {
                debug!(
                    "{}",
                    format!("{} is ignored", path.display()).truecolor(150, 150, 150)
                );
                continue;
            }

            let exist = if self.exists(&target) {
                " (exists)"
            } else {
                ""
            };

            debug!(
                "{}",
                format!("{} -> {}{}", path.display(), target.display(), exist)
                    .truecolor(150, 150, 150)
            );

            // if target is a symlink
            if self.exists(&target) && target.is_symlink() {
                // if the copy flag is on, then try to overwrite, give up if overwrite flag is off
                if self.args.copy {
                    self.remove(&target)?;
                } else {
                    // if the symlink already points to the target path, then skip, otherwise try overwrite.
                    let origin = std::fs::read_link(&target)?;
                    if origin == path {
                        info!(
                            "{}",
                            format!("Skip: {} already exists", target.display()).yellow()
                        );
                        continue;
                    } else {
                        self.remove(&target)?;
                    }
                }
            }

            // try overwrite or skip if there's any target already exists
            if self.exists(&target) && path.is_file() {
                self.remove(&target)?;
            }

            // if target not exists, copy or link path to it.
            if !self.exists(&target) {
                if self.args.copy {
                    self.copy(&path, &target)?;
                } else {
                    self.link(&path, &target)?;
                }
                continue;
            }

            // if target is a dir, then repeat.
            if self.exists(&target) && target.is_dir() {
                self.stow(module, target_dir, &path, config)?;
            }
        }

        // post-install hook
        if let Some(hook) = &config.post_install
            && current == module
        {
            self.execute_hook("post-install", hook)?;
        }

        Ok(())
    }

    fn unstow(
        &mut self,
        module: &Path,
        target_dir: &Path,
        current: &Path,
        config: &Config,
    ) -> Result<()> {
        // pre-uninstall hook
        if let Some(hook) = &config.pre_uninstall
            && current == module
        {
            self.execute_hook("pre-uninstall", hook)?;
        }

        for entry in read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let target = get_relative_target(path.as_path(), module, target_dir);

            // ignore matching pattern
            let mut matched = false;
            let relative_path = path.strip_prefix(module)?;
            for pattern in &config.ignore {
                if fast_glob::glob_match(pattern, relative_path.to_str().unwrap()) {
                    matched = true;
                    break;
                }
            }

            if matched {
                debug!(
                    "{}",
                    format!("{} is ignored", path.display()).truecolor(150, 150, 150)
                );
                continue;
            }

            debug!(
                "{}",
                format!("{} -> {}", path.display(), target.display()).truecolor(150, 150, 150)
            );

            // if target exists, remove it
            if target.is_symlink() {
                self.remove(&target)?;
            } else if target.is_file() {
                self.remove(&target)?;
                // father is empty
                if target.parent().unwrap().read_dir()?.next().is_none() {
                    self.remove(target.parent().unwrap())?;
                }
            }
            // if target is a dir, then repeat.
            else if target.is_dir() {
                self.unstow(module, target_dir, &path, config)?;
            }
        }

        // post-uninstall hook
        if let Some(hook) = &config.post_uninstall
            && current == module
        {
            self.execute_hook("post-uninstall", hook)?;
        }

        Ok(())
    }

    // a wrapper of unstow and stow
    fn restow(
        &mut self,
        module: &Path,
        targetdir: &Path,
        current: &Path,
        config: &Config,
    ) -> Result<()> {
        self.unstow(module, targetdir, current, config)?;
        self.stow(module, targetdir, current, config)?;
        Ok(())
    }
}

// calculate the target path based on src(file/dir path), root(dotfile dir path) and dst(target dir path)
fn get_relative_target(src: &Path, root: &Path, dst: &Path) -> PathBuf {
    let relative_path = src.strip_prefix(root).unwrap();
    dst.join(relative_path)
}

fn main() {
    let args = cli::Args::parse();
    let log_level = match (args.quiet, args.verbose) {
        (true, false) => log::LevelFilter::Off,
        (false, true) => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    };
    env_logger::builder()
        .filter_level(log_level)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {}",
                record.level().to_string().magenta(),
                record.args()
            )
        })
        .init();
    let stor = Stor::new(args);
    if let Err(err) = stor.run() {
        error!("{}", err);
    }
}
