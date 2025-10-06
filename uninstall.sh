#!/bin/bash

# Super STT Uninstallation Script
# Removes all installed components and configuration

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Default paths
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons"
METAINFO_DIR="$HOME/.local/share/metainfo"
USER_SYSTEMD_DIR="$HOME/.config/systemd/user"
CONFIG_DIR="$HOME/.config/super-stt"
DATA_DIR="$HOME/.local/share/stt"
RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/stt"

# Print functions
print_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
print_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Parse arguments
KEEP_CONFIG=false
KEEP_DATA=false

for arg in "$@"; do
  case $arg in
    --keep-config)
      KEEP_CONFIG=true
      shift
      ;;
    --keep-data)
      KEEP_DATA=true
      shift
      ;;
    --help|-h)
      echo "Super STT Uninstall Script"
      echo ""
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --keep-config    Keep configuration files"
      echo "  --keep-data      Keep data and logs"
      echo "  --help           Show this help message"
      exit 0
      ;;
  esac
done

echo "============================================="
echo "      Super STT Uninstallation"
echo "============================================="
echo ""

# Stop and disable systemd service
if [ -f "$USER_SYSTEMD_DIR/super-stt.service" ]; then
  print_info "Stopping and disabling systemd service..."
  systemctl --user stop super-stt 2>/dev/null || true
  systemctl --user disable super-stt 2>/dev/null || true
  systemctl --user daemon-reload
fi

# Remove binaries
print_info "Removing binaries..."
rm -f "$INSTALL_PREFIX/bin/super-stt"
rm -f "$INSTALL_PREFIX/bin/stt"
rm -f "$INSTALL_PREFIX/bin/super-stt-app"
rm -f "$INSTALL_PREFIX/bin/super-stt-cosmic-applet"

# Remove desktop files
print_info "Removing desktop integration files..."
rm -f "$DESKTOP_DIR/super-stt-app.desktop"
rm -f "$DESKTOP_DIR"/super-stt-cosmic-applet-*.desktop

# Remove icons
rm -f "$ICON_DIR/super-stt-app.svg"
rm -f "$ICON_DIR/super-stt-cosmic-applet.svg"

# Remove metainfo
rm -f "$METAINFO_DIR/super-stt-app.metainfo.xml"

# Remove systemd service
rm -f "$USER_SYSTEMD_DIR/super-stt.service"

# Remove COSMIC keyboard shortcut
COSMIC_SHORTCUTS_FILE="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom"
if [ -f "$COSMIC_SHORTCUTS_FILE" ]; then
  if grep -q "Super STT" "$COSMIC_SHORTCUTS_FILE"; then
    print_info "Removing COSMIC keyboard shortcut..."
    cp "$COSMIC_SHORTCUTS_FILE" "$COSMIC_SHORTCUTS_FILE.backup"

    # Remove the Super STT entry
    sed -i '/Super STT/,/Spawn.*stt/d' "$COSMIC_SHORTCUTS_FILE"

    print_info "COSMIC shortcut removed (backup: $COSMIC_SHORTCUTS_FILE.backup)"
  fi
fi

# Remove configuration
if [ "$KEEP_CONFIG" = false ]; then
  if [ -d "$CONFIG_DIR" ]; then
    print_info "Removing configuration directory..."
    rm -rf "$CONFIG_DIR"
  fi
else
  print_warn "Keeping configuration directory: $CONFIG_DIR"
fi

# Remove data and logs
if [ "$KEEP_DATA" = false ]; then
  if [ -d "$DATA_DIR" ]; then
    print_info "Removing data and logs..."
    rm -rf "$DATA_DIR"
  fi
else
  print_warn "Keeping data directory: $DATA_DIR"
fi

# Remove runtime directory
if [ -d "$RUNTIME_DIR" ]; then
  print_info "Removing runtime directory..."
  rm -rf "$RUNTIME_DIR"
fi

# Update desktop database
if command -v update-desktop-database &> /dev/null; then
  print_info "Updating desktop database..."
  update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

# Update icon cache
if command -v gtk-update-icon-cache &> /dev/null; then
  print_info "Updating icon cache..."
  gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi

# Check if user should be removed from stt group
if groups | grep -q "\bstt\b"; then
  print_warn ""
  print_warn "You are still a member of the 'stt' group."
  print_warn "To remove yourself from the group, run:"
  print_warn "  sudo gpasswd -d $(whoami) stt"
  print_warn ""
  print_warn "Note: You need to log out and back in for this to take effect."
fi

print_info ""
print_info "Uninstallation complete!"
print_info ""

if [ "$KEEP_CONFIG" = true ] || [ "$KEEP_DATA" = true ]; then
  print_info "Preserved directories:"
  [ "$KEEP_CONFIG" = true ] && print_info "  󰉋  Config: $CONFIG_DIR"
  [ "$KEEP_DATA" = true ] && print_info "  󰉋  Data: $DATA_DIR"
  print_info ""
  print_info "To remove these manually:"
  [ "$KEEP_CONFIG" = true ] && print_info "  rm -rf $CONFIG_DIR"
  [ "$KEEP_DATA" = true ] && print_info "  rm -rf $DATA_DIR"
fi
