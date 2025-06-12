use std::{
    fs::read_dir,
    io::Write,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::Colorize;
use log::{error, info, warn};

#[derive(Parser, Default)]
struct Args {
    #[arg(
        short = 't',
        long = "target",
        help = "Set target to DIR (default is $HOME)"
    )]
    targetdir: Option<String>,
    #[arg(
        short = 'n',
        long = "simulate",
        alias = "no",
        default_value_t = false,
        help = "Do not actually make any filesystem changes"
    )]
    simulate: bool,
    #[arg(
        short = 'D',
        long = "delete",
        default_value_t = false,
        help = "Unstow the package names"
    )]
    delete: bool,
    #[arg(
        short = 'R',
        long = "restow",
        default_value_t = false,
        help = "Restow (like stow -D followed by stow -S)"
    )]
    restow: bool,
    #[arg(
        short = 'c',
        long = "copy",
        default_value_t = false,
        help = "Copy instead of creating symlink"
    )]
    copy: bool,
    #[arg(
        short = 'f',
        long = "overwrite",
        default_value_t = false,
        help = "Delete if files/symlinks already exists"
    )]
    overwrite: bool,
    modules: Vec<String>,
}

struct Stor {
    args: Args,
}

impl Stor {
    fn new(mut args: Args) -> Stor {
        // handle default values
        if args.targetdir.is_none() {
            args.targetdir = Some(home::home_dir().unwrap().to_str().unwrap().to_string());
        }
        Self { args }
    }

    fn run(self) -> Result<()> {
        for module in &self.args.modules {
            // check input
            let module = module.parse::<PathBuf>().unwrap();
            if !module.is_dir() {
                warn!(
                    "{}",
                    format!(
                        "Skip: module {} doesn't exists or is a file",
                        module.display()
                    )
                    .yellow()
                );
                continue;
            }
            let target = self
                .args
                .targetdir
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
                continue;
            }

            // turn into absolute path
            let module = std::path::absolute(module).unwrap();
            let target = std::path::absolute(target).unwrap();

            // run commands
            if self.args.delete {
                // delete links and files
                self.unstow(&module, &target, &module)?;
            } else if self.args.restow {
                self.restow(&module, &target, &module)?;
            } else {
                // create links and files
                self.stow(&module, &target, &module)?;
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

    fn copy_or_link(&self, path: &Path, target: &Path) -> Result<()> {
        if self.args.copy {
            // copy is enabled.
            info!(
                "{}",
                format!("Copy: {} -> {}", path.display(), target.display()).cyan()
            );
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
        } else {
            // copy is diabled, use default symlink.
            info!(
                "{}",
                format!("Link: {} -> {}", path.display(), target.display()).cyan()
            );
            if !self.args.simulate {
                if path.is_dir() {
                    symlink(path, target).map_err(|err| anyhow!(err))?;
                } else if path.is_file() {
                    symlink(path, target).map_err(|err| anyhow!(err))?;
                }
            }
        }
        Ok(())
    }

    fn stow(&self, module: &Path, targetdir: &Path, current: &Path) -> Result<()> {
        for entry in read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let target = get_relative_target(path.as_path(), module, targetdir);

            // if target is a symlink
            if target.is_symlink() {
                // if the copy flag is on, then try to overwrite, give up if overwrite flag is off
                if self.args.copy {
                    if self.args.overwrite {
                        info!("{}", format!("Unlink: {}", target.display()).cyan());
                        if !self.args.simulate {
                            fs_extra::remove_items(&[&target])?;
                        }
                    } else {
                        warn!(
                            "{}",
                            format!("Skip: {} is not overwritten", target.display()).yellow()
                        );
                        continue;
                    }
                } else {
                    // if the symlink already points to the target path, then skip, otherwise try overwrite.
                    let origin = std::fs::read_link(&target)?;
                    if origin == path {
                        info!(
                            "{}",
                            format!("Skip: {} already exists", target.display()).cyan()
                        );
                        continue;
                    } else {
                        #[allow(clippy::collapsible_else_if)]
                        if self.args.overwrite {
                            info!("{}", format!("Delete: {}", target.display()).cyan());
                            if !self.args.simulate {
                                fs_extra::remove_items(&[&target])?;
                            }
                        } else {
                            warn!(
                                "{}",
                                format!("Skip: {} is not overwritten", target.display()).yellow()
                            );
                            continue;
                        }
                    }
                }
            }

            // try overwrite or skip if there's any target already exists
            if path.is_file() && target.exists() {
                if self.args.overwrite {
                    warn!("{}", format!("Delete: {}", target.display()).yellow());
                    if !self.args.simulate {
                        fs_extra::remove_items(&[&target])?;
                    }
                } else {
                    warn!(
                        "{}",
                        format!("Skip: {} is not overwritten, skip...", target.display()).yellow()
                    );
                    continue;
                }
            }

            // if target not exists, copy or link path to it.
            if !target.exists() {
                self.copy_or_link(&path, &target)?;
                continue;
            }

            // if target is a dir, then repeat.
            if target.is_dir() {
                self.stow(module, targetdir, &path)?;
            }
        }
        Ok(())
    }

    fn unstow(&self, module: &Path, targetdir: &Path, current: &Path) -> Result<()> {
        for entry in read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let target = get_relative_target(path.as_path(), module, targetdir);

            // if target exists, remove it
            if target.is_symlink() {
                info!("{}", format!("Unlink: {}", target.display()).cyan());
                if !self.args.simulate {
                    fs_extra::remove_items(&[&target])?;
                }
            } else if target.is_file() {
                info!("{}", format!("Delete: {}", target.display()).cyan());
                if !self.args.simulate {
                    fs_extra::remove_items(&[&target])?;
                }
                // father is empty
                if target.parent().unwrap().read_dir()?.next().is_none() {
                    #[allow(clippy::collapsible_if)]
                    if !self.args.simulate {
                        fs_extra::remove_items(&[&target.parent().unwrap()])?;
                    }
                }
            }
            // if target is a dir, then repeat.
            else if target.is_dir() {
                self.unstow(module, targetdir, &path)?;
            }
        }
        Ok(())
    }

    fn restow(&self, module: &Path, targetdir: &Path, current: &Path) -> Result<()> {
        self.unstow(module, targetdir, current)?;
        self.stow(module, targetdir, current)?;
        Ok(())
    }
}

// calculate the target path based on src(file/dir path), root(dotfile dir path) and dst(target dir path)
fn get_relative_target(src: &Path, root: &Path, dst: &Path) -> PathBuf {
    let relative_path = src.strip_prefix(root).unwrap();
    dst.join(relative_path)
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {}",
                record.level().to_string().magenta(),
                record.args()
            )
        })
        .init();
    let args = Args::parse();
    let stor = Stor::new(args);
    if let Err(err) = stor.run() {
        error!("{}", err);
    }
}
