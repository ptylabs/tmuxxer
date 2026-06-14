# tmuxxer

Tmux power tools. `tmuxxer` is a sessionizer that combines configured project folders, live tmux sessions, and running Docker containers in one `fzf` picker.

Inspired by [tmux-sessionizer](https://github.com/joshmedeski/tmux-sessionizer), with existing sessions and optional Docker containers listed alongside candidate directories.

## Requirements

- [tmux](https://github.com/tmux/tmux) on `PATH`
- [fzf](https://github.com/junegunn/fzf) on `PATH`
- Rust toolchain to build from source

Docker is optional. When Docker is available and enabled in config, running containers can appear in the picker.

## Install

```bash
cargo install --path .
```

This installs a durable `tmuxxer` executable that can be written safely into shell
and tmux key bindings.

For development or a quick trial, run commands through Cargo without installing:

```bash
cargo run -- sessionize
```

Permanent key binding setup through `tmuxxer init` or `tmuxxer user-config`
requires an installed executable on `PATH`; Cargo build artifacts such as
`target/debug/tmuxxer` are intentionally not written into dotfiles.

## Usage

```bash
tmuxxer              # setup on first run, then open the picker
tmuxxer sessionize   # same as default
tmuxxer s            # short alias for sessionize
tmuxxer init         # re-run setup and rewrite project roots
tmuxxer user-config  # configure Ctrl+F bindings and Docker behavior

tmuxxer config path
tmuxxer config list
tmuxxer config get sources.docker
tmuxxer config set sources.docker false
tmuxxer config toggle sources.docker
tmuxxer config validate
tmuxxer config migrate

tmuxxer --add ~/code        # toggle a search root
tmuxxer --ignore target     # toggle an ignore pattern
tmuxxer update --check
tmuxxer update --dismiss
tmuxxer --version
tmuxxer --help
```

## Picker Entries

- `[session] name` attaches or switches to an existing tmux session.
- `[docker] name - image (id)` opens a shell inside a running container when Docker entries are enabled.
- `[dir] label - /full/path` creates or switches to a directory session.

Inside tmux, selected directory sessions use `tmux switch-client`. Outside tmux, tmuxxer creates or attaches to the target session.

## Configuration

Config lives at `$XDG_CONFIG_HOME/tmuxxer/config`, or `~/.config/tmuxxer/config` when `XDG_CONFIG_HOME` is not set.

See [docs/config.md](docs/config.md) for every supported config flag, valid values, defaults, and migration notes.
