#!/bin/sh
# Installs the latest dogu release for macOS or Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/New1Direction/dogu/main/install.sh | sh
#
# Settings, all optional:
#   DOGU_VERSION=v0.2.1        a release tag. Default: the latest release
#   DOGU_INSTALL_DIR=~/bin     where to put the binary. Default: ~/.local/bin
set -eu

REPO="New1Direction/dogu"
INSTALL_DIR="${DOGU_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
fail() { printf 'dogu install: %s\n' "$*" >&2; exit 1; }

target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Darwin)
      # A shell running under Rosetta reports x86_64 on an Apple Silicon Mac.
      if [ "$arch" = x86_64 ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || echo 0)" = 1 ]; then arch=arm64; fi
      case "$arch" in
        arm64) echo aarch64-apple-darwin ;;
        x86_64) echo x86_64-apple-darwin ;;
        *) fail "no build for macOS on $arch" ;;
      esac ;;
    Linux)
      case "$arch" in
        x86_64 | amd64) echo x86_64-unknown-linux-musl ;;
        aarch64 | arm64) echo aarch64-unknown-linux-musl ;;
        *) fail "no build for Linux on $arch. Build it with: cargo install --git https://github.com/$REPO" ;;
      esac ;;
    *) fail "dogu runs on macOS and Linux, not $os" ;;
  esac
}

download() { # url file
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$2" "$1" || fail "could not download $1"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$2" "$1" || fail "could not download $1"
  else
    fail "needs curl or wget"
  fi
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    fail "needs sha256sum or shasum to check the download"
  fi
}

main() {
  t=$(target)
  version="${DOGU_VERSION:-latest}"
  if [ "$version" = latest ]; then
    base="https://github.com/$REPO/releases/latest/download"
  else
    base="https://github.com/$REPO/releases/download/$version"
  fi
  base="${DOGU_DOWNLOAD_URL:-$base}" # for testing against a local copy of a release

  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT INT TERM

  say "Downloading dogu ($version) for $t"
  download "$base/dogu-$t.tar.gz" "$work/dogu.tar.gz"
  download "$base/dogu-$t.sha256" "$work/dogu.sha256"

  want=$(cut -d ' ' -f 1 "$work/dogu.sha256")
  got=$(sha256_of "$work/dogu.tar.gz")
  [ -n "$want" ] && [ "$want" = "$got" ] || fail "checksum does not match (expected $want, got $got). Nothing was installed."

  tar -xzf "$work/dogu.tar.gz" -C "$work"
  [ -f "$work/dogu" ] || fail "the archive has no dogu binary"
  mkdir -p "$INSTALL_DIR"
  chmod 755 "$work/dogu"
  mv "$work/dogu" "$INSTALL_DIR/dogu"

  say "Installed $("$INSTALL_DIR/dogu" --version) to $INSTALL_DIR/dogu"
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) say "" && say "$INSTALL_DIR is not on your PATH. Add this to your shell profile:" && say "  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
  esac
  say ""
  say "Next: dogu sync"
}

main "$@"
