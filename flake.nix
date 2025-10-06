{
  description = "Super STT - High-performance speech-to-text service";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    let
      # Supported systems
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];

      # GPU compute capabilities to variants
      cudaVariants = {
        sm75 = "cuda-cudnn-sm75";  # Turing (RTX 20xx, T4)
        sm80 = "cuda-cudnn-sm80";  # Ampere datacenter (A100)
        sm86 = "cuda-cudnn-sm86";  # Ampere consumer (RTX 30xx)
        sm89 = "cuda-cudnn-sm89";  # Ada Lovelace (RTX 40xx)
        sm90 = "cuda-cudnn-sm90";  # Hopper (H100)
      };

      # Version to download (override via --override-input)
      version = "0.1.0";  # Update this to match latest release

    in
    flake-utils.lib.eachSystem supportedSystems (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Map Nix system to release architecture
        archMap = {
          x86_64-linux = "x86_64-unknown-linux-gnu";
          aarch64-linux = "aarch64-unknown-linux-gnu";
        };
        arch = archMap.${system};

        # Hash registry for all variants
        # Update these with: nix-prefetch-url <url>
        variantHashes = {
          # CPU variants (always available)
          cpu = {
            x86_64-linux = "sha256-CQCeLJR482C7nIypNhCnwa/c6UcmVlNNTkyf3rehrYo=";
            aarch64-linux = "sha256-h7mb+50vg4Dazr4av1lvvWHIV9EkeWMfCP2wyBNN1XM=";
          };
          # CUDA variants (may not exist for all releases)
          cuda-cudnn-sm75 = {
            x86_64-linux = "sha256-ExlviVI9pDf/y7kk55XEkUsUa+kL9lUzfoTclsdqa8o=";
            aarch64-linux = null;  # Not available for ARM
          };
          cuda-cudnn-sm80 = {
            x86_64-linux = "sha256-2D28ssEcKrUhJO3Ef9Dty957+NetHXQhIjzTcadUHNo=";
            aarch64-linux = null;
          };
          cuda-cudnn-sm86 = {
            x86_64-linux = "sha256-0VlV/Cb39yYyiDF2xP5pEqwbUbsMiFgnD4smylXg350=";
            aarch64-linux = null;
          };
          cuda-cudnn-sm89 = {
            x86_64-linux = "sha256-VBrC7moHBUGS0JL/RHk4kTJkyLgL252OxJ9VuHZoa2M=";
            aarch64-linux = null;
          };
          cuda-cudnn-sm90 = {
            x86_64-linux = "sha256-Ysk5irzf7+Pvurzqs1wuGSYwfIvVR9DWBJ/+n0JW/Hs=";
            aarch64-linux = null;
          };
        };

        # Determine fallback chain for a variant
        getFallbackChain = variant:
          if pkgs.lib.hasPrefix "cuda-cudnn-sm" variant then
            [ variant "cuda-cudnn-sm75" "cpu" ]
          else if pkgs.lib.hasPrefix "cuda-sm" variant then
            [ variant "cuda-sm75" "cpu" ]
          else
            [ variant ];

        # Build package for specific variant
        mkSuperSTT = { variant ? "cpu", pname ? "super-stt" }:
          let
            tarballName = "super-stt-${arch}-${variant}.tar.gz";
            url = "https://github.com/jorge-menjivar/super-stt/releases/download/v${version}/${tarballName}";

            # Get hash for this variant + system combo
            hash = variantHashes.${variant}.${system} or null;

            # If hash is null, this variant isn't available for this system
            src = if hash != null then
              pkgs.fetchurl {
                inherit url;
                sha256 = hash;
              }
            else
              throw ''
                Variant "${variant}" is not available for ${system}.
                Available variants: ${pkgs.lib.concatStringsSep ", " (builtins.attrNames (pkgs.lib.filterAttrs (_: v: v.${system} or null != null) variantHashes))}
                Fallback chain would be: ${pkgs.lib.concatStringsSep " -> " (getFallbackChain variant)}
              '';
          in
          pkgs.stdenv.mkDerivation {
            inherit pname version src;

            nativeBuildInputs = [
              pkgs.autoPatchelfHook
              pkgs.makeWrapper
            ];

            buildInputs = [
              pkgs.stdenv.cc.cc.lib
              pkgs.alsa-lib
              pkgs.libGL
              pkgs.libxkbcommon
              pkgs.wayland
              pkgs.vulkan-loader
              pkgs.xorg.libX11
              pkgs.xorg.libXcursor
              pkgs.xorg.libXi
              pkgs.xorg.libXrandr
            ];

            # Note: CUDA runtime libraries not needed as buildInputs since we're
            # downloading pre-built binaries. autoPatchelfHook will find CUDA libs
            # in the user's system if present.

            # Skip missing CUDA libraries during autopatchelf
            autoPatchelfIgnoreMissingDeps = [
              "libcuda.so.1"
              "libcurand.so.10"
              "libcublas.so.12"
              "libcublasLt.so.12"
              "libcudnn.so.9"
              "libcudnn_ops.so.9"
              "libcudnn_cnn.so.9"
            ];

            sourceRoot = ".";

            installPhase = ''
              runHook preInstall

              # Create directories
              mkdir -p $out/bin
              mkdir -p $out/share/applications
              mkdir -p $out/share/icons/hicolor/scalable/apps
              mkdir -p $out/share/metainfo
              mkdir -p $out/lib/systemd/user

              # Install daemon binary
              install -m755 super-stt $out/bin/super-stt

              # Install desktop app if present
              if [ -f super-stt-app ]; then
                install -m755 super-stt-app $out/bin/super-stt-app
                install -m644 resources/super-stt-app.desktop $out/share/applications/
                install -m644 resources/icons/hicolor/scalable/apps/super-stt-app.svg \
                  $out/share/icons/hicolor/scalable/apps/
                install -m644 resources/super-stt-app.metainfo.xml $out/share/metainfo/
              fi

              # Install COSMIC applet if present
              if [ -f super-stt-cosmic-applet ]; then
                install -m755 super-stt-cosmic-applet $out/bin/super-stt-cosmic-applet
                cp resources/super-stt-cosmic-applet-*.desktop $out/share/applications/ || true
                install -m644 resources/icons/hicolor/scalable/apps/super-stt-cosmic-applet.svg \
                  $out/share/icons/hicolor/scalable/apps/ || true
              fi

              # Install systemd service
              if [ -f systemd/super-stt.service ]; then
                install -m644 systemd/super-stt.service $out/lib/systemd/user/
              fi

              # Create stt wrapper script
              makeWrapper $out/bin/super-stt $out/bin/stt \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.coreutils ]}

              # Wrap GUI applications with Wayland libraries
              if [ -f $out/bin/super-stt-app ]; then
                wrapProgram $out/bin/super-stt-app \
                  --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
                    pkgs.wayland
                    pkgs.libxkbcommon
                    pkgs.vulkan-loader
                    pkgs.libGL
                  ]}
              fi

              if [ -f $out/bin/super-stt-cosmic-applet ]; then
                wrapProgram $out/bin/super-stt-cosmic-applet \
                  --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
                    pkgs.wayland
                    pkgs.libxkbcommon
                    pkgs.vulkan-loader
                    pkgs.libGL
                  ]}
              fi

              runHook postInstall
            '';

            meta = with pkgs.lib; {
              description = "High-performance speech-to-text service";
              homepage = "https://github.com/jorge-menjivar/super-stt";
              license = licenses.mit;
              platforms = [ system ];
              mainProgram = "stt";
            };
          };

      in
      {
        packages = {
          # CPU variant (default)
          default = mkSuperSTT { };
          super-stt-cpu = mkSuperSTT { };

          # CUDA variants
          super-stt-cuda-sm75 = mkSuperSTT { variant = "cuda-cudnn-sm75"; };
          super-stt-cuda-sm80 = mkSuperSTT { variant = "cuda-cudnn-sm80"; };
          super-stt-cuda-sm86 = mkSuperSTT { variant = "cuda-cudnn-sm86"; };
          super-stt-cuda-sm89 = mkSuperSTT { variant = "cuda-cudnn-sm89"; };
          super-stt-cuda-sm90 = mkSuperSTT { variant = "cuda-cudnn-sm90"; };
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
          ];
        };

        # Apps for easy running
        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/stt";
        };
      }
    ) // {
      # Home Manager module
      homeManagerModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.super-stt;
          inherit (lib) mkEnableOption mkOption mkIf types;

          # Try to select package with fallback
          selectPackageWithFallback = variant:
            let
              # Determine fallback chain (same logic as in packages)
              getFallbackChain = v:
                if lib.hasPrefix "cuda-cudnn-sm" v then
                  [ v "cuda-cudnn-sm75" "cpu" ]
                else if lib.hasPrefix "cuda-sm" v then
                  [ v "cuda-sm75" "cpu" ]
                else
                  [ v ];

              fallbackChain = getFallbackChain variant;

              # Try each variant in order
              tryVariants = variants:
                if variants == [] then
                  # Ultimate fallback: CPU
                  self.packages.${pkgs.system}.default
                else
                  let
                    currentVariant = lib.head variants;
                    packageName = "super-stt-${currentVariant}";
                    result = builtins.tryEval (self.packages.${pkgs.system}.${packageName});
                  in
                  if result.success then
                    result.value
                  else
                    tryVariants (lib.tail variants);
            in
            tryVariants fallbackChain;

          # Auto-detect variant based on system
          autoVariant =
            if config.hardware.nvidia.package or null != null then
              # TODO: Detect actual compute capability from nvidia-smi
              # For now, default to widely compatible SM 8.6
              "cuda-cudnn-sm86"
            else
              "cpu";

          selectedPackage =
            if cfg.variant == "auto" then
              selectPackageWithFallback autoVariant
            else if cfg.package == null then
              selectPackageWithFallback cfg.variant
            else
              cfg.package;

        in
        {
          options.services.super-stt = {
            enable = mkEnableOption "Super STT speech-to-text service";

            variant = mkOption {
              type = types.str;
              default = "auto";
              description = "Variant to install (auto, cpu, cuda-sm75, cuda-sm80, cuda-sm86, cuda-sm89, cuda-sm90)";
            };

            package = mkOption {
              type = types.nullOr types.package;
              default = null;
              description = ''
                Super STT package to use. If null, automatically selected based on variant with fallback support.
                Fallback chain: requested variant -> SM75 (if CUDA) -> CPU
              '';
            };

            enableApp = mkOption {
              type = types.bool;
              default = true;
              description = "Install desktop application";
            };

            enableApplet = mkOption {
              type = types.bool;
              default = false;
              description = "Install COSMIC applet (requires COSMIC desktop)";
            };

            autoStart = mkOption {
              type = types.bool;
              default = true;
              description = "Start daemon automatically";
            };

            # Daemon configuration options
            model = mkOption {
              type = types.nullOr (types.enum [
                "whisper-tiny"
                "whisper-base"
                "whisper-small"
                "whisper-medium"
                "whisper-large"
                "whisper-large-v3"
                "whisper-large-v3-turbo"
              ]);
              default = null;
              description = "STT model to use (null = use saved config)";
            };

            device = mkOption {
              type = types.enum [ "cuda" "cpu" ];
              default = "cuda";
              description = "Device to use: cuda (GPU with CPU fallback) or cpu (force CPU)";
            };

            socket = mkOption {
              type = types.nullOr types.path;
              default = null;
              description = "Custom socket path (null = use default runtime dir)";
            };

            udpPort = mkOption {
              type = types.port;
              default = 8765;
              description = "UDP port for audio streaming";
            };

            audioTheme = mkOption {
              type = types.enum [
                "classic"
                "gentle"
                "minimal"
                "scifi"
                "musical"
                "nature"
                "retro"
                "silent"
              ];
              default = "classic";
              description = "Audio feedback theme";
            };

            verbose = mkOption {
              type = types.bool;
              default = false;
              description = "Enable verbose logging";
            };
          };

          config = mkIf cfg.enable {
            home.packages = [ selectedPackage ];

            # Systemd user service
            systemd.user.services.super-stt =
              let
                # Build command with flags
                daemonCmd = lib.concatStringsSep " " (
                  [ "${selectedPackage}/bin/super-stt" ]
                  ++ lib.optional (cfg.model != null) "--model ${cfg.model}"
                  ++ [ "--device ${cfg.device}" ]
                  ++ lib.optional (cfg.socket != null) "--socket ${cfg.socket}"
                  ++ [ "--udp-port ${toString cfg.udpPort}" ]
                  ++ [ "--audio-theme ${cfg.audioTheme}" ]
                  ++ lib.optional cfg.verbose "--verbose"
                );
              in
              {
                Unit = {
                  Description = "Super STT Daemon";
                  After = [ "graphical-session.target" ];
                };

                Service = {
                  Type = "simple";
                  ExecStart = daemonCmd;
                  Restart = "on-failure";
                  RestartSec = "5s";

                  # Security hardening
                  PrivateTmp = true;
                  ProtectSystem = "strict";
                  NoNewPrivileges = true;
                  RuntimeDirectory = "stt";
                  StateDirectory = "stt";
                  CacheDirectory = "stt";
                  LogsDirectory = "stt";

                  # Wayland environment for keyboard injection
                  PassEnvironment = [ "WAYLAND_DISPLAY" "XDG_RUNTIME_DIR" ];
                };

                Install = mkIf cfg.autoStart {
                  WantedBy = [ "graphical-session.target" ];
                };
              };

            # XDG configuration
            xdg = {
              # Configuration files
              configFile = {
                # Super STT configuration
                "super-stt/config.toml".text = ''
                  # Super STT Configuration
                  [daemon]
                  socket_path = "$XDG_RUNTIME_DIR/stt/daemon.sock"

                  [logging]
                  level = "info"
                  directory = "$HOME/.local/share/stt/logs"
                '';
              } // lib.optionalAttrs cfg.enableApplet {
                # COSMIC keyboard shortcut (only if applet enabled)
                "cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom".text = ''
                  {
                      (
                          modifiers: [
                              Super,
                          ],
                          key: "space",
                          description: Some("Super STT"),
                      ): Spawn("${selectedPackage}/bin/stt record --write"),
                  }
                '';
              };

              # Desktop entries (if app enabled)
              desktopEntries = mkIf cfg.enableApp {
                super-stt-app = {
                  name = "Super STT";
                  genericName = "Speech to Text";
                  comment = "High-performance speech-to-text application";
                  exec = "${selectedPackage}/bin/super-stt-app";
                  icon = "super-stt-app";
                  terminal = false;
                  categories = [ "Utility" "Audio" "AudioVideo" ];
                  startupNotify = true;
                  settings = {
                    Keywords = "speech;transcription;stt;dictation;";
                  };
                };
              };

              # Data files
              dataFile = {
                "stt/.keep".text = "";
              } // lib.optionalAttrs cfg.enableApp {
                # Install icon for desktop app
                "icons/hicolor/scalable/apps/super-stt-app.svg".source =
                  "${selectedPackage}/share/icons/hicolor/scalable/apps/super-stt-app.svg";
              };
            };

            # Update desktop database and icon cache
            home.activation.super-stt = lib.hm.dag.entryAfter ["writeBoundary"] ''
              $DRY_RUN_CMD ${pkgs.desktop-file-utils}/bin/update-desktop-database \
                $HOME/.local/share/applications

              if [ -x ${pkgs.gtk3}/bin/gtk-update-icon-cache ]; then
                $DRY_RUN_CMD ${pkgs.gtk3}/bin/gtk-update-icon-cache -f -t \
                  $HOME/.local/share/icons/hicolor
              fi
            '';
          };
        };

      # NixOS module (system-wide installation)
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.services.super-stt;
          inherit (lib) mkEnableOption mkOption mkIf types;
        in
        {
          options.services.super-stt = {
            enable = mkEnableOption "Super STT system-wide service";

            variant = mkOption {
              type = types.str;
              default = "cpu";
              description = "Variant to install";
            };

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}."super-stt-${cfg.variant}";
              description = "Super STT package to use";
            };

            # Daemon configuration options
            model = mkOption {
              type = types.nullOr (types.enum [
                "whisper-tiny"
                "whisper-base"
                "whisper-small"
                "whisper-medium"
                "whisper-large"
                "whisper-large-v3"
                "whisper-large-v3-turbo"
              ]);
              default = null;
              description = "STT model to use (null = use saved config)";
            };

            device = mkOption {
              type = types.enum [ "cuda" "cpu" ];
              default = "cuda";
              description = "Device to use: cuda (GPU with CPU fallback) or cpu (force CPU)";
            };

            socket = mkOption {
              type = types.nullOr types.path;
              default = null;
              description = "Custom socket path (null = use default runtime dir)";
            };

            udpPort = mkOption {
              type = types.port;
              default = 8765;
              description = "UDP port for audio streaming";
            };

            audioTheme = mkOption {
              type = types.enum [
                "classic"
                "gentle"
                "minimal"
                "scifi"
                "musical"
                "nature"
                "retro"
                "silent"
              ];
              default = "classic";
              description = "Audio feedback theme";
            };

            verbose = mkOption {
              type = types.bool;
              default = false;
              description = "Enable verbose logging";
            };
          };

          config = mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];

            # Create stt group
            users.groups.stt = { };

            # System-wide socket activation
            systemd.user.services.super-stt =
              let
                # Build command with flags
                daemonCmd = lib.concatStringsSep " " (
                  [ "${cfg.package}/bin/super-stt" ]
                  ++ lib.optional (cfg.model != null) "--model ${cfg.model}"
                  ++ [ "--device ${cfg.device}" ]
                  ++ lib.optional (cfg.socket != null) "--socket ${cfg.socket}"
                  ++ [ "--udp-port ${toString cfg.udpPort}" ]
                  ++ [ "--audio-theme ${cfg.audioTheme}" ]
                  ++ lib.optional cfg.verbose "--verbose"
                );
              in
              {
                description = "Super STT Daemon";
                after = [ "graphical-session.target" ];
                serviceConfig = {
                  Type = "simple";
                  ExecStart = daemonCmd;
                  Restart = "on-failure";
                  RestartSec = "5s";
                  SupplementaryGroups = [ "stt" ];

                  # Security hardening
                  PrivateTmp = true;
                  ProtectSystem = "strict";
                  NoNewPrivileges = true;
                  RuntimeDirectory = "stt";
                  StateDirectory = "stt";
                  CacheDirectory = "stt";
                  LogsDirectory = "stt";

                  # Wayland environment for keyboard injection
                  PassEnvironment = [ "WAYLAND_DISPLAY" "XDG_RUNTIME_DIR" ];
                };
              };
          };
        };
    };
}
