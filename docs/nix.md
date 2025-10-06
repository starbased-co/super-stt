# Nix Installation Guide

This document explains how to install and manage Super STT using Nix flakes.

## Quick Start

### Try Without Installing

```bash
# CPU variant
nix run github:jorge-menjivar/super-stt

# CUDA variant (SM 8.6 - RTX 30xx series)
nix run github:jorge-menjivar/super-stt#super-stt-cuda-sm86
```

### Install to Profile

```bash
# Install CPU variant
nix profile install github:jorge-menjivar/super-stt

# Install specific CUDA variant
nix profile install github:jorge-menjivar/super-stt#super-stt-cuda-sm86

# Run
stt record --write
```

### Uninstall from Profile

```bash
# List installed packages
nix profile list

# Remove by index (e.g., 5)
nix profile remove 5
```

## Home Manager Integration

### Installation

Add to your `home.nix` or `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    super-stt.url = "github:jorge-menjivar/super-stt";
    home-manager.url = "github:nix-community/home-manager";
  };

  outputs = { nixpkgs, super-stt, home-manager, ... }: {
    homeConfigurations.youruser = home-manager.lib.homeManagerConfiguration {
      pkgs = nixpkgs.legacyPackages.x86_64-linux;

      modules = [
        super-stt.homeManagerModules.default
        {
          services.super-stt = {
            enable = true;
            variant = "auto";  # auto-detect, or specify: cpu, cuda-sm75, cuda-sm86, etc.
            enableApp = true;
            enableApplet = false;  # Set true for COSMIC desktop
            autoStart = true;
          };
        }
      ];
    };
  };
}
```

### Configuration Options

```nix
services.super-stt = {
  # Enable the service
  enable = true;

  # Variant selection
  variant = "auto";  # Options: auto, cpu, cuda-sm75, cuda-sm80, cuda-sm86, cuda-sm89, cuda-sm90

  # Override package (advanced)
  # package = pkgs.super-stt-cuda-sm86;

  # Install desktop application
  enableApp = true;

  # Install COSMIC applet (requires COSMIC desktop)
  enableApplet = false;

  # Auto-start daemon with graphical session
  autoStart = true;
};
```

### Uninstallation (Home Manager)

Remove or disable the service in your configuration:

```nix
{
  services.super-stt.enable = false;
}
```

Then rebuild:

```bash
home-manager switch
```

To remove all data and configuration:

```bash
rm -rf ~/.config/super-stt
rm -rf ~/.local/share/stt
```

## NixOS System-Wide Installation

### Configuration

Add to `/etc/nixos/configuration.nix`:

```nix
{
  inputs = {
    super-stt.url = "github:jorge-menjivar/super-stt";
  };

  outputs = { nixpkgs, super-stt, ... }: {
    nixosConfigurations.yourhostname = nixpkgs.lib.nixosSystem {
      modules = [
        super-stt.nixosModules.default
        {
          services.super-stt = {
            enable = true;
            variant = "cuda-sm86";
          };

          # Add users to stt group
          users.users.youruser.extraGroups = [ "stt" ];
        }
      ];
    };
  };
}
```

### Uninstallation (NixOS)

```nix
{
  services.super-stt.enable = false;
}
```

Rebuild:

```bash
sudo nixos-rebuild switch
```

## Available Variants

| Variant | Description | Hardware |
|---------|-------------|----------|
| `cpu` | CPU-only (default) | All systems |
| `cuda-sm75` | CUDA + cuDNN SM 7.5 | Turing (RTX 20xx, T4) |
| `cuda-sm80` | CUDA + cuDNN SM 8.0 | Ampere datacenter (A100, A30) |
| `cuda-sm86` | CUDA + cuDNN SM 8.6 | Ampere consumer (RTX 30xx, A40) |
| `cuda-sm89` | CUDA + cuDNN SM 8.9 | Ada Lovelace (RTX 40xx, L4) |
| `cuda-sm90` | CUDA + cuDNN SM 9.0 | Hopper (H100, H200) |

## XDG Integration

The flake automatically handles:

- **Desktop files**: Installed to `~/.local/share/applications/`
- **Icons**: Installed to `~/.local/share/icons/hicolor/`
- **Configuration**: `~/.config/super-stt/config.toml`
- **Data**: `~/.local/share/stt/`
- **Systemd service**: `~/.config/systemd/user/super-stt.service`
- **COSMIC shortcuts**: `~/.config/cosmic/.../custom` (if enabled)

Desktop database and icon cache are automatically updated after installation.

## Development

### Building from Source

```bash
git clone https://github.com/jorge-menjivar/super-stt
cd super-stt

# Build CPU variant
nix build

# Build CUDA variant
nix build .#super-stt-cuda-sm86

# Development shell
nix develop
```

### Updating Hashes

When updating to a new release, update `sha256` hashes:

```bash
# Get hash for CPU variant
nix-prefetch-url https://github.com/jorge-menjivar/super-stt/releases/download/v0.1.0/super-stt-x86_64-unknown-linux-gnu-cpu.tar.gz

# Update flake.nix with the hash
```

## Troubleshooting

### Service Not Starting

Check logs:

```bash
journalctl --user -u super-stt -f
```

### CUDA Issues

Verify CUDA availability:

```bash
nvidia-smi

# Check installed variant
nix-store -q --references $(which stt) | grep cuda
```

### Permission Issues

Ensure you're in the `stt` group:

```bash
groups | grep stt

# If not, log out and back in
```

### Clean Uninstall

For Home Manager:

```bash
# Stop service
systemctl --user stop super-stt

# Remove from profile
nix profile remove <index>

# Clean data
rm -rf ~/.config/super-stt ~/.local/share/stt
```

For direct profile installation:

```bash
nix profile list | grep super-stt
nix profile remove <index>
```

## Comparison: Nix vs Traditional Install

| Feature | Nix | Traditional (`install.sh`) |
|---------|-----|---------------------------|
| **Rollback** | ✅ Automatic | ❌ Manual |
| **Reproducible** | ✅ Yes | ⚠️  Depends on release availability |
| **Multi-version** | ✅ Yes | ❌ No |
| **Dependencies** | ✅ Automatic | ⚠️  System-dependent |
| **Uninstall** | ✅ Clean | ⚠️  Manual (`uninstall.sh`) |
| **Root required** | ❌ No | ⚠️  For group creation |
| **Learning curve** | ⚠️  Moderate | ✅ Low |

## Additional Resources

- [Nix Flakes Manual](https://nixos.org/manual/nix/stable/command-ref/new-cli/nix3-flake.html)
- [Home Manager Manual](https://nix-community.github.io/home-manager/)
- [NixOS Manual](https://nixos.org/manual/nixos/stable/)
