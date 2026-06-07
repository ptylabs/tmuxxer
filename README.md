# tmuxxer

Tmux power tools. A **sessionizer** that combines configured project folders and live tmux sessions in one `fzf` picker, then creates or attaches a session.

Inspired by [tmux-sessionizer](https://github.com/joshmedeski/tmux-sessionizer), with existing sessions listed alongside candidate directories.

## Requirements

- [tmux](https://github.com/tmux/tmux) on `PATH`
- [fzf](https://github.com/junegunn/fzf) on `PATH`
- Rust toolchain (to build)

If either tool is missing, tmuxxer exits with an error before doing anything else.

## Install

```bash
cargo install --path .
```

Or run without installing:

```bash
cargo run
```

## Usage

```bash
tmuxxer              # setup on first run, then fzf picker
tmuxxer sessionize   # same
tmuxxer init         # re-run setup and overwrite config
tmuxxer user-config  # re-run tmux/bash integration setup
tmuxxer --ignore     # append ignored paths or patterns
tmuxxer --version    # print version (-v also works)
tmuxxer --help
```

### First run

On the first invocation (no config file yet), tmuxxer runs a short CLI setup:

1. **Dependency check** — fails fast if `tmux` or `fzf` are not installed.
2. **Guided setup wizard**:
   - Optional `[y/N]` prompt to include `~` as a search root.
   - For each common folder found on disk (`~/code`, `~/work`, `~/projects`, `~/personal`, `~/dev`), a `[Y/n]` prompt to include it.
   - Free-form loop to type any extra paths (`~` supported); blank line finishes.
   - Per-folder scan depth prompts with a default of `1`.
3. **Writes config** to `$XDG_CONFIG_HOME/tmuxxer/config` or `~/.config/tmuxxer/config`.
4. **Session picker** — opens the fzf picker for normal use.

Run `tmuxxer init` anytime to redo this and overwrite the config.
Run `tmuxxer user-config` anytime to reconfigure the tmux/bash bindings. If a tmuxxer block is already present, it is updated in place instead of duplicated.

At the end of setup you can opt in to **Ctrl+F** bindings:

**tmux** (default yes) — written to the user tmux config file tmux is loading, or `~/.tmux.conf` if none exists yet:

```tmux
# >>> tmuxxer >>>
bind-key -n C-f send-keys C-u \; send-keys -l "'/path/to/tmuxxer' sessionize" \; send-keys Enter
# <<< tmuxxer <<<
```

The tmux binding runs tmuxxer in the current pane so the picker uses the same compact `fzf` panel as it does from Bash. It is intended for shell prompts; if another full-screen program is active in the pane, tmux sends the keys to that program.

After writing the binding, setup asks whether to reload tmux immediately. If tmux is not running yet or reload fails, run `tmux source-file <that file>` after starting tmux.

**bash** (default yes) — `~/.bashrc` sources the runtime config path:

```bash
# >>> tmuxxer >>>
if [[ $- == *i* ]]; then
  _tmuxxer_bind="${XDG_CONFIG_HOME:-$HOME/.config}/tmuxxer/bash-bind.sh"
  [[ -r "$_tmuxxer_bind" ]] && source "$_tmuxxer_bind"
  unset _tmuxxer_bind
fi
# <<< tmuxxer <<<
```

The sourced `bash-bind.sh` only binds interactive Bash shells outside tmux:

```bash
bind -x '"\C-f": "tmuxxer sessionize"'
```

Re-running `tmuxxer init` and accepting the prompt updates that block in place (no duplicates).
`tmuxxer user-config` does the same without touching the project search paths.

After writing the Bash binding, new interactive Bash shells pick it up automatically. The current shell cannot be modified by `tmuxxer init`; run `source ~/.bashrc` there if you want Ctrl+F without opening a new shell.

### Ignoring paths

Run `tmuxxer --ignore` after setup to append ignored path or component patterns to the config. Pass paths on the command line to toggle them without the interactive prompt, for example `tmuxxer --ignore ./folder/` adds the ignore and running it again removes it.

Run `tmuxxer --add ~/code` to toggle a search root without re-running init.

These commands only need the config file; they do not require `tmux` or `fzf`.

Examples:

```ini
ignore = target
ignore = .git
ignore = .*
ignore = node_modules/*
ignore = ~/work/tmp
```

Matching rules:

- no slash: matches any path component, with `*` supported, e.g. `.*`, `target`
- slash without leading `/`: matches relative paths anywhere below each search root, e.g. `node_modules/*`
- leading `~/` or `/`: matches from that absolute path prefix

### Session picker

The picker uses `fzf --height=80% --layout=reverse --border` both inside and outside tmux, so it appears as the same compact panel instead of switching between tmux popup and fullscreen modes.

- `[session] name` — attach or switch to an existing tmux session
- `[dir] label — /full/path` — create a session named from the folder basename (`.` → `_`) and attach

**Session naming**

The session name is the directory basename with dots replaced by underscores (tmux treats `.` specially in targets).

**Attach behavior**

- Outside tmux, no server: `tmux new-session -s NAME -c DIR` (creates and attaches)
- Outside tmux, server running: create detached if missing, then `tmux attach`
- Inside tmux: create detached if missing, then `tmux switch-client`

## Config

`$XDG_CONFIG_HOME/tmuxxer/config` if `XDG_CONFIG_HOME` is set, otherwise `~/.config/tmuxxer/config`.

Created by the first-run wizard (or `tmuxxer init`). Simple `key = value` format:

```ini
# Generated by tmuxxer setup

path = ~/code
depth = 1

path = ~/work
depth = 3

ignore = target
```

- `path` — search root (repeatable)
- `depth` — how deep to scan under the preceding path (1 = immediate children only)
- `ignore` — path or component pattern to skip while scanning, with simple gitignore-like `*` matching

Edit the file by hand anytime; use `tmuxxer init` to reconfigure paths interactively. Existing `ignore` entries are preserved when setup rewrites roots.

## Project layout

```
src/
  main.rs         CLI entry
  deps.rs         tmux/fzf presence check
  config.rs       Config load / save
  setup.rs        First-run CLI prompts
  bashrc.rs       Optional Ctrl+F bashrc block
  fzf.rs          fzf integration
  tmux.rs         tmux command wrappers
  sessionizer.rs  Collect, pick, create/attach
```

no dependencies yet.
