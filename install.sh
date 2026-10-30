#!/usr/bin/env bash
# Installs the prerequisites (libfuse2, zenity), downloads the latest Teams for Linux
# AppImage, verifies it, installs it to ~/.local/opt/teams-for-linux and starts it.
#
#   curl -fsSL https://raw.githubusercontent.com/community-artifacts/linux-teams/main/install.sh | bash
#
# Options (pass after `bash -s --` when piping): --no-run  install without starting the app
# Environment: INSTALL_DIR (default ~/.local/opt/teams-for-linux), TEAMS_BASE_URL (download location)
set -euo pipefail

REPO=community-artifacts/linux-teams
NAME=Teams_for_Linux-x86_64.AppImage
BASE_URL=${TEAMS_BASE_URL:-https://github.com/$REPO/releases/latest/download}
INSTALL_DIR=${INSTALL_DIR:-$HOME/.local/opt/teams-for-linux}
RUN=1
[ "${1:-}" = "--no-run" ] && RUN=0

say() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[ "$(uname -m)" = x86_64 ] || die "only x86_64 is supported"
[ "$(id -u)" -ne 0 ] || die "run this as your normal user, not root (it will use sudo when needed)"
command -v curl >/dev/null || die "curl is required"

has_fuse2() { { /sbin/ldconfig -p || ldconfig -p; } 2>/dev/null | grep -q 'libfuse\.so\.2'; }

install_deps() {
  local pkgs=()
  has_fuse2 || pkgs+=(fuse2)
  command -v zenity >/dev/null || pkgs+=(zenity)
  [ ${#pkgs[@]} -eq 0 ] && return
  say "installing prerequisites: ${pkgs[*]}"
  command -v sudo >/dev/null || die "sudo is needed to install: ${pkgs[*]}"
  if command -v apt-get >/dev/null; then
    local apt=()
    for p in "${pkgs[@]}"; do
      case $p in
        fuse2) apt+=("$(apt-cache show libfuse2t64 >/dev/null 2>&1 && echo libfuse2t64 || echo libfuse2)") ;;
        *) apt+=("$p") ;;
      esac
    done
    sudo apt-get update && sudo apt-get install -y "${apt[@]}"
  elif command -v dnf >/dev/null; then
    sudo dnf install -y "${pkgs[@]/fuse2/fuse-libs}"
  elif command -v pacman >/dev/null; then
    sudo pacman -S --needed --noconfirm "${pkgs[@]}"
  elif command -v zypper >/dev/null; then
    sudo zypper install -y "${pkgs[@]/fuse2/libfuse2}"
  else
    die "unknown package manager; install manually: ${pkgs[*]} (libfuse2 and zenity)"
  fi
}

install_deps

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
say "downloading $NAME"
curl -fL --progress-bar -o "$tmp/$NAME" "$BASE_URL/$NAME" || die "download failed: $BASE_URL/$NAME"
if curl -fsSL -o "$tmp/$NAME.sha256" "$BASE_URL/$NAME.sha256"; then
  (cd "$tmp" && sha256sum -c "$NAME.sha256" >/dev/null) || die "checksum mismatch"
  say "checksum OK"
else
  say "no checksum published, skipping verification"
fi

# A running copy keeps the single-instance lock, so stop it before replacing the file.
pkill -x teams-tray 2>/dev/null || true
mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/$NAME" "$INSTALL_DIR/$NAME"
say "installed to $INSTALL_DIR/$NAME"

if [ "${XDG_CURRENT_DESKTOP:-}" != "${XDG_CURRENT_DESKTOP#*GNOME}" ] && command -v gnome-extensions >/dev/null \
   && ! gnome-extensions list --enabled 2>/dev/null | grep -qi appindicator; then
  say "note: enable the 'AppIndicator and KStatusNotifierItem Support' GNOME extension to see the tray icon"
fi

if [ "$RUN" -eq 1 ]; then
  say "starting Teams for Linux"
  setsid nohup "$INSTALL_DIR/$NAME" >/dev/null 2>&1 &
else
  say "start it with: $INSTALL_DIR/$NAME"
fi
