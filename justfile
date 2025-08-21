app_name := 'super-stt-app'
daemon_name := 'super-stt'
wrapper_name := 'stt'
cosmic_name := 'cosmic-ext'
applet_name := 'super-stt-cosmic-applet'

# Applet
applet_full_desktop_file_name := 'com.github.jorge-menjivar.super_stt_cosmic_applet-full.desktop'
applet_left_desktop_file_name := 'com.github.jorge-menjivar.super_stt_cosmic_applet-left.desktop'
applet_right_desktop_file_name := 'com.github.jorge-menjivar.super_stt_cosmic_applet-right.desktop'
applet_icon_name := 'com.github.jorge-menjivar.super_stt_cosmic_applet.svg'

# Installation paths
home_dir := env_var('HOME')
user_bin_dir := home_dir / '.local' / 'bin'
system_bin_dir := '/usr/local/bin'
user_systemd_dir := home_dir / '.config' / 'systemd' / 'user'
run_dir := env_var('XDG_RUNTIME_DIR') / 'stt'
log_dir := home_dir / '.local' / 'share' / 'stt' / 'logs'
user_desktop_dir := home_dir / '.local' / 'share' / 'applications'
user_icons_dir := home_dir / '.local' / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'

# Binary paths
app_src := 'target' / 'release' / app_name
daemon_src := 'target' / 'release' / daemon_name
cosmic_src := 'target' / 'release' / cosmic_name
applet_src := 'target' / 'release' / applet_name
debug_applet_src := 'target' / 'debug' / applet_name
app_dst := user_bin_dir / app_name
daemon_dst := user_bin_dir / daemon_name
cosmic_dst := user_bin_dir / cosmic_name
applet_dst := user_bin_dir / applet_name
wrapper_dst := user_bin_dir / wrapper_name

# App files
app_desktop_file_name := 'com.github.jorge-menjivar.super-stt-app.desktop'
app_icon_name := 'com.github.jorge-menjivar.super-stt-app.svg'
app_desktop_file_src := 'super-stt-app' / 'resources' / 'app.desktop'
app_icon_src := 'super-stt-cosmic-applet' / 'data' / 'icons' / 'scalable' / 'app' / 'super-stt-icon.svg'
app_desktop_file_dst := user_desktop_dir / app_desktop_file_name
app_icon_dst := user_icons_dir / app_icon_name

# Other files paths
applet_full_desktop_file_src := 'super-stt-cosmic-applet' / 'data' / applet_full_desktop_file_name
applet_left_desktop_file_src := 'super-stt-cosmic-applet' / 'data' / applet_left_desktop_file_name
applet_right_desktop_file_src := 'super-stt-cosmic-applet' / 'data' / applet_right_desktop_file_name
applet_icon_src := 'super-stt-cosmic-applet' / 'data' / 'icons' / 'scalable' / 'app' / applet_icon_name
applet_full_desktop_file_dst := user_desktop_dir / applet_full_desktop_file_name
applet_left_desktop_file_dst := user_desktop_dir / applet_left_desktop_file_name
applet_right_desktop_file_dst := user_desktop_dir / applet_right_desktop_file_name
applet_icon_dst := user_icons_dir / applet_icon_name

# Service file
service_file := daemon_name + '.service'
service_dst := user_systemd_dir / service_file

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
# Usage: just build-debug [--cuda|--cudnn]
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
# Usage: just build-release [--cuda|--cudnn]
build-release *args:
    cargo build --release {{args}}

# Compiles release profile with vendored dependencies
# Usage: just build-vendored [--cuda|--cudnn]
build-vendored *args: vendor-extract
    just build-release --frozen --offline {{args}}

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run the app for testing purposes
run-app *args:
    env RUST_BACKTRACE=full RUST_LOG=debug cargo run --bin {{app_name}} {{args}}

# Run the daemon for testing purposes
# Usage: just run-daemon [--model MODEL] [other args]
run-daemon *args:
    env RUST_BACKTRACE=full RUST_LOG=debug cargo run --bin {{daemon_name}} -v {{args}}

# Run security audit to check for vulnerabilities
audit:
    cargo audit

# Run the cosmic applet in the cosmic panel for testing purposes
run-applet *args:
    #!/usr/bin/env bash
    set -euo pipefail

    env RUST_BACKTRACE=full RUST_LOG=debug,super_stt_shared=debug,warn cargo build --bin {{applet_name}} {{args}}

    echo "Installing Debug Super STT COSMIC applet..."
    mkdir -p {{user_bin_dir}}
    install -m755 {{debug_applet_src}} {{applet_dst}}

    # Install the debug desktop entries for panel integration
    mkdir -p {{user_desktop_dir}}

    echo "Installing desktop entries for COSMIC panel integration..."
    install -Dm0644 {{applet_full_desktop_file_src}} {{applet_full_desktop_file_dst}}
    install -Dm0644 {{applet_left_desktop_file_src}} {{applet_left_desktop_file_dst}}
    install -Dm0644 {{applet_right_desktop_file_src}} {{applet_right_desktop_file_dst}}

    # Install the applet icon
    mkdir -p {{user_icons_dir}}
    echo "Installing applet icon..."
    install -Dm0644 {{applet_icon_src}} {{applet_icon_dst}}

    cosmic-panel

run-applet-windowed *args:
    env RUST_BACKTRACE=full RUST_LOG=debug,super_stt_shared=debug,warn cargo run --bin {{applet_name}} {{args}}

# Run the cosmic applet in the cosmic panel for testing purposes
run-applet-kill *args:
    #!/usr/bin/env bash
    set -euo pipefail

    env RUST_BACKTRACE=full RUST_LOG=debug,super_stt_shared=debug,warn cargo build --bin {{applet_name}} {{args}}

    echo "Installing Debug Super STT COSMIC applet..."
    mkdir -p {{user_bin_dir}}
    install -m755 {{debug_applet_src}} {{applet_dst}}

    # Install the debug desktop entries for panel integration
    mkdir -p {{user_desktop_dir}}

    echo "Installing desktop entries for COSMIC panel integration..."
    install -Dm0644 {{applet_full_desktop_file_src}} {{applet_full_desktop_file_dst}}
    install -Dm0644 {{applet_left_desktop_file_src}} {{applet_left_desktop_file_dst}}
    install -Dm0644 {{applet_right_desktop_file_src}} {{applet_right_desktop_file_dst}}

    # Install the applet icon
    mkdir -p {{user_icons_dir}}
    echo "Installing applet icon..."
    install -Dm0644 {{applet_icon_src}} {{applet_icon_dst}}

    # Restart cosmic panel for changes to take effect
    pkill -f cosmic-panel || true

    echo "Running cosmic-panel in this terminal..."
    cosmic-panel

# Run the cosmic applet for testing purposes with different sides
run-applet-left *args:
    env RUST_BACKTRACE=full RUST_LOG=debug,super_stt_shared=debug,warn cargo run --bin {{applet_name}} {{args}} -- --side left

run-applet-right *args:
    env RUST_BACKTRACE=full RUST_LOG=debug,super_stt_shared=debug,warn cargo run --bin {{applet_name}} {{args}} -- --side right

run-applet-full *args:
    env RUST_BACKTRACE=full RUST_LOG=debug,super_stt_shared=debug,warn cargo run --bin {{applet_name}} {{args}} -- --side full

# Build only the app
build-app *args:
    cargo build --release --bin {{app_name}} {{args}}

# Build only the daemon
build-daemon *args:
    cargo build --release --bin {{daemon_name}} {{args}}

# Build only the cosmic applet
build-applet:
    echo "🔧 Building COSMIC applet..."
    cargo build --release --bin {{applet_name}}

# Install the app (user-local installation)
install-app:
    #!/usr/bin/env bash
    # Build the app first
    echo "Building app..."
    if ! just build-app; then
        echo "❌ App build failed or was interrupted"
        exit 1
    fi

    # Check if binary exists
    if [ ! -f "{{app_src}}" ]; then
        echo "❌ App binary not found at {{app_src}}"
        exit 1
    fi

    echo "Installing Super STT app to {{app_dst}}"
    mkdir -p {{user_bin_dir}}
    install -m755 {{app_src}} {{app_dst}}

    # Install the desktop entry for application menu
    mkdir -p {{user_desktop_dir}}
    echo "Installing desktop entry..."
    install -Dm0644 {{app_desktop_file_src}} {{app_desktop_file_dst}}

    # Install the app icon
    mkdir -p {{user_icons_dir}}
    echo "Installing app icon..."
    install -Dm0644 {{app_icon_src}} {{app_icon_dst}}

    # Update desktop database
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database {{user_desktop_dir}} 2>/dev/null || true
    fi

    # Update icon cache
    if command -v gtk-update-icon-cache &> /dev/null; then
        gtk-update-icon-cache {{user_icons_dir}} 2>/dev/null || true
    fi

    echo "✓ Super STT app installed: {{app_dst}}"
    echo "✓ Desktop entry installed: {{app_desktop_file_dst}}"
    echo "✓ App icon installed: {{app_icon_dst}}"


# Install the cosmic applet (user-local installation)
install-applet:
    #!/usr/bin/env bash
    # Build the cosmic applet first
    echo "Building COSMIC applet..."
    if ! just build-applet; then
        echo "❌ COSMIC applet build failed or was interrupted"
        exit 1
    fi

    # Check if binary exists
    if [ ! -f "{{applet_src}}" ]; then
        echo "❌ COSMIC applet binary not found at {{applet_src}}"
        exit 1
    fi

    echo "Installing Super STT COSMIC applet..."
    mkdir -p {{user_bin_dir}}
    install -m755 {{applet_src}} {{applet_dst}}

    # Install the desktop entries for panel integration
    mkdir -p {{user_desktop_dir}}

    echo "Installing desktop entries for COSMIC panel integration..."
    install -Dm0644 {{applet_full_desktop_file_src}} {{applet_full_desktop_file_dst}}
    install -Dm0644 {{applet_left_desktop_file_src}} {{applet_left_desktop_file_dst}}
    install -Dm0644 {{applet_right_desktop_file_src}} {{applet_right_desktop_file_dst}}

    # Install the applet icon
    mkdir -p {{user_icons_dir}}
    echo "Installing applet icon..."
    install -Dm0644 {{applet_icon_src}} {{applet_icon_dst}}

    echo "✓ COSMIC applet installed: {{applet_dst}}"
    echo "✓ Desktop entries installed for panel integration:"
    echo "  - Super STT Applet (Full)"
    echo "  - Super STT Applet (Left Side)"
    echo "  - Super STT Applet (Right Side)"
    echo ""
    echo "🚀 Ready to use! The applet can now be added to your COSMIC panel through:"
    echo "-- COSMIC Settings > Desktop > Panel > Configure panel applets > Add Applet"

# Install the daemon (user installation)
# Usage: just install-daemon [--cuda|--cudnn] [--model MODEL]
install-daemon *args:
    #!/usr/bin/env bash
    # Build the daemon first
    echo "Building daemon..."

    # Extract model parameter
    model=""
    args_array=({{args}})
    for i in "${!args_array[@]}"; do
        if [[ "${args_array[$i]}" == "--model" ]]; then
            # Next argument is the model name
            if [[ $((i+1)) -lt ${#args_array[@]} ]]; then
                model="${args_array[$((i+1))]}"
            fi
            break
        elif [[ "${args_array[$i]}" == --model=* ]]; then
            model="${args_array[$i]#--model=}"
            break
        fi
    done

    if [[ "{{args}}" == *"--cudnn"* ]]; then
        if ! just build-daemon --features "cuda,cudnn"; then
            echo "❌ Daemon build failed or was interrupted"
            exit 1
        fi
    elif [[ "{{args}}" == *"--cuda"* ]]; then
        if ! just build-daemon --features "cuda"; then
            echo "❌ Daemon build failed or was interrupted"
            exit 1
        fi
    else
        if ! just build-daemon; then
            echo "❌ Daemon build failed or was interrupted"
            exit 1
        fi
    fi

    # Check if binary exists
    if [ ! -f "{{daemon_src}}" ]; then
        echo "❌ Daemon binary not found at {{daemon_src}}"
        exit 1
    fi

    echo "Installing Super STT daemon as user service..."

    # Setup stt group for secure socket access
    if [ -f "scripts/setup-stt-group.sh" ]; then
        echo "Setting up stt group for secure access..."
        bash scripts/setup-stt-group.sh || true
    fi

    # Install binary
    echo "Installing daemon binary to {{daemon_dst}}"
    mkdir -p {{user_bin_dir}}
    install -m755 {{daemon_src}} {{daemon_dst}}

    # Create user directories
    echo "Creating user directories..."
    mkdir -p {{run_dir}}
    mkdir -p {{log_dir}}
    mkdir -p {{user_systemd_dir}}

    # Create user systemd service file
    echo "Creating user systemd service file..."
    cat > {{service_dst}} << 'EOF'
    [Unit]
    Description=Super STT Daemon - Speech-to-text service
    Documentation=https://github.com/jorge-menjivar/super-stt
    After=pipewire.service pulseaudio.service graphical-session.target
    Requires=graphical-session.target

    [Service]
    Type=simple
    ExecStartPre=/bin/sh -c 'while [ ! -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]; do sleep 0.1; done'
    ExecStart={{wrapper_dst}} --socket {{run_dir}}/super-stt.sock
    ExecReload=/bin/kill -HUP $MAINPID
    Restart=always
    RestartSec=5
    TimeoutStartSec=60
    TimeoutStopSec=10

    # Environment
    Environment=CUDA_PATH=/opt/cuda
    Environment=LD_LIBRARY_PATH=/opt/cuda/lib64:/usr/local/cuda/lib64:/usr/lib/x86_64-linux-gnu

    # Output to user journal
    StandardOutput=journal
    StandardError=journal
    SyslogIdentifier=stt-daemon

    [Install]
    WantedBy=default.target
    EOF

    # Add model parameter to ExecStart if specified
    if [[ -n "$model" ]]; then
        echo "Configuring daemon to use model: $model"
        sed -i "s|--socket {{run_dir}}/super-stt.sock|--socket {{run_dir}}/super-stt.sock --model $model|" {{service_dst}}
    fi

    # Create wrapper script for automatic group switching
    echo "Creating wrapper script at {{wrapper_dst}}"
    echo '#!/bin/bash' > {{wrapper_dst}}
    echo '# Super STT Daemon Wrapper' >> {{wrapper_dst}}
    echo '# This wrapper automatically uses the stt group for socket access' >> {{wrapper_dst}}
    echo '' >> {{wrapper_dst}}
    echo 'exec sg stt -c "{{daemon_dst}} $*"' >> {{wrapper_dst}}
    chmod +x {{wrapper_dst}}

    # Update PATH in user's shell config
    shell_config="$HOME/.bashrc"
    if [[ "$SHELL" == *"zsh"* ]]; then
        shell_config="$HOME/.zshrc"
    fi

    if ! grep -q "{{user_bin_dir}}" "$shell_config" 2>/dev/null; then
        echo "Adding {{user_bin_dir}} to PATH in $shell_config"
        echo 'export PATH="{{user_bin_dir}}:$PATH"' >> "$shell_config"
        echo "⚠️  Restart your shell or run: source $shell_config"
    fi

    echo "✓ Super STT installed to {{app_dst}}"
    echo "✓ Wrapper script created at {{wrapper_dst}}
    echo "✓ Convenience shortcut 'stt' created"
    echo ""
    echo "🚀 Ready to use!"
    echo "-- stt record --write         # Record, transcribe, and type result"

    # Reload user systemd and enable service
    echo "Reloading user systemd..."
    systemctl --user daemon-reload

    echo "✓ Daemon installed successfully as user service!"
    echo ""
    systemctl --user start {{daemon_name}}
    systemctl --user enable {{daemon_name}}


# Install both client and daemon
# Usage: just install [--cuda|--cudnn] [--model MODEL]
install *args:
    #!/usr/bin/env bash
    # Check if cuDNN or CUDA is requested and call commands with the right args
    if ! just install-daemon {{args}}; then
        echo "❌ Daemon installation failed"
        exit 1
    fi

    if ! just install-app; then
        echo "❌ App installation failed"
        exit 1
    fi

# Install everything (daemon, app, and COSMIC applet)
# Usage: just install-all [--cuda|--cudnn] [--model MODEL]
install-all *args:
    #!/usr/bin/env bash
    if ! just install {{args}}; then
        echo "❌ Core installation failed"
        exit 1
    fi

    if ! just install-cosmic-all; then
        echo "❌ COSMIC applet installation failed"
        exit 1
    fi

    echo ""
    echo "🎉 Complete Super STT installation finished!"

# Uninstall the app
uninstall-app:
    #!/usr/bin/env bash
    echo "Uninstalling Super STT App..."
    rm -f {{app_dst}}
    rm -f {{app_desktop_file_dst}}
    rm -f {{app_icon_dst}}

    # Update desktop database
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database {{user_desktop_dir}} 2>/dev/null || true
    fi

    # Update icon cache
    if command -v gtk-update-icon-cache &> /dev/null; then
        gtk-update-icon-cache {{user_icons_dir}} 2>/dev/null || true
    fi

    echo "✓ Super STT App uninstalled"
    echo "✓ Desktop entry removed"
    echo "✓ App icon removed"

# Uninstall the cosmic applet
uninstall-applet:
    #!/usr/bin/env bash
    echo "Uninstalling Super STT COSMIC applet..."
    rm -f {{applet_dst}}
    rm -f {{applet_full_desktop_file_dst}}
    rm -f {{applet_left_desktop_file_dst}}
    rm -f {{applet_right_desktop_file_dst}}
    # Remove the applet icon
    rm -f {{applet_icon_dst}}
    echo "✓ COSMIC applet uninstalled"
    echo "✓ Desktop entries removed"
    echo "✓ Applet icon removed"

# Uninstall the daemon
uninstall-daemon:
    #!/usr/bin/env bash
    echo "Uninstalling Super STT daemon user service..."

    # Stop and disable user service
    systemctl --user stop {{daemon_name}} || true
    systemctl --user disable {{daemon_name}} || true

    # Remove service file
    rm -f {{service_dst}}

    # Remove binary
    rm -f {{daemon_dst}}

    rm -f "{{user_bin_dir}}/stt"

    # Remove directories (but preserve logs)
    rm -rf {{run_dir}}
    echo "Log directory {{log_dir}} preserved"

    # Reload user systemd
    systemctl --user daemon-reload

    echo "✓ Super STT Daemon user service uninstalled"

# Uninstall both app and daemon
uninstall: uninstall-daemon uninstall-app uninstall-applet

# Start the daemon user service
start-daemon:
    systemctl --user start {{daemon_name}}

# Stop the daemon user service
stop-daemon:
    systemctl --user stop {{daemon_name}}

# Enable daemon to start with user session
enable-daemon:
    systemctl --user enable {{daemon_name}}

# Disable daemon from starting with user session
disable-daemon:
    systemctl --user disable {{daemon_name}}

# Check daemon status
status-daemon:
    systemctl --user status {{daemon_name}}

# Show overall system status and test connectivity
status: status-daemon
    #!/usr/bin/env bash
    echo ""
    echo "🔍 Super STT System Status"
    echo "=========================="
    echo ""

    # Check if app is installed
    if command -v stt &> /dev/null; then
        echo "✅ App tools: Installed (stt command available)"
    elif [ -f "{{app_dst}}" ]; then
        echo "✅ Super STT App: Installed"
    else
        echo "❌ Super STT App: Not installed"
    fi

    # Check if daemon binary exists
    if [ -f "{{daemon_dst}}" ]; then
        echo "✅ Daemon binary: Installed"
    else
        echo "❌ Daemon binary: Not installed"
    fi

    # Check if cosmic applet is installed
    if [ -f "{{applet_dst}}" ]; then
        echo "✅ COSMIC applet: Installed"
    else
        echo "❌ COSMIC applet: Not installed"
    fi

# View daemon logs
logs-daemon:
    journalctl --user -u {{daemon_name}} -f

# View recent daemon logs
logs-daemon-recent:
    journalctl --user -u {{daemon_name}} -n 50

# Restart the daemon user service
restart-daemon:
    systemctl --user restart {{daemon_name}}

# Vendor dependencies locally
vendor:
    #!/usr/bin/env bash
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    echo '[env]' >> .cargo/config.toml
    if [ -n "${SOURCE_DATE_EPOCH}" ]
    then
        source_date="$(date -d "@${SOURCE_DATE_EPOCH}" "+%Y-%m-%d")"
        echo "VERGEN_GIT_COMMIT_DATE = \"${source_date}\"" >> .cargo/config.toml
    fi
    if [ -n "${SOURCE_GIT_HASH}" ]
    then
        echo "VERGEN_GIT_SHA = \"${SOURCE_GIT_HASH}\"" >> .cargo/config.toml
    fi
    tar pcf vendor.tar .cargo vendor
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar
