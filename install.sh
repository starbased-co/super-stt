#!/bin/bash

# Super STT Installation Script
# This script downloads and installs the appropriate pre-built binaries

# Force immediate output for debugging piped execution
exec 2>&1

# Very early debug output
printf "SCRIPT START: Super STT Installation Script\n" >&2

set -e

# Debug mode for troubleshooting piped installation
if [ "${DEBUG_INSTALL}" = "1" ]; then
    set -x
fi

printf "SCRIPT INITIALIZED: Starting setup...\n" >&2

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

printf "COLORS DEFINED: Color variables set\n" >&2

# Print functions
print_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
print_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

printf "FUNCTIONS DEFINED: Print functions loaded\n" >&2

# Detect system architecture
detect_arch() {
    local arch=$(uname -m)
    case "$arch" in
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) print_error "Unsupported architecture: $arch"; exit 1 ;;
    esac
}

# Detect GPU compute capability
detect_gpu_compute_cap() {
    if ! command -v nvidia-smi &> /dev/null; then
        return
    fi

    # Get GPU name and try to determine compute capability
    local gpu_name=$(timeout 10 nvidia-smi --query-gpu=name --format=csv,noheader,nounits 2>/dev/null | head -1)

    if [ -z "$gpu_name" ]; then
        return
    fi

    print_info "Detected GPU: $gpu_name" >&2

    # Map GPU names to compute capabilities
    case "$gpu_name" in
        # SM 12.0 (Blackwell)
        *"RTX 50"*|*"RTX PRO"*"Blackwell"*) echo "120" ;;

        # SM 10.0 (Blackwell server)
        *"B200"*|*"GB200"*) echo "100" ;;

        # SM 9.0 (Hopper)
        *"H100"*|*"H200"*|*"GH200"*) echo "90" ;;

        # SM 8.9 (Ada Lovelace)
        *"RTX 40"*|*"RTX Ada"*|*"L4"*|*"L40"*) echo "89" ;;

        # SM 8.6 (Ampere consumer)
        *"RTX 30"*|*"RTX A"[0-9]*|*"A40"*|*"A10"*|*"A16"*|*"A2"*) echo "86" ;;

        # SM 8.0 (Ampere datacenter)
        *"A100"*|*"A30"*) echo "80" ;;

        # SM 7.5 (Turing)
        *"RTX 20"*|*"TITAN RTX"*|*"QUADRO RTX"*|*"T4"*|*"T1000"*|*"T1200"*|*"T2000"*|*"T400"*|*"T500"*|*"T600"*|*"GTX 1650 Ti"*) echo "75" ;;

        # Default fallback - use most compatible (SM 7.5)
        *)
            print_warn "Unknown GPU: $gpu_name - using SM 7.5 (most compatible)" >&2
            echo "75"
            ;;
    esac
}

# Detect CUDA/cuDNN availability and compute capability
detect_cuda() {
    if command -v nvidia-smi &> /dev/null; then
        print_info "NVIDIA GPU detected" >&2
        if timeout 10 nvidia-smi --version &> /dev/null && timeout 10 nvidia-smi | grep -q "CUDA Version"; then
            CUDA_VERSION=$(timeout 10 nvidia-smi | grep "CUDA Version" | sed 's/.*CUDA Version: \([0-9.]*\).*/\1/')
            print_info "CUDA runtime $CUDA_VERSION detected" >&2

            # Detect compute capability
            local compute_cap=$(detect_gpu_compute_cap)

            # Check for cuDNN
            CUDNN_FOUND=false
            if [ -f /usr/local/cuda/include/cudnn.h ] || [ -f /usr/include/cudnn.h ] || \
               [ -f /usr/local/include/cudnn.h ] || ldconfig -p 2>/dev/null | grep -q libcudnn || \
               find /usr -name "libcudnn.so*" 2>/dev/null | grep -q .; then
                CUDNN_FOUND=true
            fi

            if [ -n "$compute_cap" ]; then
                if [ "$CUDNN_FOUND" = true ]; then
                    print_info "cuDNN found - using cuda-cudnn-sm${compute_cap} variant" >&2
                    echo "cuda-cudnn-sm${compute_cap}"
                else
                    print_info "cuDNN not found - using cuda-sm${compute_cap} variant" >&2
                    echo "cuda-sm${compute_cap}"
                fi
            else
                # Fallback to generic CUDA variants
                if [ "$CUDNN_FOUND" = true ]; then
                    print_info "cuDNN found - using generic cuda-cudnn-sm75 variant" >&2
                    echo "cuda-cudnn-sm75"
                else
                    print_info "cuDNN not found - using generic cuda-sm75 variant" >&2
                    echo "cuda-sm75"
                fi
            fi
        else
            print_warn "NVIDIA GPU found but CUDA runtime not available - using CPU variant" >&2
            echo "cpu"
        fi
    else
        print_info "No NVIDIA GPU detected - using CPU variant" >&2
        echo "cpu"
    fi
}

# Setup stt group for secure daemon access
setup_stt_group() {
    if command -v groupadd &> /dev/null; then
        if ! getent group stt > /dev/null 2>&1; then
            print_info "Creating stt group..."
            sudo groupadd stt || true
        fi
        print_info "Adding current user to stt group..."
        sudo usermod -a -G stt "$(whoami)" || true
    fi
}

# Install daemon and CLI components
install_daemon() {
    local temp_dir="$1"
    local install_prefix="$2"

    if [ ! -f "$temp_dir/super-stt" ]; then
        print_error "super-stt binary not found"
        return 1
    fi

    # Setup group and directories
    setup_stt_group
    mkdir -p "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/stt"
    mkdir -p "$HOME/.local/share/stt/logs"
    mkdir -p "$HOME/.config/systemd/user"

    # Install daemon binary
    print_info "Installing daemon and CLI..."
    mkdir -p "$install_prefix/bin"

    if [ -w "$install_prefix/bin" ]; then
        install -m 755 "$temp_dir/super-stt" "$install_prefix/bin/"
    else
        sudo install -m 755 "$temp_dir/super-stt" "$install_prefix/bin/"
    fi

    # Create wrapper script for group switching
    cat > "$temp_dir/stt" << EOF
#!/bin/bash
# Super STT Daemon Wrapper
exec sg stt -c "$install_prefix/bin/super-stt \\$*"
EOF

    if [ -w "$install_prefix/bin" ]; then
        install -m 755 "$temp_dir/stt" "$install_prefix/bin/"
    else
        sudo install -m 755 "$temp_dir/stt" "$install_prefix/bin/"
    fi
}

# Install desktop app
install_app() {
    local temp_dir="$1"
    local install_prefix="$2"

    if [ ! -f "$temp_dir/super-stt-app" ]; then
        print_warn "Desktop app binary not found - skipping"
        return 0
    fi

    print_info "Installing desktop app..."
    mkdir -p "$install_prefix/bin"

    # Install binary
    if [ -w "$install_prefix/bin" ]; then
        install -m 755 "$temp_dir/super-stt-app" "$install_prefix/bin/"
    else
        sudo install -m 755 "$temp_dir/super-stt-app" "$install_prefix/bin/"
    fi

    # Install desktop file and icons if available
    if [ -d "$temp_dir/resources/app" ]; then
        print_info "Installing desktop integration files..."

        # Determine installation paths
        if [[ "$install_prefix" == "/usr"* ]]; then
            # System-wide installation
            DESKTOP_DIR="/usr/share/applications"
            ICON_DIR="/usr/share/icons/hicolor"
            METAINFO_DIR="/usr/share/metainfo"
            USE_SUDO=true
        else
            # User installation
            DESKTOP_DIR="$HOME/.local/share/applications"
            ICON_DIR="$HOME/.local/share/icons/hicolor"
            METAINFO_DIR="$HOME/.local/share/metainfo"
            USE_SUDO=false
        fi

        # Install desktop file
        if [ -f "$temp_dir/resources/app/app.desktop" ]; then
            mkdir -p "$DESKTOP_DIR"
            if [ "$USE_SUDO" = true ]; then
                sudo install -m 644 "$temp_dir/resources/app/app.desktop" "$DESKTOP_DIR/super-stt-app.desktop"
            else
                install -m 644 "$temp_dir/resources/app/app.desktop" "$DESKTOP_DIR/super-stt-app.desktop"
            fi
        fi

        # Install icons
        if [ -d "$temp_dir/resources/app/icons" ]; then
            mkdir -p "$ICON_DIR"
            if [ "$USE_SUDO" = true ]; then
                sudo cp -r "$temp_dir/resources/app/icons"/* "$ICON_DIR/"
            else
                cp -r "$temp_dir/resources/app/icons"/* "$ICON_DIR/"
            fi
        fi

        # Install metainfo
        if [ -f "$temp_dir/resources/app/app.metainfo.xml" ]; then
            mkdir -p "$METAINFO_DIR"
            if [ "$USE_SUDO" = true ]; then
                sudo install -m 644 "$temp_dir/resources/app/app.metainfo.xml" "$METAINFO_DIR/super-stt-app.metainfo.xml"
            else
                install -m 644 "$temp_dir/resources/app/app.metainfo.xml" "$METAINFO_DIR/super-stt-app.metainfo.xml"
            fi
        fi

        # Update icon cache
        if command -v gtk-update-icon-cache &> /dev/null; then
            if [ "$USE_SUDO" = true ]; then
                sudo gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
            else
                gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
            fi
        fi
    fi
}

# Install COSMIC applet
install_applet() {
    local temp_dir="$1"
    local install_prefix="$2"

    if [ ! -f "$temp_dir/super-stt-cosmic-applet" ]; then
        print_warn "COSMIC applet binary not found - skipping"
        return 0
    fi

    if ! command -v cosmic-panel &> /dev/null; then
        print_warn "COSMIC panel not found - skipping applet installation"
        return 0
    fi

    print_info "Installing COSMIC applet..."
    mkdir -p "$install_prefix/bin"

    # Install binary
    if [ -w "$install_prefix/bin" ]; then
        install -m 755 "$temp_dir/super-stt-cosmic-applet" "$install_prefix/bin/"
    else
        sudo install -m 755 "$temp_dir/super-stt-cosmic-applet" "$install_prefix/bin/"
    fi

    # Install desktop files and icons if available
    if [ -d "$temp_dir/resources/cosmic-applet" ]; then
        print_info "Installing COSMIC applet integration files..."

        # Determine installation paths
        if [[ "$install_prefix" == "/usr"* ]]; then
            # System-wide installation
            DESKTOP_DIR="/usr/share/applications"
            ICON_DIR="/usr/share/icons/hicolor"
            USE_SUDO=true
        else
            # User installation
            DESKTOP_DIR="$HOME/.local/share/applications"
            ICON_DIR="$HOME/.local/share/icons/hicolor"
            USE_SUDO=false
        fi

        # Install desktop files
        mkdir -p "$DESKTOP_DIR"
        for desktop_file in "$temp_dir/resources/cosmic-applet"/*.desktop; do
            if [ -f "$desktop_file" ]; then
                local filename=$(basename "$desktop_file")
                if [ "$USE_SUDO" = true ]; then
                    sudo install -m 644 "$desktop_file" "$DESKTOP_DIR/$filename"
                else
                    install -m 644 "$desktop_file" "$DESKTOP_DIR/$filename"
                fi
            fi
        done

        # Install icons
        if [ -d "$temp_dir/resources/cosmic-applet/icons" ]; then
            mkdir -p "$ICON_DIR"
            if [ "$USE_SUDO" = true ]; then
                sudo cp -r "$temp_dir/resources/cosmic-applet/icons"/* "$ICON_DIR/"
            else
                cp -r "$temp_dir/resources/cosmic-applet/icons"/* "$ICON_DIR/"
            fi

            # Update icon cache
            if command -v gtk-update-icon-cache &> /dev/null; then
                if [ "$USE_SUDO" = true ]; then
                    sudo gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
                else
                    gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
                fi
            fi
        fi
    fi
}

# Install systemd service
install_service() {
    local temp_dir="$1"

    if ! command -v systemctl &> /dev/null; then
        print_warn "Systemd not detected - skipping service installation"
        return 0
    fi

    if [ ! -f "$temp_dir/systemd/super-stt.service" ]; then
        print_warn "Service file not found - skipping"
        return 0
    fi

    print_info "Installing systemd service..."
    USER_SYSTEMD_DIR="$HOME/.config/systemd/user"
    mkdir -p "$USER_SYSTEMD_DIR"
    cp "$temp_dir/systemd/super-stt.service" "$USER_SYSTEMD_DIR/"
    systemctl --user daemon-reload
}

# Update PATH if needed
update_path() {
    local bin_dir="$1"

    if [[ ":$PATH:" != *":$bin_dir:"* ]]; then
        print_warn "Add $bin_dir to your PATH:"
        print_warn "  export PATH=\"$bin_dir:\$PATH\""

        # Try to add to shell config for user installations
        if [[ "$bin_dir" == "$HOME"* ]]; then
            SHELL_CONFIG=""
            if [ -f "$HOME/.bashrc" ]; then
                SHELL_CONFIG="$HOME/.bashrc"
            elif [ -f "$HOME/.zshrc" ]; then
                SHELL_CONFIG="$HOME/.zshrc"
            fi

            if [ -n "$SHELL_CONFIG" ]; then
                if ! grep -q "$bin_dir" "$SHELL_CONFIG"; then
                    echo "export PATH=\"$bin_dir:\$PATH\"" >> "$SHELL_CONFIG"
                    print_info "Added PATH update to $SHELL_CONFIG"
                fi
            fi
        fi
    fi
}

# Configure COSMIC keyboard shortcut
configure_cosmic_shortcut() {
    local install_prefix="$1"

    # Check if we're on COSMIC desktop
    if ! command -v cosmic-panel &> /dev/null; then
        return 0
    fi

    COSMIC_SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1"
    COSMIC_SHORTCUTS_FILE="$COSMIC_SHORTCUTS_DIR/custom"

    # Ask user if they want to add the shortcut
    echo -n "Add COSMIC keyboard shortcut (Super+Space)? [Y/n]: "
    if [ -t 2 ]; then
        read -r add_shortcut < /dev/tty
    else
        # Non-interactive fallback - default to yes
        add_shortcut="y"
        echo "y"
    fi

    if [[ "$add_shortcut" =~ ^[Nn]$ ]]; then
        return 0
    fi

    # Create the shortcuts directory if it doesn't exist
    mkdir -p "$COSMIC_SHORTCUTS_DIR"

    # Use the full path to the stt wrapper for reliability
    local stt_command="$install_prefix/bin/stt record --write"

    # Check if shortcuts file exists and has content
    if [ -f "$COSMIC_SHORTCUTS_FILE" ] && [ -s "$COSMIC_SHORTCUTS_FILE" ]; then
        # File exists with content, check if our shortcut is already there
        if grep -q "Super STT" "$COSMIC_SHORTCUTS_FILE"; then
            return 0
        fi

        # Check if Super+Space is already used
        if grep -q 'key: "space"' "$COSMIC_SHORTCUTS_FILE" && grep -A5 -B5 'key: "space"' "$COSMIC_SHORTCUTS_FILE" | grep -q 'Super'; then
            return 0
        fi

        # Create a backup
        cp "$COSMIC_SHORTCUTS_FILE" "$COSMIC_SHORTCUTS_FILE.backup"

        # Create a temporary file with the new shortcut entry
        local temp_file=$(mktemp)

        # Check if the file is empty (just {}) and handle accordingly
        if grep -q '^{}$' "$COSMIC_SHORTCUTS_FILE"; then
            # File is empty, replace entirely
            echo '{' > "$temp_file"
        else
            # File has content, remove the closing brace and add our shortcut
            head -n -1 "$COSMIC_SHORTCUTS_FILE" > "$temp_file"
        fi

        cat >> "$temp_file" << EOFSHORTCUT
    (
        modifiers: [
            Super,
        ],
        key: "space",
        description: Some("Super STT"),
    ): Spawn("$stt_command"),
}
EOFSHORTCUT

        # Replace the original file
        mv "$temp_file" "$COSMIC_SHORTCUTS_FILE"

        # Verify the file is valid by checking it has proper structure
        if ! grep -q '^{' "$COSMIC_SHORTCUTS_FILE" || ! grep -q '^}' "$COSMIC_SHORTCUTS_FILE"; then
            mv "$COSMIC_SHORTCUTS_FILE.backup" "$COSMIC_SHORTCUTS_FILE"
            return 1
        fi

        # Remove backup if successful
        rm -f "$COSMIC_SHORTCUTS_FILE.backup"
    else
        # Create new shortcuts file with our shortcut

        cat > "$COSMIC_SHORTCUTS_FILE" << EOF
{
    (
        modifiers: [
            Super,
        ],
        key: "space",
        description: Some("Super STT"),
    ): Spawn("$stt_command"),
}
EOF
    fi

}

# Default values
INSTALL_PREFIX="$HOME/.local"
GITHUB_REPO="jorge-menjivar/super-stt"
VERSION="latest"
INSTALL_OPTION="all"

# Parse arguments
INTERACTIVE=true
for arg in "$@"; do
    case $arg in
        --non-interactive)
            INTERACTIVE=false
            shift
            ;;
        --version=*)
            VERSION="${arg#*=}"
            shift
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
    esac
done

# Interactive menu function
show_menu() {
    clear
    echo "============================================="
    echo "         Super STT Installation Menu"
    echo "============================================="
    echo ""
    echo "Detected system:"
    echo "  Architecture: $ARCH"
    echo "  Optimal variant: $VARIANT"
    echo ""
    echo "What would you like to install?"
    echo ""
    echo "1. All $([ ! command -v cosmic-panel &> /dev/null ] && echo "(skip COSMIC applet)" || echo "(includes COSMIC applet)") [DEFAULT]"
    echo "2. Daemon + CLI only"
    echo "3. Desktop App only"
    echo "4. COSMIC Applet only $([ ! command -v cosmic-panel &> /dev/null ] && echo "(not available)")"
    echo ""
    echo "q. Quit"
    echo ""
    echo "============================================="
}

handle_menu() {
    printf "DEBUG: Entering handle_menu function\n" >&2
    while true; do
        printf "DEBUG: Showing menu\n" >&2
        show_menu
        printf "DEBUG: Waiting for user input\n" >&2
        echo -n "Select option [1-4, q] (default: 1): "
        read -r choice
        printf "DEBUG: User selected: '%s'\n" "$choice" >&2

        # Default to option 1 if empty
        if [ -z "$choice" ]; then
            choice="1"
        fi

        case $choice in
            1)
                printf "DEBUG: Selected option 1 (all)\n" >&2
                INSTALL_OPTION="all"
                break
                ;;
            2)
                printf "DEBUG: Selected option 2 (daemon)\n" >&2
                INSTALL_OPTION="daemon"
                break
                ;;
            3)
                printf "DEBUG: Selected option 3 (app)\n" >&2
                INSTALL_OPTION="app"
                break
                ;;
            4)
                printf "DEBUG: Selected option 4 (applet)\n" >&2
                if ! command -v cosmic-panel &> /dev/null; then
                    print_warn "COSMIC panel not found - applet not available"
                    sleep 1
                else
                    INSTALL_OPTION="applet"
                    break
                fi
                ;;
            q|Q)
                printf "DEBUG: Selected quit\n" >&2
                print_info "Installation cancelled"
                exit 0
                ;;
            *)
                printf "DEBUG: Invalid option: '%s'\n" "$choice" >&2
                print_warn "Invalid option. Please try again."
                sleep 1
                ;;
        esac
    done
    printf "DEBUG: Exiting handle_menu function with INSTALL_OPTION='%s'\n" "$INSTALL_OPTION" >&2
}

# Show interactive menu if in interactive mode
# Check if we have a controlling terminal (works better with piped input)
if [ "$INTERACTIVE" = true ] && [ -t 2 ]; then
    echo "DEBUG: Starting interactive menu setup..." >&2
    # Do minimal detection for menu display (avoid CUDA detection here)
    echo "DEBUG: Detecting architecture for menu..." >&2
    ARCH=$(detect_arch)
    echo "DEBUG: Setting placeholder variant for menu..." >&2
    VARIANT="detecting..."  # Placeholder for menu display
    echo "DEBUG: Starting menu interaction..." >&2

    # Redirect stdin from the controlling terminal for the menu
    exec < /dev/tty
    handle_menu
    clear
    echo "DEBUG: Menu completed, install option: $INSTALL_OPTION" >&2
fi

echo "Checking for sudo"

# Check if we need sudo and prompt early for daemon installations
if [ "$INSTALL_OPTION" = "all" ] || [ "$INSTALL_OPTION" = "daemon" ]; then
    echo "DEBUG: Checking sudo access for daemon installation..." >&2
    print_info "Daemon installation requires sudo to create the stt group"
    print_info "You will be prompted for your password..."
    # Test sudo access early
    if [ -t 2 ]; then
        # Terminal available, test sudo normally
        if ! sudo -v; then
            print_error "Sudo access required for daemon installation"
            exit 1
        fi
    else
        # Piped input, redirect sudo from terminal
        if ! sudo -v < /dev/tty; then
            print_error "Sudo access required for daemon installation"
            exit 1
        fi
    fi
    echo "DEBUG: Sudo access confirmed" >&2
fi

# Detect what we need based on install option
print_info "Starting architecture detection..."
ARCH=$(detect_arch)
print_info "Detected architecture: $ARCH"

if [ "$INSTALL_OPTION" = "all" ] || [ "$INSTALL_OPTION" = "daemon" ]; then
    # Need optimal daemon variant
    print_info "Starting GPU/CUDA detection for daemon..."
    VARIANT=$(detect_cuda)
    print_info "Using variant: $VARIANT"
else
    # For app/applet-only, just use CPU variant (app/applet are identical in all variants)
    print_info "Using CPU variant for app/applet-only installation"
    VARIANT="cpu"
fi

# Get the latest release version if not specified
if [ "$VERSION" = "latest" ]; then
    print_info "Fetching latest release version..."
    VERSION=$(curl -s "https://api.github.com/repos/$GITHUB_REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        print_error "Failed to fetch latest version"
        exit 1
    fi
    print_info "Found version: $VERSION"
fi

print_info "Installing Super STT $VERSION"

# Construct download URL
TARBALL_NAME="super-stt-${ARCH}-${VARIANT}.tar.gz"
DOWNLOAD_URL="https://github.com/$GITHUB_REPO/releases/download/$VERSION/$TARBALL_NAME"

print_info "Downloading from: $DOWNLOAD_URL"
print_info "Creating temporary directory..."

# Create temporary directory
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

# Download the tarball with fallback support
download_with_fallback() {
    local variant="$1"
    local arch="$2"
    local tarball_name="super-stt-${arch}-${variant}.tar.gz"
    local download_url="https://github.com/$GITHUB_REPO/releases/download/$VERSION/$tarball_name"

    print_info "Trying to download: $tarball_name" >&2

    if curl -L -o "$TEMP_DIR/$tarball_name" "$download_url" 2>/dev/null; then
        echo "$tarball_name"
        return 0
    fi

    # If specific compute capability failed, try fallbacks
    if [[ "$variant" =~ cuda.*-sm[0-9]+ ]]; then
        local base_variant
        if [[ "$variant" =~ cuda-cudnn ]]; then
            base_variant="cuda-cudnn-sm75"  # Most compatible cuDNN variant
        else
            base_variant="cuda-sm75"        # Most compatible CUDA variant
        fi

        if [ "$variant" != "$base_variant" ]; then
            print_warn "Specific compute capability variant not found, trying $base_variant" >&2
            tarball_name="super-stt-${arch}-${base_variant}.tar.gz"
            download_url="https://github.com/$GITHUB_REPO/releases/download/$VERSION/$tarball_name"

            if curl -L -o "$TEMP_DIR/$tarball_name" "$download_url" 2>/dev/null; then
                echo "$tarball_name"
                return 0
            fi
        fi

        # Last resort: try CPU variant
        print_warn "CUDA variants not found, falling back to CPU variant" >&2
        tarball_name="super-stt-${arch}-cpu.tar.gz"
        download_url="https://github.com/$GITHUB_REPO/releases/download/$VERSION/$tarball_name"

        if curl -L -o "$TEMP_DIR/$tarball_name" "$download_url" 2>/dev/null; then
            echo "$tarball_name"
            return 0
        fi
    fi

    return 1
}

# Try to download with fallback support
DOWNLOADED_TARBALL=$(download_with_fallback "$VARIANT" "$ARCH")

if [ -z "$DOWNLOADED_TARBALL" ]; then
    print_error "Failed to download any compatible variant"
    print_error "Tried URLs:"
    print_error "  - https://github.com/$GITHUB_REPO/releases/download/$VERSION/super-stt-${ARCH}-${VARIANT}.tar.gz"
    if [[ "$VARIANT" =~ cuda.*-sm[0-9]+ ]]; then
        if [[ "$VARIANT" =~ cuda-cudnn ]]; then
            print_error "  - https://github.com/$GITHUB_REPO/releases/download/$VERSION/super-stt-${ARCH}-cuda-cudnn-sm75.tar.gz"
        else
            print_error "  - https://github.com/$GITHUB_REPO/releases/download/$VERSION/super-stt-${ARCH}-cuda-sm75.tar.gz"
        fi
        print_error "  - https://github.com/$GITHUB_REPO/releases/download/$VERSION/super-stt-${ARCH}-cpu.tar.gz"
    fi
    exit 1
fi

print_info "Successfully downloaded: $DOWNLOADED_TARBALL"

# Extract the tarball
print_info "Extracting binaries..."
tar -xzf "$TEMP_DIR/$DOWNLOADED_TARBALL" -C "$TEMP_DIR"

# Install components based on selection
case $INSTALL_OPTION in
    "all")
        # Install everything (skip applet if COSMIC not available)
        install_daemon "$TEMP_DIR" "$INSTALL_PREFIX"
        install_app "$TEMP_DIR" "$INSTALL_PREFIX"
        if command -v cosmic-panel &> /dev/null; then
            install_applet "$TEMP_DIR" "$INSTALL_PREFIX"
        fi
        install_service "$TEMP_DIR"
        ;;
    "daemon")
        # Install daemon + CLI + service
        install_daemon "$TEMP_DIR" "$INSTALL_PREFIX"
        install_service "$TEMP_DIR"
        ;;
    "app")
        # Install app only
        install_app "$TEMP_DIR" "$INSTALL_PREFIX"
        ;;
    "applet")
        # Install applet only
        install_applet "$TEMP_DIR" "$INSTALL_PREFIX"
        ;;
esac

# Add to PATH if installing to non-standard location
if [ "$INSTALL_PREFIX" != "/usr/local" ] && [ "$INSTALL_PREFIX" != "/usr" ]; then
    update_path "$INSTALL_PREFIX/bin"
fi

# Configure COSMIC shortcut if daemon was installed and in interactive mode
if [ "$INTERACTIVE" = true ] && ([ "$INSTALL_OPTION" = "all" ] || [ "$INSTALL_OPTION" = "daemon" ]); then
    configure_cosmic_shortcut "$INSTALL_PREFIX"
fi

print_info ""
print_info "Installation complete!"
print_info ""
print_info "Installed components:"

# Determine the actual variant that was installed
INSTALLED_VARIANT=$(echo "$DOWNLOADED_TARBALL" | sed 's/super-stt-.*-\(.*\)\.tar\.gz/\1/')

case $INSTALL_OPTION in
    "all")
        print_info "  ✅ super-stt (daemon + CLI) [$INSTALLED_VARIANT variant]"
        print_info "  ✅ stt (convenience wrapper)"
        [ -f "$INSTALL_PREFIX/bin/super-stt-app" ] && print_info "  ✅ super-stt-app (desktop app)"
        [ -f "$INSTALL_PREFIX/bin/super-stt-cosmic-applet" ] && print_info "  ✅ super-stt-cosmic-applet (COSMIC applet)"
        print_info "  ✅ systemd user service"
        print_info ""
        print_info "Remember to log out and back in, or run: newgrp stt"
        print_info ""
        print_info "Then run 'stt record --write' to get started"
        ;;
    "daemon")
        print_info "  ✅ super-stt (daemon + CLI) [$INSTALLED_VARIANT variant]"
        print_info "  ✅ stt (convenience wrapper)"
        print_info "  ✅ systemd user service"
        print_info ""
        print_info "Remember to log out and back in, or run: newgrp stt"
        print_info ""
        print_info "Then run 'stt record --write' to get started"
        ;;
    "app")
        [ -f "$INSTALL_PREFIX/bin/super-stt-app" ] && print_info "  ✅ super-stt-app (desktop app)"
        print_info ""
        print_info "Desktop app installed. Note: You'll need the daemon to use Super STT functionality."
        ;;
    "applet")
        [ -f "$INSTALL_PREFIX/bin/super-stt-cosmic-applet" ] && print_info "  ✅ super-stt-cosmic-applet (COSMIC applet)"
        print_info ""
        print_info "COSMIC applet installed. Note: You'll need the daemon to use Super STT functionality."
        ;;
esac
