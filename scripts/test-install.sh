#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/tmuxxer-install-test.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT HUP INT TERM
mkdir -p "$test_dir/bin" "$test_dir/assets" "$test_dir/package"

cat > "$test_dir/bin/uname" <<'EOF'
#!/bin/sh
case "$1" in
    -s) printf '%s\n' Darwin ;;
    -m) printf '%s\n' "$TEST_ARCH" ;;
    *) exit 1 ;;
esac
EOF

cat > "$test_dir/bin/curl" <<'EOF'
#!/bin/sh
destination=
url=
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) shift; destination=$1 ;;
        https://*) url=$1 ;;
    esac
    shift
done
case "$url" in
    */releases/latest) printf '%s\n' '{"tag_name":"v9.9.9"}' ;;
    */releases/download/v9.9.9/*)
        cp "$TEST_ASSET_DIR/${url##*/}" "$destination" ;;
    *) exit 1 ;;
esac
EOF

cat > "$test_dir/package/tmuxxer" <<'EOF'
#!/bin/sh
printf '%s\n' 'tmuxxer 9.9.9'
EOF
chmod 755 "$test_dir/bin/uname" "$test_dir/bin/curl" "$test_dir/package/tmuxxer"

for mapping in 'arm64 aarch64-apple-darwin' 'x86_64 x86_64-apple-darwin'; do
    set -- $mapping
    arch=$1
    target=$2
    asset="tmuxxer-9.9.9-$target.tar.gz"
    tar -C "$test_dir/package" -czf "$test_dir/assets/$asset" tmuxxer
    if command -v sha256sum >/dev/null 2>&1; then
        hash=$(sha256sum "$test_dir/assets/$asset")
    else
        hash=$(shasum -a 256 "$test_dir/assets/$asset")
    fi
    printf '%s  %s\n' "${hash%% *}" "$asset" > "$test_dir/assets/tmuxxer-9.9.9-sha256sums.txt"

    for installer in scripts/install.sh docs/install.sh; do
        install_dir="$test_dir/install-$arch-${installer%%/*}"
        TEST_ARCH=$arch TEST_ASSET_DIR="$test_dir/assets" \
            TMUXXER_INSTALL_DIR="$install_dir" \
            PATH="$test_dir/bin:$PATH" \
            sh "$repo_dir/$installer" > "$test_dir/install.log"
        test "$("$install_dir/tmuxxer" --version)" = 'tmuxxer 9.9.9'
        grep -q "for $target" "$test_dir/install.log"
    done
done

printf '%s\n' 'macOS installer tests passed (Apple Silicon and Intel)'
