# tmuxxer

Switch between project folders, tmux sessions, and running Docker containers with `Ctrl+f`.

<p align="center">
  <a href="https://crates.io/crates/tmuxxer">
    <img src="https://img.shields.io/crates/v/tmuxxer?style=flat&logo=rust&color=0891b2" alt="Crates.io Version" />
  </a>
  <img src="https://img.shields.io/badge/Made%20with-Rust-black?style=flat&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Runs%20with-tmux-1BB91F?style=flat&logo=tmux&logoColor=white" alt="tmux" />
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat" alt="License" />
</p>

<p align="center">
  <sub>Made with ❤️ by <a href="https://github.com/ptylabs">ptylabs</a></sub>
</p>

`tmuxxer` combines configured project folders, live tmux sessions, and running containers in one `fzf` picker.

Inspired by [tmux-sessionizer](https://github.com/theprimeagen/tmux-sessionizer).

## Requirements

- [tmux](https://github.com/tmux/tmux) installed
- [fzf](https://github.com/junegunn/fzf) installed

On macOS, install both with Homebrew:

```bash
brew install tmux fzf
```

> **Note:** Docker is optional. When Docker is available and enabled in config, running containers can appear in the picker. Rust is only required when building from source.

## Install

Install and compile from crates.io:

```bash
cargo install tmuxxer
```

Or install a prebuilt binary on Linux, Apple Silicon macOS, or Intel macOS:

```bash
curl -fsSL https://ptylabs.github.io/tmuxxer/install.sh | sh
```

[View installation script here](https://raw.githubusercontent.com/ptylabs/tmuxxer/main/docs/install.sh)

The prebuilt macOS binaries become available with the next release containing these changes. Until then, use `cargo install tmuxxer` on macOS.

Permanent key binding setup through `tmuxxer init` or `tmuxxer user-config` requires an installed executable on `PATH`; Cargo build artifacts such as `target/debug/tmuxxer` are intentionally not written into dotfiles.

## Usage

Run `tmuxxer init` to set up bindings for your shell and tmux. Source your shell config or restart your terminal, then press `Ctrl+f` to choose a project.

Basic usage commands:

```bash
tmuxxer              # setup on first run, then open the picker
tmuxxer sessionize   # same as default
tmuxxer init         # re-run setup and rewrite project roots
tmuxxer user-config  # configure Ctrl+F bindings and Docker behavior
tmuxxer --add        # Add a project to the fuzzy finder
tmuxxer --ignore     # Ignore a project from the fuzzy finder
```

See [performance notes](docs/performance.md) for build settings and a reproducible benchmark.

## What you'll see when fuzzy finding

- `[session] name` attaches or switches to an existing tmux session.
- `[docker] name - image (id)` opens a shell inside a running container when Docker entries are enabled.
- `[dir] label - /full/path` creates or switches to a directory session.

Inside tmux, selected directory sessions use `tmux switch-client`. Outside tmux, tmuxxer creates or attaches to the target session.

## Configuration

Config lives at `$XDG_CONFIG_HOME/tmuxxer/config`, or `~/.config/tmuxxer/config` when `XDG_CONFIG_HOME` is not set.

See [docs/config.md](docs/config.md) for every supported config flag, valid values, defaults, and migration notes.

## Release Assets

The install script and `tmuxxer update` both consume GitHub Release assets named:

```text
tmuxxer-{version}-{target}.tar.gz
tmuxxer-{version}-sha256sums.txt
```

`{version}` is the Cargo version without a leading `v`; the Git tag keeps the leading `v` (`v1.0.1` tag, `tmuxxer-1.0.1-x86_64-unknown-linux-gnu.tar.gz` asset). The release workflow currently builds:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
