#!/bin/bash
# Setup script for Super STT group permissions

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Super STT Group Setup${NC}"
echo "======================="
echo ""

# Check if running as root
if [ "$EUID" -eq 0 ]; then 
   echo -e "${RED}Please do not run this script as root${NC}"
   echo "Run it as a regular user. You will be prompted for sudo password when needed."
   exit 1
fi

# Check if stt group exists
if getent group stt > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Group 'stt' already exists"
else
    echo "Creating group 'stt'..."
    sudo groupadd stt
    echo -e "${GREEN}✓${NC} Group 'stt' created"
fi

# Add current user to stt group
USERNAME=$(whoami)
echo "Adding user '$USERNAME' to group 'stt'..."

if groups "$USERNAME" | grep -q '\bstt\b'; then
    echo -e "${GREEN}✓${NC} User '$USERNAME' is already in group 'stt'"
else
    sudo usermod -a -G stt "$USERNAME"
    echo -e "${GREEN}✓${NC} User '$USERNAME' added to group 'stt'"
    echo ""
    echo -e "${YELLOW}⚠️  IMPORTANT:${NC} You need to log out and log back in for group changes to take effect"
    echo "   Alternatively, run: newgrp stt"
fi

# Set ownership for socket directory if it exists
SOCKET_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/stt"
if [ -d "$SOCKET_DIR" ]; then
    echo "Setting ownership for socket directory..."
    sudo chown "$USERNAME:stt" "$SOCKET_DIR"
    chmod 770 "$SOCKET_DIR"
    echo -e "${GREEN}✓${NC} Socket directory permissions configured"
fi

echo ""
echo -e "${GREEN}Setup complete!${NC}"
echo ""
echo "Security model:"
echo "  • Daemon socket uses permissions 0660 (owner + group access)"
echo "  • Only members of 'stt' group can connect to the daemon"
echo "  • This prevents unauthorized local users from accessing the service"
echo ""
echo "To verify your group membership, run: groups"