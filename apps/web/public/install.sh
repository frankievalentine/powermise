#!/bin/sh
# Installs powermise, the unofficial desktop companion for mise, on macOS or
# Linux from its GitHub releases:
#
#   curl -fsSL https://powermise.pages.dev/install.sh | sh
#
# macOS: powermise.app goes in /Applications (or ~/Applications).
# Linux: the .deb (apt) or .rpm (dnf, zypper, yum) through your package
# manager, which asks for your password; otherwise the AppImage in
# ~/.local/bin with a launcher entry.
#
# Settings, all optional:
#   POWERMISE_VERSION=0.2.0          a specific release (default: the latest)
#   POWERMISE_LINUX_PACKAGE=appimage deb, rpm, or appimage
#   POWERMISE_INSTALL_DIR=<dir>      where powermise.app or the AppImage goes
#   POWERMISE_DOWNLOAD_URL=<url>     a mirror of the release files
set -eu

REPO="frankievalentine/powermise"
version="${POWERMISE_VERSION:-latest}"
if [ -n "${POWERMISE_DOWNLOAD_URL:-}" ]; then
  base="${POWERMISE_DOWNLOAD_URL%/}"
elif [ "$version" = latest ]; then
  base="https://github.com/$REPO/releases/latest/download"
else
  base="https://github.com/$REPO/releases/download/v${version#v}"
fi

say() { printf '%s\n' "$*"; }
fail() {
  printf 'powermise: %s\n' "$*" >&2
  exit 1
}
has() { command -v "$1" >/dev/null 2>&1; }

has curl || fail "this installer needs curl."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

# Downloads a release file and checks it against the release's SHA256SUMS.
download() {
  say "Downloading $1…"
  curl -fL --progress-bar -o "$tmp/$1" "$base/$1" || fail "couldn't download $base/$1"
  curl -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" || fail "couldn't download $base/SHA256SUMS"
  expected=$(awk -v name="$1" '$2 == name { print $1 }' "$tmp/SHA256SUMS")
  [ -n "$expected" ] || fail "the release has no checksum for $1"
  if has sha256sum; then
    actual=$(sha256sum "$tmp/$1" | awk '{ print $1 }')
  else
    actual=$(shasum -a 256 "$tmp/$1" | awk '{ print $1 }')
  fi
  [ "$expected" = "$actual" ] || fail "$1 doesn't match its checksum; nothing was installed"
}

# Runs a command as root, through sudo unless we already are.
as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  else
    has sudo || fail "installing needs root; run this as root or install sudo"
    sudo "$@"
  fi
}

install_macos() {
  file=powermise-macos-universal.dmg
  download "$file"
  mount="$tmp/mount"
  mkdir "$mount"
  hdiutil attach -nobrowse -readonly -quiet -mountpoint "$mount" "$tmp/$file"
  app=$(find "$mount" -maxdepth 1 -name '*.app' | head -n 1)
  [ -n "$app" ] || {
    hdiutil detach -quiet "$mount"
    fail "the disk image has no app in it"
  }
  dest="${POWERMISE_INSTALL_DIR:-/Applications}"
  if [ -z "${POWERMISE_INSTALL_DIR:-}" ] && [ ! -w "$dest" ]; then
    dest="$HOME/Applications"
  fi
  mkdir -p "$dest"
  rm -rf "$dest/powermise.app"
  cp -R "$app" "$dest/powermise.app"
  hdiutil detach -quiet "$mount"
  # curl doesn't mark downloads as quarantined, but clear it to be sure:
  # these builds aren't notarized yet.
  xattr -dr com.apple.quarantine "$dest/powermise.app" 2>/dev/null || true
  say "Installed powermise in $dest."
  say "Open it from Launchpad or Spotlight, or run: open \"$dest/powermise.app\""
}

install_appimage() {
  file="powermise-linux-$arch.AppImage"
  download "$file"
  dest="${POWERMISE_INSTALL_DIR:-$HOME/.local/bin}"
  mkdir -p "$dest"
  install -m 755 "$tmp/$file" "$dest/powermise"
  apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  mkdir -p "$apps"
  cat >"$apps/powermise.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=powermise
Comment=An unofficial desktop companion for mise
Exec=$dest/powermise
Categories=Development;
Terminal=false
EOF
  say "Installed powermise in $dest/powermise, with a launcher entry."
  if has ldconfig && ! ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; then
    say "AppImages need FUSE 2 (libfuse2, or libfuse2t64 on Ubuntu 24.04). Install it if powermise doesn't open."
  fi
  case ":$PATH:" in
    *":$dest:"*) ;;
    *) say "Add $dest to your PATH to run powermise from a terminal." ;;
  esac
}

install_linux() {
  case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 deb_arch=amd64 ;;
    aarch64 | arm64) arch=aarch64 deb_arch=arm64 ;;
    *) fail "there's no Linux build for $(uname -m) yet." ;;
  esac
  kind="${POWERMISE_LINUX_PACKAGE:-}"
  if [ -z "$kind" ]; then
    if has apt-get && has dpkg; then
      kind=deb
    elif has dnf || has zypper || has yum; then
      kind=rpm
    else
      kind=appimage
    fi
  fi
  case "$kind" in
    deb)
      file="powermise-linux-$deb_arch.deb"
      download "$file"
      chmod 644 "$tmp/$file"
      chmod 755 "$tmp"
      as_root apt-get install -y "$tmp/$file"
      say "Installed powermise. Open it from your app launcher, or run: powermise"
      ;;
    rpm)
      file="powermise-linux-$arch.rpm"
      download "$file"
      if has dnf; then
        as_root dnf install -y "$tmp/$file"
      elif has zypper; then
        as_root zypper --non-interactive install --allow-unsigned-rpm "$tmp/$file"
      else
        as_root yum install -y "$tmp/$file"
      fi
      say "Installed powermise. Open it from your app launcher, or run: powermise"
      ;;
    appimage) install_appimage ;;
    *) fail "POWERMISE_LINUX_PACKAGE must be deb, rpm, or appimage." ;;
  esac
}

case "$(uname -s)" in
  Darwin) install_macos ;;
  Linux) install_linux ;;
  MINGW* | MSYS* | CYGWIN*)
    fail "on Windows, run this in PowerShell instead: irm https://powermise.pages.dev/install.ps1 | iex"
    ;;
  *) fail "there's no powermise build for $(uname -s) yet." ;;
esac

if ! has mise; then
  say "powermise runs mise for everything it does; it offers to install mise the first time it opens."
fi
