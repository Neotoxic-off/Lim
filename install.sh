#!/bin/sh
# lim installer for Linux and macOS: builds lim from source with cargo and puts it in ~/.cargo/bin.
#   curl -fsSL https://raw.githubusercontent.com/Neotoxic-off/Lim/master/install.sh | sh
set -eu

# LIM_REPO installs from a fork or a local clone instead.
repo="${LIM_REPO:-https://github.com/Neotoxic-off/Lim}"

if ! command -v cargo >/dev/null 2>&1; then
    echo "lim is built from source and needs Rust 1.88+ (cargo). Install it, then run this again:" >&2
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
fi

echo "Building lim from $repo (about a minute)..."
cargo install --git "$repo" --locked --force lim

bin="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"
echo "lim installed: $bin/lim"
case ":$PATH:" in
    *":$bin:"*) echo "Run: lim" ;;
    *) echo "Add $bin to your PATH, then run: lim" ;;
esac
