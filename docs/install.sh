#!/usr/bin/env sh
set -eu

REPO="ptylabs/tmuxxer"
BINARY="tmuxxer"
API_URL="https://api.github.com/repos/$REPO/releases/latest"
GITHUB_URL="https://github.com/$REPO"
USER_AGENT="tmuxxer-installer"

debug() {
    if [ "${TMUXXER_DEBUG:-0}" = "1" ]; then
        printf 'debug: %s\n' "$*" >&2
    fi
}

info() {
    printf '%s\n' "$*"
}

warn() {
    printf 'Warning: %s\n' "$*" >&2
}

fail() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

have_cmd() {
    command -v "$1" >/dev/null 2>&1
}

need_cmd() {
    have_cmd "$1" || fail "$1 is required"
}

https_only() {
    case "$1" in
        https://*) ;;
        *) fail "refusing non-HTTPS URL: $1" ;;
    esac
}

download_to_stdout() {
    url=$1
    https_only "$url"

    if have_cmd curl; then
        curl -fsSL --proto '=https' --tlsv1.2 -H "User-Agent: $USER_AGENT" "$url"
        return
    fi

    if have_cmd wget; then
        wget -q --user-agent="$USER_AGENT" -O - "$url"
        return
    fi

    fail "curl or wget is required to download tmuxxer"
}

download_to_file() {
    url=$1
    dest=$2
    https_only "$url"
    debug "download $url -> $dest"

    if have_cmd curl; then
        curl -fsSL --proto '=https' --tlsv1.2 -H "User-Agent: $USER_AGENT" -o "$dest" "$url"
        return
    fi

    if have_cmd wget; then
        wget -q --user-agent="$USER_AGENT" -O "$dest" "$url"
        return
    fi

    fail "curl or wget is required to download tmuxxer"
}

normalize_version() {
    version=$1
    case "$version" in
        v* | V*) printf '%s\n' "${version#?}" ;;
        *) printf '%s\n' "$version" ;;
    esac
}

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os/$arch" in
        Linux/x86_64 | Linux/amd64)
            printf '%s\n' "x86_64-unknown-linux-gnu"
            ;;
        Linux/aarch64 | Linux/arm64)
            printf '%s\n' "aarch64-unknown-linux-gnu"
            ;;
        Linux/*)
            fail "unsupported Linux architecture '$arch' (supported: x86_64, aarch64)"
            ;;
        Darwin/*)
            fail "macOS is not supported by this installer yet; build from source with Cargo"
            ;;
        *)
            fail "unsupported operating system '$os'"
            ;;
    esac
}

latest_tag() {
    need_cmd sed
    json=$(download_to_stdout "$API_URL")
    tag=$(printf '%s\n' "$json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | sed '1q')
    [ -n "$tag" ] || fail "could not read latest release tag from GitHub"
    printf '%s\n' "$tag"
}

sha256_file() {
    file=$1

    if have_cmd sha256sum; then
        output=$(sha256sum "$file") || fail "sha256sum failed for $file"
        printf '%s\n' "${output%% *}"
        return
    fi

    if have_cmd shasum; then
        output=$(shasum -a 256 "$file") || fail "shasum failed for $file"
        printf '%s\n' "${output%% *}"
        return
    fi

    if have_cmd openssl; then
        output=$(openssl dgst -sha256 -r "$file") || fail "openssl failed for $file"
        printf '%s\n' "${output%% *}"
        return
    fi

    fail "sha256sum, shasum, or openssl is required to verify downloads"
}

verify_checksum() {
    checksums_file=$1
    asset_name=$2
    archive=$3
    expected=""

    while read -r hash name _rest; do
        name=${name#\*}
        name=${name#./}
        if [ "$name" = "$asset_name" ]; then
            expected=$hash
            break
        fi
    done < "$checksums_file"

    [ -n "$expected" ] || fail "$checksums_file has no checksum for $asset_name"
    [ "${#expected}" -eq 64 ] || fail "invalid SHA256 length for $asset_name"
    case "$expected" in
        *[!0123456789abcdefABCDEF]*) fail "invalid SHA256 digest for $asset_name" ;;
    esac

    actual=$(sha256_file "$archive")
    expected_lc=$(printf '%s\n' "$expected" | tr '[:upper:]' '[:lower:]')
    actual_lc=$(printf '%s\n' "$actual" | tr '[:upper:]' '[:lower:]')

    [ "$actual_lc" = "$expected_lc" ] || fail "checksum mismatch for $asset_name"
}

validate_archive() {
    archive=$1
    entries=$2

    tar -tzf "$archive" > "$entries" || fail "failed to list $archive"
    while IFS= read -r entry; do
        case "$entry" in
            "" | /* | ../* | */../* | */.. | ..)
                fail "release archive contains unsafe path: $entry"
                ;;
        esac
    done < "$entries"
}

path_contains() {
    dir=$1
    case ":$PATH:" in
        *:"$dir":*) return 0 ;;
        *) return 1 ;;
    esac
}

print_path_help() {
    dir=$1
    shell_name=${SHELL##*/}

    printf '\n%s is not on PATH yet. Add it, then restart your shell.\n' "$dir"
    case "$shell_name" in
        fish | fsh)
            printf '  fish_add_path "%s"\n' "$dir"
            ;;
        zsh)
            printf '  printf '\''%%s\\n'\'' '\''export PATH="%s:$PATH"'\'' >> ~/.zshrc\n' "$dir"
            ;;
        nu | nushell)
            printf '  mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/nushell"\n'
            printf '  printf '\''%%s\\n'\'' '\''$env.PATH = ($env.PATH | prepend "%s")'\'' >> "${XDG_CONFIG_HOME:-$HOME/.config}/nushell/config.nu"\n' "$dir"
            ;;
        *)
            printf '  printf '\''%%s\\n'\'' '\''export PATH="%s:$PATH"'\'' >> ~/.bashrc\n' "$dir"
            ;;
    esac
}

need_cmd uname
need_cmd mktemp
need_cmd mkdir
need_cmd chmod
need_cmd cp
need_cmd tar
need_cmd tr

if [ -n "${TMUXXER_INSTALL_DIR:-}" ]; then
    install_dir=$TMUXXER_INSTALL_DIR
else
    [ -n "${HOME:-}" ] || fail "HOME is not set; set TMUXXER_INSTALL_DIR to an absolute directory"
    install_dir=$HOME/.local/bin
fi

case "$install_dir" in
    /*) ;;
    *) fail "TMUXXER_INSTALL_DIR must be an absolute path: $install_dir" ;;
esac

if [ -n "${TMUXXER_VERSION:-}" ]; then
    version=$(normalize_version "$TMUXXER_VERSION")
    tag="v$version"
else
    tag=$(latest_tag)
    version=$(normalize_version "$tag")
fi

[ -n "$version" ] || fail "release version is empty"

target=$(detect_target)
asset="$BINARY-$version-$target.tar.gz"
checksums="$BINARY-$version-sha256sums.txt"
release_url="$GITHUB_URL/releases/download/$tag"
asset_url="$release_url/$asset"
checksums_url="$release_url/$checksums"

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/tmuxxer-install.XXXXXX")
cleanup() {
    if [ -n "${tmp_dir:-}" ] && [ -d "$tmp_dir" ]; then
        rm -rf "$tmp_dir"
    fi
}
trap cleanup EXIT HUP INT TERM

archive=$tmp_dir/$asset
checksums_file=$tmp_dir/$checksums
entries_file=$tmp_dir/archive-entries.txt
extract_dir=$tmp_dir/extract
binary_path=$extract_dir/$BINARY
install_path=$install_dir/$BINARY

info "Installing tmuxxer $tag for $target..."
download_to_file "$asset_url" "$archive"

if [ "${TMUXXER_SKIP_CHECKSUM:-0}" = "1" ]; then
    warn "Skipping checksum verification because TMUXXER_SKIP_CHECKSUM=1"
else
    download_to_file "$checksums_url" "$checksums_file" || fail "release $tag does not publish $checksums; refusing to install without checksum"
    verify_checksum "$checksums_file" "$asset" "$archive"
fi

mkdir -p "$extract_dir"
validate_archive "$archive" "$entries_file"
tar -xzf "$archive" -C "$extract_dir" || fail "failed to unpack $asset"

if [ ! -f "$binary_path" ]; then
    for candidate in "$extract_dir"/*/"$BINARY"; do
        if [ -f "$candidate" ]; then
            binary_path=$candidate
            break
        fi
    done
fi

[ -f "$binary_path" ] || fail "$asset did not contain a $BINARY binary"
mkdir -p "$install_dir"
cp "$binary_path" "$install_path"
chmod 0755 "$install_path"

info "Installed to $install_path"
if installed_version=$("$install_path" --version); then
    info "$installed_version"
else
    warn "Installed binary did not print a version"
fi

if path_contains "$install_dir"; then
    if command -v "$BINARY" >/dev/null 2>&1; then
        info "Run 'tmuxxer init' to get started."
    else
        warn "$install_dir is on PATH, but command -v tmuxxer did not resolve yet; restart your shell"
    fi
else
    print_path_help "$install_dir"
    info "After updating PATH, run 'tmuxxer init' to get started."
fi
