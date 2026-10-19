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

# The latest release, so lim knows its version; master when there is none (or for LIM_REPO).
tag=""
if [ -z "${LIM_REPO:-}" ]; then
    tag=$(curl -fsSL https://api.github.com/repos/Neotoxic-off/Lim/releases/latest 2>/dev/null |
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1) || tag=""
fi

echo "Building lim ${tag:-master} from $repo (about a minute)..."
if [ -n "$tag" ]; then
    cargo install --git "$repo" --tag "$tag" --locked --force lim
else
    cargo install --git "$repo" --locked --force lim
fi

bin="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"
echo "lim installed: $bin/lim"
case ":$PATH:" in
    *":$bin:"*) echo "Run: lim" ;;
    *) echo "Add $bin to your PATH, then run: lim" ;;
esac
