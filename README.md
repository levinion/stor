# Stor

Stor is an alternative to GNU Stow. It has more features and easy to use.

```shell
stor -t $HOME path/to/module
```

![show](./assets/show.png)

Stor also has some flags like stow:

- -t, --target DIR: target dir (defaults to $HOME)
- -n, --simulate: dry-run
- -D, --delete: remove previously linked or copied items
- -V, --version: show version number

and feature new:

- -c, --copy: copy instead of creating symlinks
- -f, --overwrite: if target file/dir exists, overwrite it without ask
- -v, --verbose / -q, --quiet: change log verbosity
- -I, --ignore \<glob> to ignore patterns

Some features are removed since not that useful:

- -d, --dir DIR is used to set workdir, but now module supports a path rather than a name.
- -i, --interactive: interactive mode, asking if there's different choices.

Advanced features is supposed to be added later:

- --adopt: used with -t, adopt a dir as a module then link/copy it to its original place


## Install

- Cargo:

```shell
cargo install --git "https://github.com/levinion/stor"
```

- AUR:

```shell
$AUR_HELPER -S stor
```

- Git:

```shell
git clone "https://github.com/levinion/stor"
cd stor
make
```

## Example

A usecase is like:

Assuming you have this structure:

```markdown
dotfiles/
└── modules/
    ├── nvim/.config/nvim/init.lua
    └── ...
```

Deploy all modules to your home directory with:

```shell
cd dotfiles
stor -t $HOME modules/*/
```

This creates symlinks or copies from `modules/*/…` into `$HOME/…` while preserving relative paths.

For example, `dotfiles/modules/nvim/.config/nvim` will be linked to `$HOME/.config/nvim`.

Since $HOME is the default target, the same result can be achieved with:

```shell
stor modules/*/
```

---

To recover changes, you can use the flag `-D`:

```shell
stor -D modules/*/
```

That means, `stor -D modules/nvim` will unlink the linked `$HOME/.config/nvim` directory.

---

To see what will be changed, use `-n` or `--simulate` before you execute any action:

```shell
stor -n modules/*/
```

## Advanced

Stor allows per-module configuration via a stor.toml file located at the root of each module (e.g., `<module_name>/stor.toml`).

The configuration file and all its fields are optional.

```toml
# Runs before installing module 
pre_install = "echo 'Installing module...'"

# Runs after all module are installed
post_install = "read -p 'Press enter to continue...'"

# Runs before removing module
pre_uninstall = "echo 'Cleaning up...'"

# Runs after module is removed
post_uninstall = "echo 'Module uninstalled'"

# Patterns to exclude from stor
ignore = ["**/.git", "**/.DS_Store"]
```

You could use `-N` or `--disable-hooks` to disable hooks if you don't like it.
