# tmuxxer

Tmux power tools. A **sessionizer** that combines configured project folders, live tmux sessions, and running Docker containers in one configurable `fzf` picker, then opens the selected target.

Inspired by [tmux-sessionizer](https://github.com/joshmedeski/tmux-sessionizer), with existing sessions listed alongside candidate directories.

## Requirements

- [tmux](https://github.com/tmux/tmux) on `PATH`
- [fzf](https://github.com/junegunn/fzf) on `PATH`
- Rust toolchain (to build)

If either tool is missing, tmuxxer exits with an error before doing anything else.

Docker is optional. When `docker` is available, the daemon is reachable, and `sources.docker = true`, running containers are included in the picker.

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
tmuxxer config list  # print configurable launch settings
tmuxxer config get sources.docker
tmuxxer config set sources.docker false
tmuxxer config set session.name_strategy basename
tmuxxer config toggle sources.docker
tmuxxer config migrate
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
3. **Writes TOML config** to `$XDG_CONFIG_HOME/tmuxxer/config` or `~/.config/tmuxxer/config`.
4. **Session picker** — opens the fzf picker for normal use.

Run `tmuxxer init` anytime to redo this and overwrite the config.
Run `tmuxxer user-config` anytime to reconfigure the tmux/bash bindings and Docker entry behavior. If a tmuxxer block is already present, it is updated in place instead of duplicated.

At the end of setup you can opt in to **Ctrl+F** bindings:

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

The sourced `bash-bind.sh` binds interactive Bash shells both inside and outside tmux:

```bash
_tmuxxer_sessionize() {
  '/path/to/tmuxxer' sessionize
}
bind -x '"\C-f": "_tmuxxer_sessionize"'
```

**tmux passthrough** (default yes when the Bash binding exists) — written to the user tmux config file tmux is loading, or `~/.tmux.conf` if none exists yet:

```tmux
# >>> tmuxxer >>>
bind-key -n C-f send-keys C-f
# <<< tmuxxer <<<
```

The tmux binding forwards Ctrl+F into the current pane. In interactive Bash shells, the Bash binding handles that key and runs tmuxxer without typing a command into the prompt. If another full-screen program is active in the pane, tmux forwards Ctrl+F to that program.

After writing the binding, setup asks whether to reload tmux immediately. If tmux is not running yet or reload fails, run `tmux source-file <that file>` after starting tmux.

Re-running `tmuxxer init` and accepting the prompt updates that block in place (no duplicates).
`tmuxxer user-config` does the same without touching the project search paths. It can also toggle whether Docker containers appear in the picker and whether selected Docker entries open in a new tmux session or directly in the current pane.

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

### Config commands

Use `tmuxxer config` to inspect and change launch settings without hand-editing the file:

```bash
tmuxxer config path
tmuxxer config list
tmuxxer config get sources.docker
tmuxxer config set sources.docker false
tmuxxer config set session.name_strategy basename
tmuxxer config toggle sources.docker
tmuxxer config migrate
```

Supported boolean keys:

- `sources.sessions` — show existing tmux sessions in the picker
- `sources.directories` — show scanned project directories in the picker
- `sources.docker` — show running Docker containers in the picker
- `docker.new_session` — open selected Docker entries in their own tmux sessions

Supported string keys:

- `session.name_strategy` — `path` for stable path-derived directory session names, or `basename` for legacy basename-only names

`tmuxxer config migrate` loads either the legacy config or TOML v2 config and rewrites the file as TOML v2 without changing behavior.

### Session picker

The picker uses `fzf --height=80% --layout=reverse --border` both inside and outside tmux, so it appears as the same compact panel instead of switching between tmux popup and fullscreen modes.

- `[session] name` — attach or switch to an existing tmux session
- `[docker] name — image (id)` — open a shell inside that container when `sources.docker = true`
- `[dir] label — /full/path` — create a directory session using `session.name_strategy` and attach

**Session naming**

Directory session names default to `session.name_strategy = "path"`. This uses the directory basename plus a stable path hash, so folders with the same basename map to different tmux sessions deterministically. Set `session.name_strategy = "basename"` to use the legacy basename-only behavior, where dots are replaced by underscores.
Docker session names are prefixed with `docker_` and derived from the container name, with non-session-friendly characters replaced by `_`. This only applies when `docker.new_session = true`.

**Attach behavior**

- Outside tmux, no server: `tmux new-session -s NAME -c DIR` (creates and attaches)
- Outside tmux, server running: create detached if missing, then `tmux attach`
- Inside tmux: create detached if missing, then `tmux switch-client`
- Docker containers: when enabled, create or reuse a tmux session running `docker exec -it CONTAINER SHELL`, preferring the container's `SHELL` env when it points to a supported shell, then common shells such as `bash`, `sh`, and `ash`
- With `docker.new_session = false`, selected Docker containers open directly in the current pane instead of a new tmux session
- With `sources.docker = false`, Docker containers are hidden from search results entirely

## Config

`$XDG_CONFIG_HOME/tmuxxer/config` if `XDG_CONFIG_HOME` is set, otherwise `~/.config/tmuxxer/config`.

Created by the first-run wizard (or `tmuxxer init`). New configs are written as TOML v2:

```toml
# Generated by tmuxxer setup

version = 2

[sources]
sessions = true
directories = true
docker = true

[session]
name_strategy = "path"

[docker]
new_session = true

[search]
ignore = ["target", ".git"]

[[search.roots]]
path = "~/code"
depth = 1

[[search.roots]]
path = "~/work"
depth = 3
```

- `sources.sessions` — include existing tmux sessions in the picker
- `sources.directories` — include scanned project directories in the picker
- `sources.docker` — include running Docker containers in the picker
- `session.name_strategy` — `path` by default for collision-safe directory session names; set to `basename` for legacy names
- `docker.new_session` — `true` by default; set to `false` to open selected Docker entries directly in the current pane
- `search.roots` — search roots with per-root `depth` (1 = immediate children only)
- `search.ignore` — path or component patterns to skip while scanning, with simple gitignore-like `*` matching

Edit the file by hand anytime, or use `tmuxxer config get/set/toggle` for configurable launch settings. Use `tmuxxer init` to reconfigure paths interactively. Existing `sources`, `session`, `docker`, and `search.ignore` settings are preserved when setup rewrites roots.

Legacy key/value configs are loaded automatically:

```ini
docker_new_session = true
path = ~/code
depth = 1
ignore = target
```

Any command that saves the config, including `tmuxxer config migrate`, rewrites it as TOML v2 without changing the mapped behavior.

## Project layout

```
src/
  main.rs         CLI entry
  deps.rs         tmux/fzf presence check
  docker.rs       Docker container listing / shell command
  config.rs       Config load / save
  config_cmd.rs   Config CLI commands
  setup.rs        First-run CLI prompts
  bashrc.rs       Optional Ctrl+F bashrc block
  fzf.rs          fzf integration
  tmux.rs         tmux command wrappers
  sessionizer.rs  Collect, pick, create/attach
```

no dependencies yet.
