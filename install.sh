#!/bin/sh
# Installs the latest engram-notes build from GitHub releases.
#   curl -fsSL https://raw.githubusercontent.com/overcuriousity/engram-notes/master/install.sh | sh
# ENGRAM_NOTES_SLIM=1 takes the bare binary instead of the AppImage, for a
# machine that already has webkit2gtk 4.1 and GTK 3.
set -eu

repo=${ENGRAM_NOTES_REPO:-overcuriousity/engram-notes}
tag=${ENGRAM_NOTES_TAG:-latest}
bin_dir=${ENGRAM_NOTES_BIN_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}

die() { printf 'engram-notes: %s\n' "$1" >&2; exit 1; }

case "$(uname -s)" in
  Linux) ;;
  Darwin) die "macOS builds are not published yet; build from source instead" ;;
  *) die "on Windows, download engram-notes-x86_64-pc-windows-msvc.zip from https://github.com/$repo/releases/$tag" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) ;;
  *) die "only x86_64 Linux is published so far (this is $(uname -m))" ;;
esac

if [ "${ENGRAM_NOTES_SLIM:-0}" = 1 ]; then
  asset=engram-notes-x86_64-unknown-linux-gnu.tar.gz
else
  asset=engram-notes-x86_64.AppImage
fi
base=https://github.com/$repo/releases/download/$tag

command -v curl >/dev/null 2>&1 || die "curl is required"
if command -v sha256sum >/dev/null 2>&1; then
  checksum="sha256sum -c -"
elif command -v shasum >/dev/null 2>&1; then
  checksum="shasum -a 256 -c -"
else
  die "sha256sum or shasum is required"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

# A private repository serves no anonymous download; the gh CLI carries the token.
fetch() {
  if curl -fsSL "$base/$1" -o "$tmp/$1"; then
    return 0
  elif command -v gh >/dev/null 2>&1; then
    gh release download "$tag" --repo "$repo" --pattern "$1" --dir "$tmp" --clobber
  else
    die "cannot download $1 from $base — if the repository is private, install the gh CLI and log in"
  fi
}

printf 'engram-notes: downloading %s from %s\n' "$asset" "$tag"
fetch "$asset"
fetch "$asset.sha256"

(cd "$tmp" && $checksum < "$asset.sha256" >/dev/null) ||
  die "checksum mismatch — refusing to install"

case "$asset" in
  *.tar.gz) tar -xzf "$tmp/$asset" -C "$tmp"; built=$tmp/engram-notes ;;
  *) built=$tmp/$asset ;;
esac

mkdir -p "$bin_dir"
install -m 755 "$built" "$bin_dir/engram-notes"
printf 'engram-notes: installed to %s/engram-notes\n' "$bin_dir"

case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) printf 'engram-notes: %s is not on your PATH; add it to your shell profile\n' "$bin_dir" ;;
esac

case "$asset" in
  *.tar.gz)
    if ldd "$bin_dir/engram-notes" 2>/dev/null | grep -q 'not found'; then
      printf 'engram-notes: shared libraries are missing; install webkit2gtk 4.1 and GTK 3, or rerun without ENGRAM_NOTES_SLIM\n'
    fi ;;
  *)
    # An AppImage mounts itself through FUSE 2; without it, it can still unpack.
    if ! ldconfig -p 2>/dev/null | grep -q libfuse.so.2; then
      printf 'engram-notes: libfuse2 is missing; install it, or run `engram-notes --appimage-extract-and-run <vault>`\n'
    fi ;;
esac

printf 'engram-notes: run it with `engram-notes <vault>`\n'
exit 0
