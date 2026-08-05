#!/usr/bin/env bash
#
# stor — integration test suite.
#
# Builds the binary and exercises it end-to-end in a sandboxed environment
# (isolated $HOME and $XDG_CONFIG_HOME), covering stow/unstow/restow,
# simulate/copy/overwrite/ignore, config files and --adopt.
#
# Usage:
#   tests/test.sh            # build the debug binary and run the tests
#   tests/test.sh --release  # build the release binary and run the tests
#
# Exit status is 0 if every test passes, 1 otherwise.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/debug/stor"

if [[ "${1:-}" == "--release" ]]; then
    BIN="$ROOT/target/release/stor"
    cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml"
else
    cargo build --quiet --manifest-path "$ROOT/Cargo.toml"
fi

echo "stor under test: $BIN"
"$BIN" -V

# sandbox: every test works inside $WORK and never touches the real $HOME
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
FAILURES=()

# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------

# run stor with an isolated environment; `-t` is given by each test
stor() {
    HOME="$WORK/home" XDG_CONFIG_HOME="$WORK/xdg" "$BIN" "$@"
}

# run stor without -t (target defaults to $HOME), with $HOME overridden
stor_default_home() {
    local home="$1"; shift
    HOME="$home" XDG_CONFIG_HOME="$WORK/xdg" "$BIN" "$@"
}

# Note: `exit` (not `return`) is deliberate. Tests run in a subshell that is
# part of an `if`/`||` condition, where bash suspends `set -e`, so a plain
# `return 1` would not stop the test and it could "pass" despite failures.
fail() {
    echo "    FAIL: $1"
    exit 1
}

assert_file() {
    [[ -f "$1" ]] || { fail "expected file: $1"; return 1; }
}
assert_dir() {
    [[ -d "$1" ]] || { fail "expected dir: $1"; return 1; }
}
assert_absent() {
    [[ ! -e "$1" ]] || { fail "expected absent: $1"; return 1; }
}
assert_link() {
    [[ -L "$1" ]] || { fail "expected symlink: $1"; return 1; }
}
assert_symlink_to() {
    local path="$1" want="$2"
    [[ -L "$path" ]] || { fail "expected symlink: $path"; return 1; }
    local got
    got="$(readlink "$path")"
    [[ "$got" == "$want" ]] || { fail "symlink $path -> $got, want -> $want"; return 1; }
}
assert_content() {
    local file="$1" want="$2"
    assert_file "$file" || return 1
    local got
    got="$(cat "$file")"
    [[ "$got" == "$want" ]] || { fail "content of $file is '$got', want '$want'"; return 1; }
}

# run a single test function in a subshell; tool logs (stderr) are saved and
# shown on failure
run_test() {
    local name="$1"; shift
    echo "== $name =="
    mkdir -p "$WORK/logs"
    local log="$WORK/logs/$name.log"
    if ( "$name" ) 2>"$log"; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1))
        FAILURES+=("$name")
        echo "    FAILED"
        sed 's/^/    | /' "$log" | tail -30
    fi
}

# ---------------------------------------------------------------------------
# basic flags
# ---------------------------------------------------------------------------

test_version() {
    local out
    out="$(stor -V)" || return 1
    grep -q "stor version" <<<"$out" || return 1
    echo "    ok: version printed"
}

test_help() {
    local out
    out="$(stor --help)" || return 1
    grep -q -- "--adopt" <<<"$out" || return 1
    echo "    ok: --adopt documented"
}

test_default_target_home() {
    local t="$WORK/default_home"
    mkdir -p "$t/home" "$t/mod"
    echo "bashrc" > "$t/mod/.bashrc"
    stor_default_home "$t/home" "$t/mod"
    assert_symlink_to "$t/home/.bashrc" "$t/mod/.bashrc"
    assert_content "$t/home/.bashrc" "bashrc"
    echo "    ok: target defaults to \$HOME"
}

test_skip_invalid() {
    local t="$WORK/skip_invalid"
    mkdir -p "$t/home"
    echo "f" > "$t/notadir"
    stor -t "$t/home" "$t/notadir"            # module is a file -> skip
    stor -t "$t/missing_target" "$t/home"     # target missing -> skip
    echo "    ok: invalid module/target skipped"
}

# ---------------------------------------------------------------------------
# stow / unstow / restow
# ---------------------------------------------------------------------------

test_stow_link() {
    local t="$WORK/stow_link"
    mkdir -p "$t/home" "$t/mod/.config/nvim"
    echo "init" > "$t/mod/.config/nvim/init.lua"
    stor -t "$t/home" "$t/mod"
    # stor links the outermost dir that doesn't exist in the target yet
    assert_symlink_to "$t/home/.config" "$t/mod/.config"
    assert_content "$t/home/.config/nvim/init.lua" "init"
    echo "    ok: symlink created"
}

test_stow_already_stowed() {
    local t="$WORK/stow_twice"
    mkdir -p "$t/home" "$t/mod"
    echo "x" > "$t/mod/x.txt"
    stor -t "$t/home" "$t/mod"
    stor -t "$t/home" "$t/mod"                # second run: already stowed
    assert_symlink_to "$t/home/x.txt" "$t/mod/x.txt"
    echo "    ok: re-stowing is a no-op"
}

test_existing_target_skip() {
    local t="$WORK/existing"
    mkdir -p "$t/home" "$t/mod"
    echo "live" > "$t/home/x.txt"
    echo "mod" > "$t/mod/x.txt"
    stor -t "$t/home" "$t/mod"
    assert_file "$t/home/x.txt"               # untouched, not a symlink
    assert_content "$t/home/x.txt" "live"
    echo "    ok: existing target not overwritten without -f"
}

test_overwrite() {
    local t="$WORK/overwrite"
    mkdir -p "$t/home" "$t/mod"
    echo "live" > "$t/home/x.txt"
    echo "mod" > "$t/mod/x.txt"
    stor -f -t "$t/home" "$t/mod"
    assert_symlink_to "$t/home/x.txt" "$t/mod/x.txt"
    assert_content "$t/home/x.txt" "mod"
    echo "    ok: -f replaces existing target"
}

test_copy() {
    local t="$WORK/copy"
    mkdir -p "$t/home" "$t/mod"
    echo "data" > "$t/mod/data.txt"
    stor -c -t "$t/home" "$t/mod"
    assert_file "$t/home/data.txt"            # real copy, not a symlink
    assert_content "$t/home/data.txt" "data"
    echo "    ok: -c copies instead of linking"
}

test_simulate() {
    local t="$WORK/simulate"
    mkdir -p "$t/home" "$t/mod/.config"
    echo "a" > "$t/mod/.config/a.txt"
    stor -n -t "$t/home" "$t/mod"
    assert_absent "$t/home/.config/a.txt"
    assert_file "$t/mod/.config/a.txt"
    echo "    ok: -n changes nothing"
}

test_delete() {
    local t="$WORK/delete"
    mkdir -p "$t/home" "$t/mod/.config/nvim"
    echo "init" > "$t/mod/.config/nvim/init.lua"
    stor -t "$t/home" "$t/mod"
    assert_link "$t/home/.config"
    stor -D -t "$t/home" "$t/mod"
    assert_absent "$t/home/.config"           # link removed
    assert_content "$t/mod/.config/nvim/init.lua" "init"  # module intact
    echo "    ok: -D removes links only"
}

test_restow() {
    local t="$WORK/restow"
    mkdir -p "$t/home" "$t/mod"
    echo "v1" > "$t/mod/app.conf"
    stor -t "$t/home" "$t/mod"
    rm "$t/home/app.conf"                     # replace link with a real file
    echo "changed" > "$t/home/app.conf"
    # without -f, -R refuses to clobber the real file
    stor -R -t "$t/home" "$t/mod"
    assert_file "$t/home/app.conf"
    assert_content "$t/home/app.conf" "changed"
    # with -f it deletes the real file and re-links
    stor -R -f -t "$t/home" "$t/mod"
    assert_symlink_to "$t/home/app.conf" "$t/mod/app.conf"
    echo "    ok: -R unstows then stows again"
}

test_delete_keeps_target_root() {
    local t="$WORK/delete_root"
    mkdir -p "$t/home" "$t/mod"
    echo "x" > "$t/mod/x.txt"
    stor -t "$t/home" "$t/mod"
    rm "$t/home/x.txt"                         # replace link with a real file
    echo "live" > "$t/home/x.txt"
    stor -D -t "$t/home" "$t/mod"
    assert_absent "$t/home/x.txt"             # real file removed by -D
    assert_dir "$t/home"                       # target dir itself survives
    echo "    ok: -D never removes the target dir itself"
}

# ---------------------------------------------------------------------------
# ignore & config
# ---------------------------------------------------------------------------

test_ignore_cli() {
    local t="$WORK/ignore_cli"
    mkdir -p "$t/home" "$t/mod"
    echo "keep" > "$t/mod/b.txt"
    echo "skip" > "$t/mod/a.tmp"
    stor -I '*.tmp' -t "$t/home" "$t/mod"
    assert_symlink_to "$t/home/b.txt" "$t/mod/b.txt"
    assert_absent "$t/home/a.tmp"
    echo "    ok: -I ignores matching patterns"
}

test_config_project() {
    local t="$WORK/config_project"
    mkdir -p "$t/home" "$t/mod/.cache"
    echo "keep" > "$t/mod/app.txt"
    echo "cached" > "$t/mod/.cache/data"
    printf 'ignore = ["**/.cache"]\n' > "$t/mod/stor.toml"
    stor -t "$t/home" "$t/mod"
    assert_symlink_to "$t/home/app.txt" "$t/mod/app.txt"
    assert_absent "$t/home/.cache"
    assert_absent "$t/home/stor.toml"         # stor.toml is never stowed
    echo "    ok: project config honored"
}

test_config_global() {
    local t="$WORK/config_global"
    mkdir -p "$t/home" "$t/mod" "$WORK/xdg/stor"
    echo "keep" > "$t/mod/app.txt"
    echo "ds" > "$t/mod/.DS_Store"
    printf 'ignore = ["**/.DS_Store"]\n' > "$WORK/xdg/stor/stor.toml"
    stor -t "$t/home" "$t/mod"
    assert_symlink_to "$t/home/app.txt" "$t/mod/app.txt"
    assert_absent "$t/home/.DS_Store"
    echo "    ok: global config honored"
}

# ---------------------------------------------------------------------------
# --adopt
# ---------------------------------------------------------------------------

test_adopt_dir() {
    local t="$WORK/adopt_dir"
    mkdir -p "$t/home/.config/nvim" "$t/mod/.config/nvim"   # empty module skeleton
    echo "v2" > "$t/home/.config/nvim/init.lua"
    echo "colors" > "$t/home/.config/nvim/colors.txt"
    stor -t "$t/home" --adopt "$t/mod"
    assert_symlink_to "$t/home/.config/nvim" "$t/mod/.config/nvim"
    assert_content "$t/mod/.config/nvim/init.lua" "v2"
    assert_content "$t/mod/.config/nvim/colors.txt" "colors"
    echo "    ok: whole dir adopted and linked back"
}

test_adopt_file() {
    local t="$WORK/adopt_file"
    mkdir -p "$t/home" "$t/mod"
    echo "live" > "$t/home/.bashrc"
    echo "placeholder" > "$t/mod/.bashrc"
    stor -t "$t/home" --adopt "$t/mod"
    assert_content "$t/mod/.bashrc" "live"    # live version wins
    assert_symlink_to "$t/home/.bashrc" "$t/mod/.bashrc"
    echo "    ok: file adopted over placeholder"
}

test_adopt_merge() {
    local t="$WORK/adopt_merge"
    mkdir -p "$t/home/.config/nvim" "$t/mod/.config/nvim"
    echo "live" > "$t/home/.config/nvim/init.lua"
    echo "module" > "$t/mod/.config/nvim/init.lua"
    echo "extra" > "$t/home/.config/nvim/extra.txt"  # only in target
    stor -t "$t/home" --adopt "$t/mod"
    assert_content "$t/mod/.config/nvim/init.lua" "live"
    assert_symlink_to "$t/home/.config/nvim/init.lua" "$t/mod/.config/nvim/init.lua"
    assert_content "$t/home/.config/nvim/extra.txt" "extra"  # unrelated file untouched
    echo "    ok: non-empty module dir merged entry by entry"
}

test_adopt_copy() {
    local t="$WORK/adopt_copy"
    mkdir -p "$t/home" "$t/mod"
    echo "live" > "$t/home/conf.ini"
    echo "old" > "$t/mod/conf.ini"
    stor -c -t "$t/home" --adopt "$t/mod"
    assert_content "$t/mod/conf.ini" "live"
    assert_file "$t/home/conf.ini"            # real copy, not a symlink
    assert_content "$t/home/conf.ini" "live"
    echo "    ok: -c --adopt copies instead of linking"
}

test_adopt_simulate() {
    local t="$WORK/adopt_sim"
    mkdir -p "$t/home/.config/nvim" "$t/mod/.config/nvim"
    echo "live" > "$t/home/.config/nvim/init.lua"
    stor -n -t "$t/home" --adopt "$t/mod"
    assert_dir "$t/home/.config/nvim"         # still a real dir
    assert_content "$t/home/.config/nvim/init.lua" "live"
    assert_absent "$t/mod/.config/nvim/init.lua"  # nothing moved into module
    echo "    ok: -n --adopt changes nothing"
}

test_adopt_idempotent() {
    local t="$WORK/adopt_idem"
    mkdir -p "$t/home/.config/nvim" "$t/mod/.config/nvim"
    echo "live" > "$t/home/.config/nvim/init.lua"
    stor -t "$t/home" --adopt "$t/mod"
    stor -t "$t/home" --adopt "$t/mod"        # second run: already stowed
    assert_symlink_to "$t/home/.config/nvim" "$t/mod/.config/nvim"
    assert_content "$t/mod/.config/nvim/init.lua" "live"
    echo "    ok: re-running --adopt is a no-op"
}

test_adopt_delete() {
    local t="$WORK/adopt_delete"
    mkdir -p "$t/home/.config/nvim" "$t/mod/.config/nvim"
    echo "live" > "$t/home/.config/nvim/init.lua"
    stor -t "$t/home" --adopt "$t/mod"
    stor -D -t "$t/home" "$t/mod"
    assert_absent "$t/home/.config/nvim"      # link removed
    assert_content "$t/mod/.config/nvim/init.lua" "live"  # adopted files kept
    echo "    ok: adopted content survives -D"
}

test_adopt_mismatch() {
    local t="$WORK/adopt_mismatch"
    mkdir -p "$t/home/somedir" "$t/mod"
    echo "keep" > "$t/home/somedir/keep.txt"
    echo "x" > "$t/mod/somedir"               # module file vs existing target dir
    stor -t "$t/home" --adopt "$t/mod"
    assert_file "$t/mod/somedir"
    assert_content "$t/home/somedir/keep.txt" "keep"
    echo "    ok: file/dir mismatch skipped without data loss"
}

# ---------------------------------------------------------------------------
# run everything
# ---------------------------------------------------------------------------

run_test test_version
run_test test_help
run_test test_default_target_home
run_test test_skip_invalid
run_test test_stow_link
run_test test_stow_already_stowed
run_test test_existing_target_skip
run_test test_overwrite
run_test test_copy
run_test test_simulate
run_test test_delete
run_test test_restow
run_test test_delete_keeps_target_root
run_test test_ignore_cli
run_test test_config_project
run_test test_config_global
run_test test_adopt_dir
run_test test_adopt_file
run_test test_adopt_merge
run_test test_adopt_copy
run_test test_adopt_simulate
run_test test_adopt_idempotent
run_test test_adopt_delete
run_test test_adopt_mismatch

echo
echo "=========================================="
echo "  $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
    printf '  failed: %s\n' "${FAILURES[@]}"
    exit 1
fi
