{
  description = ''
    VOICEVOX CLI for Apple Silicon Macs - Dynamic voice detection system

    Zero-configuration Japanese text-to-speech with automatic voice model discovery.
    Supports 26+ voice characters with dynamic detection and daemon-client architecture.

    Platform: Apple Silicon (aarch64-darwin) only

    License Information:
    - CLI Tool: MIT License + Apache License 2.0
    - VOICEVOX Core: MIT License (https://github.com/VOICEVOX/voicevox_core/blob/main/LICENSE)
    - ONNX Runtime: MIT License (https://github.com/microsoft/onnxruntime/blob/main/LICENSE)

    Usage Requirements:
    - Credit VOICEVOX when using generated audio
    - Follow individual voice library terms
    - See official repositories for complete license details
  '';

  nixConfig = {
    substituters = [
      "https://cache.nixos.org/"
      "https://voicevox-cli.cachix.org"
      "https://nix-community.cachix.org"
    ];

    trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
      "voicevox-cli.cachix.org-1:mgBVkErTVM4g1h08Bz86D73qhB4Jew/+JQ4iCjaPzj0="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];

    max-jobs = "auto";
    cores = 0;
    max-silent-time = 1800;
    timeout = 3600;
    connect-timeout = 5;
    download-attempts = 3;
    auto-optimise-store = true;
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
    }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" ] (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Read rust-toolchain.toml to ensure consistency
        rustToolchain = fenix.packages.${system}.stable;

        # Voice models and resources downloader
        voicevoxDownloader = pkgs.fetchurl {
          url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/download-osx-arm64";
          sha256 = "sha256-OL5Hpyd0Mc+77PzUhtIIFmHjRQqLVaiITuHICg1QBJU=";
        };


        # Simple resources for voicevox-download binary
        voicevoxResources = pkgs.stdenv.mkDerivation {
          name = "voicevox-resources";

          dontUnpack = true;

          installPhase = ''
            mkdir -p $out/bin
            cp ${voicevoxDownloader} $out/bin/voicevox-download
            chmod +x $out/bin/voicevox-download
          '';
        };

        packageMeta = with pkgs.lib; {
          description = "VOICEVOX CLI for Apple Silicon - Dynamic voice detection system";
          homepage = "https://github.com/usabarashi/voicevox-cli";
          license = with licenses; [
            mit
            asl20
          ];
          maintainers = [ "usabarashi" ];
          platforms = [ "aarch64-darwin" ];
        };

        voicevox-cli = pkgs.rustPlatform.buildRustPackage rec {
          pname = "voicevox-cli";
          version = "0.1.0";

          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter =
              path: type:
              let
                baseName = baseNameOf path;
              in
              !(
                # Exclude temporary directories
                (type == "directory" && pkgs.lib.hasSuffix "-extract" baseName)
                ||
                  # Exclude other temporary files
                  (type == "regular" && pkgs.lib.hasSuffix ".tar.gz" baseName && baseName != "Cargo.lock")
              );
          };

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "open_jtalk-0.1.25" = "sha256-sdUWHHY+eY3bWMGSPu/+0jGz1f4HMHq3D17Tzbwt0Nc=";
              "voicevox_core-0.0.0" = "sha256-QmnZSHB5tBxjVMEU5n0GVeV7W9c0/THXfsaN6Tu4R4Q=";
              "voicevox-ort-2.0.0-rc.4" = "sha256-ZGT3M4GkmSgAqXwuzBvnF+Zs37TPNfKXoEqTsqoT6R4=";
            };
          };

          doCheck = false;

          # Force offline mode to ensure reproducible builds
          CARGO_NET_OFFLINE = true;

          # Minimal pre-configure setup
          preConfigure = ''
            # Create a temporary HOME for build process
            export HOME=$PWD/build-home
            mkdir -p $HOME
          '';

          nativeBuildInputs = with pkgs; [
            # Use fenix-provided rust toolchain that matches rust-toolchain.toml
            rustToolchain.defaultToolchain

            # Build tools
            pkg-config
            cmake
            gnumake

            # Autotools (for dependencies)
            autoconf
            automake
            libtool

            # Version control (required by some build scripts)
            git
            cacert
          ];

          buildInputs = [];

          # Build-time environment variables
          preBuild = ''
            # Run full CI checks before build
            ${pkgs.bash}/bin/bash ${./scripts/ci.sh} --build-phase || exit 1

            # Git SSL configuration
            export GIT_SSL_CAINFO="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"

            # Simplified build configuration - no ONNX or OpenJTalk dependencies needed
            # Resources will be downloaded at runtime
          '';

          postInstall = ''
            # Install download utility
            cp ${voicevoxResources}/bin/voicevox-download $out/bin/
            
            # Install setup script (renamed from voicevox-setup-models.sh)
            install -m755 ${./scripts/voicevox-setup.sh} $out/bin/voicevox-setup
            
            # Install INSTRUCTIONS.md for MCP server
            install -m644 ${./INSTRUCTIONS.md} $out/bin/INSTRUCTIONS.md
            
            # Note: All resources (ONNX, dict, models) will be downloaded at runtime
          '';

          meta = packageMeta;
        };

        licenseAcceptor = pkgs.runCommand "voicevox-auto-setup" { } ''
          mkdir -p $out/bin
          substitute ${./scripts/voicevox-auto-setup.sh} $out/bin/voicevox-auto-setup \
            --replace "@@BASH_PATH@@" "${pkgs.bash}/bin/bash" \
            --replace "@@EXPECT_PATH@@" "${pkgs.expect}/bin/expect" \
            --replace "@@DOWNLOADER_PATH@@" "${voicevoxResources}/bin/voicevox-download"
          chmod +x $out/bin/voicevox-auto-setup
        '';

        # Common Serena environment setup script
        serenaEnvSetup = ''
          # Get the directory where this script is invoked from
          PROJECT_DIR="$(pwd)"

          # Create fake home directory structure in project
          export HOME="$PROJECT_DIR/.project-home"
          export XDG_DATA_HOME="$HOME/.local/share"
          export XDG_CACHE_HOME="$HOME/.cache"
          export UV_CACHE_DIR="$HOME/.cache/uv"
          export UV_TOOL_DIR="$HOME/.local/uv/tools"
          export CARGO_HOME="$PROJECT_DIR/.project-home/.cargo"

          # Create necessary directories
          mkdir -p "$HOME/.serena/logs"
          mkdir -p "$XDG_DATA_HOME/uv"
          mkdir -p "$XDG_CACHE_HOME"
        '';

        # Serena index creation wrapper
        serenaIndexWrapper = pkgs.writeShellScriptBin "serena-index" ''
          ${serenaEnvSetup}

          echo "Creating Serena index for project..."
          echo "HOME: $HOME"
          echo "Project: $PROJECT_DIR"

          # Run serena index command with all paths pointing to project directory
          exec ${pkgs.uv}/bin/uvx \
              --cache-dir "$UV_CACHE_DIR" \
              --from git+https://github.com/oraios/serena \
              serena project index
        '';

        # Serena MCP server wrapper with project-local paths
        serenaMcpWrapper = pkgs.writeShellScriptBin "serena-mcp-wrapper" ''
          ${serenaEnvSetup}

          echo "Starting Serena MCP server with project-local paths..."
          echo "HOME: $HOME"
          echo "Project: $PROJECT_DIR"

          # Run serena with all paths pointing to project directory
          exec ${pkgs.uv}/bin/uvx \
              --cache-dir "$UV_CACHE_DIR" \
              --from git+https://github.com/oraios/serena \
              serena start-mcp-server \
              --context ide-assistant \
              --enable-web-dashboard false \
              --project "$PROJECT_DIR"
        '';

        # Helper function to run uvx with Serena
        runSerenaCommand = ''
          exec ${pkgs.uv}/bin/uvx \
              --cache-dir "$UV_CACHE_DIR" \
              --from git+https://github.com/oraios/serena \
              serena "$@"
        '';

        # Serena memory management wrapper
        serenaMemoryWrapper = pkgs.writeShellScriptBin "serena-memory" ''
          set -euo pipefail

          ${serenaEnvSetup}

          # Handle memory commands
          case "''${1:-}" in
            write)
              if [ "$#" -lt 3 ]; then
                echo "Error: write command requires at least 2 arguments" >&2
                echo "Usage: serena-memory write <memory-name> <content>" >&2
                exit 1
              fi
              MEMORY_NAME="$2"
              echo "Writing memory: $MEMORY_NAME"
              # Shift twice to get all remaining args as content
              shift 2
              ${runSerenaCommand} memory write "$MEMORY_NAME" "$*"
              ;;
            read)
              if [ "$#" -lt 2 ]; then
                echo "Error: read command requires 1 argument" >&2
                echo "Usage: serena-memory read <memory-name>" >&2
                exit 1
              fi
              ${runSerenaCommand} memory read "$2"
              ;;
            list)
              ${runSerenaCommand} memory list
              ;;
            delete)
              if [ "$#" -lt 2 ]; then
                echo "Error: delete command requires 1 argument" >&2
                echo "Usage: serena-memory delete <memory-name>" >&2
                exit 1
              fi
              echo "Deleting memory: $2"
              ${runSerenaCommand} memory delete "$2"
              ;;
            *)
              echo "Serena Memory Management"
              echo ""
              echo "Usage:"
              echo "  serena-memory write <name> <content>  - Save a memory"
              echo "  serena-memory read <name>             - Read a memory"
              echo "  serena-memory list                    - List all memories"
              echo "  serena-memory delete <name>           - Delete a memory"
              echo ""
              echo "Example:"
              echo "  serena-memory write architecture 'This project uses daemon-client model'"
              exit 1
              ;;
          esac
        '';

      in
      {
        packages = {
          default = voicevox-cli;
          voicevox-cli = voicevox-cli;
          voicevox-say = voicevox-cli;
          voicevoxResources = voicevoxResources;
        };

        apps = {
          default = {
            type = "app";
            program = "${voicevox-cli}/bin/voicevox-say";
          };
          voicevox-say = {
            type = "app";
            program = "${voicevox-cli}/bin/voicevox-say";
          };
          voicevox-daemon = {
            type = "app";
            program = "${voicevox-cli}/bin/voicevox-daemon";
          };
          voicevox-mcp-server = {
            type = "app";
            program = "${voicevox-cli}/bin/voicevox-mcp-server";
          };

          # CI Task Runner - All checks in one command
          ci = {
            type = "app";
            program = toString (
              pkgs.writeShellScript "ci-runner" ''
                # Pass the project directory to the CI script
                export PROJECT_DIR="${toString ./.}"
                exec ${pkgs.bash}/bin/bash ${./scripts/ci.sh}
              ''
            );
          };
        };

        devShells.default = pkgs.mkShell {
          CARGO_HOME = "./.project-home/.cargo";

          buildInputs = with pkgs; [
            # Use fenix-provided rust toolchain that matches rust-toolchain.toml
            rustToolchain.defaultToolchain
            cargo-audit

            # Build tools
            pkg-config
            cmake

            # MCP
            uv
            serenaIndexWrapper
            serenaMcpWrapper
            serenaMemoryWrapper
          ];

          shellHook = ''
            # Create project-home directory for CARGO_HOME
            mkdir -p .project-home

            echo "VOICEVOX CLI Development Environment (Apple Silicon)"
            echo "Available commands:"
            echo "  cargo build --bin voicevox-say     - Build client"
            echo "  cargo build --bin voicevox-daemon  - Build daemon"
            echo "  cargo run --bin voicevox-say       - Run client"
            echo "  nix build                          - Build with Nix"
            echo "  nix run                            - Run voicevox-say directly"
            echo "  serena-index                       - Create Serena index for the project"
            echo "  serena-mcp-wrapper                 - Start Serena MCP server"
            echo "  serena-memory                      - Manage project memories"
            echo ""
            echo "Dynamic voice detection system - no hardcoded voice names"
          '';
        };

        lib = {
          mkVoicevoxCli = pkgs: voicevox-cli;
          getPackage = voicevox-cli;
          meta = packageMeta;
        };
      }
    )
    // {
      # Example usage for other projects:
      # {
      #   inputs.voicevox-cli.url = "github:usabarashi/voicevox-cli";
      #
      #   # In your system or home-manager configuration:
      #   environment.systemPackages = [
      #     voicevox-cli.packages.aarch64-darwin.default
      #   ];
      # }

      overlays.default = final: prev: {
        voicevox-cli = (self.packages.${final.system} or self.packages.aarch64-darwin).voicevox-cli;
        voicevox-say = final.voicevox-cli;
      };

      overlays.voicevox-cli = self.overlays.default;

      # Project metadata (not a standard flake output)
      # This information is available via:
      # - Individual package meta attributes
      # - README.md and LICENSE files
    };
}
