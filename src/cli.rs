use clap::Parser;

#[derive(Parser, Default)]
pub struct Args {
    #[arg(
        short = 't',
        long = "target",
        help = "Set target to DIR (default is $HOME)"
    )]
    pub target_dir: Option<String>,
    #[arg(
        short = 'n',
        long = "simulate",
        visible_alias = "dry-run",
        alias = "no",
        default_value_t = false,
        help = "Do not actually make any filesystem changes"
    )]
    pub simulate: bool,
    #[arg(
        short = 'D',
        long = "delete",
        default_value_t = false,
        help = "Unstow the package names"
    )]
    pub delete: bool,
    #[arg(
        short = 'R',
        long = "restow",
        default_value_t = false,
        help = "Restow (like stow -D followed by stow)"
    )]
    pub restow: bool,
    #[arg(
        short = 'c',
        long = "copy",
        default_value_t = false,
        help = "Copy instead of creating symlink"
    )]
    pub copy: bool,
    #[arg(
        short = 'f',
        long = "overwrite",
        default_value_t = false,
        help = "Delete if files/symlinks already exists"
    )]
    pub overwrite: bool,
    #[arg(
        short = 'V',
        long = "version",
        default_value_t = false,
        help = "Show version of stor"
    )]
    pub version: bool,
    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Change log verbosity to Debug"
    )]
    pub verbose: bool,
    #[arg(
        short = 'q',
        long,
        default_value_t = false,
        help = "Change log verbosity to Off"
    )]
    pub quiet: bool,
    #[arg(short = 'I', long, help = "Ignore pattern")]
    pub ignore: Vec<String>,
    #[arg(
        short = 'N',
        long,
        default_value_t = false,
        help = "Do not execute hooks"
    )]
    pub disable_hooks: bool,
    #[arg(short, long, default_value_t = false, help = "Run in interactive mode")]
    pub interactive: bool,
    pub modules: Vec<String>,
}
