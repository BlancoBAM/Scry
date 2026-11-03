#!/bin/bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Building Scry (Release)..."
cd "$DIR"
cargo build --release

echo "==> Installing binary to /usr/local/bin..."
sudo install -Dm755 target/release/scry /usr/local/bin/scry
sudo ln -sf /usr/local/bin/scry /usr/local/bin/Scry
sudo ln -sf /usr/local/bin/scry /usr/local/bin/lilith-web-terminal

echo "==> Installing desktop launcher..."
sudo install -Dm644 data/org.lilith.Scry.desktop /usr/share/applications/org.lilith.Scry.desktop
sudo install -Dm644 data/org.lilith.Scry.desktop /usr/share/applications/org.lilith.WebTerminal.desktop

echo "==> Installing icon..."
sudo install -Dm644 data/icons/org.lilith.Scry.svg /usr/share/icons/hicolor/scalable/apps/org.lilith.Scry.svg
sudo install -Dm644 data/icons/org.lilith.Scry.svg /usr/share/icons/hicolor/scalable/apps/org.lilith.WebTerminal.svg

echo "==> Updating desktop and icon caches..."
if command -v update-desktop-database >/dev/null 2>&1; then
    sudo update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    sudo gtk-update-icon-cache -q /usr/share/icons/hicolor || true
fi

echo "✅ Scry successfully installed!"
echo "You can now launch it via your application menu or by running: scry"
