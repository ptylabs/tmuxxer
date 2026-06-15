# tmuxxer

Tmux power tools. `tmuxxer` is a sessionizer that combines configured project folders, live tmux sessions, and running Docker containers in one `fzf` picker.

Inspired by [tmux-sessionizer](https://github.com/joshmedeski/tmux-sessionizer), with existing sessions and optional Docker containers listed alongside candidate directories.

## Requirements

- [tmux](https://github.com/tmux/tmux) on `PATH`
- [fzf](https://github.com/junegunn/fzf) on `PATH`

Docker is optional. When Docker is available and enabled in config, running containers can appear in the picker.
Rust is only required when building from source.

## Install

Recommended install:

```bash
curl -fsSL https://ptylabs.github.io/tmuxxer/install.sh | sh
```

The installer downloads the latest stable GitHub Release for your Linux architecture,
verifies its SHA256 checksum, and installs a durable binary to
`~/.local/bin/tmuxxer`. If that directory is not on `PATH`, it prints the exact
shell command to add it.

Fallback URL if GitHub Pages is not enabled yet:

```bash
curl -fsSL https://raw.githubusercontent.com/ptylabs/tmuxxer/main/docs/install.sh | sh
```

Installer environment overrides:

```bash
TMUXXER_INSTALL_DIR=/usr/local/bin sh docs/install.sh
TMUXXER_VERSION=1.0.0 sh docs/install.sh
TMUXXER_DEBUG=1 sh docs/install.sh
```

Alternative source install from crates.io, when published:

```bash
cargo install tmuxxer
```

Development install from a local checkout:

```bash
cargo install --path .
```

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
tmuxxer update                # update a script or Cargo install
tmuxxer update --disable-auto # disable automatic update checks
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

## Release assets

The install script and `tmuxxer update` both consume GitHub Release assets named:

```text
tmuxxer-{version}-{target}.tar.gz
tmuxxer-{version}-sha256sums.txt
```

`{version}` is the Cargo version without a leading `v`; the Git tag keeps the
leading `v` (`v1.0.0` tag, `tmuxxer-1.0.0-x86_64-unknown-linux-gnu.tar.gz`
asset). The release workflow currently builds:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`

To publish the installer URL, enable GitHub Pages from repository
Settings → Pages → Deploy from a branch → `main` / `docs`. First release:

```bash
cargo test --locked
git tag v1.0.0
git push origin v1.0.0
```

After the workflow finishes, verify the release contains both tarballs and
`tmuxxer-1.0.0-sha256sums.txt`, then verify:

```bash
curl -fsSL https://ptylabs.github.io/tmuxxer/install.sh | sh
tmuxxer --version
```
