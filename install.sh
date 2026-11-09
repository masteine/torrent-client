#!/bin/sh
set -eu

repository=${SIMPLE_TORRENT_REPO:-masteine/torrent-client}

case "$(uname -s)" in
  Darwin)
    case "$(uname -m)" in
      arm64) archive=simple-torrent-macos-arm64.zip ;;
      x86_64) archive=simple-torrent-macos-x86_64.zip ;;
      *) printf '%s\n' 'Ця архітектура macOS поки не підтримується.' >&2; exit 1 ;;
    esac
    platform=macos
    ;;
  Linux)
    if [ "$(uname -m)" != x86_64 ]; then
      printf '%s\n' 'Готова збірка Linux доступна лише для x86_64.' >&2
      exit 1
    fi
    archive=simple-torrent-linux-x86_64.tar.gz
    platform=linux
    ;;
  *) printf '%s\n' 'Для Windows скористайтеся install.ps1.' >&2; exit 1 ;;
esac

temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM
url="https://github.com/$repository/releases/latest/download/$archive"
printf 'Завантажую %s...\n' "$archive"
curl --fail --location --silent --show-error --retry 3 "$url" --output "$temporary_dir/$archive"

if [ "$platform" = macos ]; then
  ditto -x -k "$temporary_dir/$archive" "$temporary_dir"
  mkdir -p "$HOME/Applications"
  ditto "$temporary_dir/Simple Torrent.app" "$HOME/Applications/Simple Torrent.app"
  printf 'Встановлено: %s\n' "$HOME/Applications/Simple Torrent.app"
else
  tar -xzf "$temporary_dir/$archive" -C "$temporary_dir"
  mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
  install -m 755 "$temporary_dir/simple-torrent-gui" "$HOME/.local/bin/simple-torrent-gui"
  cat > "$HOME/.local/share/applications/simple-torrent.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Простий торрент-клієнт
Exec="$HOME/.local/bin/simple-torrent-gui"
Terminal=false
Categories=Network;FileTransfer;
EOF
  printf 'Встановлено: %s\n' "$HOME/.local/bin/simple-torrent-gui"
fi
