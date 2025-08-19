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

        # Read rust-toolchain.toml for version consistency
        # Parse the TOML file to get the channel version
        rustChannelVersion = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain.channel;
        
        rustToolchain = fenix.packages.${system}.toolchainOf {
          channel = rustChannelVersion;
          sha256 = "sha256-+9FmLhAOezBZCOziO0Qct1NOrfpjNsXxc/8I0c7BdKE=";
        };

        # ONNX Runtime - will be bundled in the same package
        onnxRuntime = pkgs.fetchurl {
          url = "https://github.com/VOICEVOX/onnxruntime-builder/releases/download/voicevox_onnxruntime-1.17.3/voicevox_onnxruntime-osx-arm64-1.17.3.tgz";
          sha256 = "sha256-ltfqGSigoVSFSS03YhOH31D0CnkuKmgX1N9z7NGFcfI=";
        };

        # Voice models downloader (still needed for runtime)
        voicevoxDownloader = pkgs.fetchurl {
          url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.1/download-osx-arm64";
          sha256 = "sha256-SrsBlDNmSXdlRcblUWKQ4TmjRqcbmgbCQAwyoizqoLw=";
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
                (type == "directory" && pkgs.lib.hasSuffix "-extract" baseName)
                || (type == "regular" && pkgs.lib.hasSuffix ".tar.gz" baseName && baseName != "Cargo.lock")
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

          # Allow network access for voicevox-ort to download ONNX Runtime
          # This is needed since we're building from source
          CARGO_NET_OFFLINE = false;

          # Pre-configure phase to setup build environment
          preConfigure = ''
            export HOME=$PWD/build-home
            mkdir -p $HOME
            
            # Extract ONNX Runtime for build linking
            mkdir -p $HOME/onnxruntime
            cd $HOME/onnxruntime
            ${pkgs.gnutar}/bin/tar -xzf ${onnxRuntime}
            
            # Find and move the extracted directory contents
            ONNX_DIR=$(find . -maxdepth 1 -name "voicevox_onnxruntime*" -type d | head -1)
            if [ -n "$ONNX_DIR" ]; then
              mv "$ONNX_DIR"/* .
              rmdir "$ONNX_DIR"
            fi
            
            # Create symlink for build
            if [ -f "lib/libvoicevox_onnxruntime.dylib" ] && [ ! -f "lib/libonnxruntime.dylib" ]; then
              ln -s libvoicevox_onnxruntime.dylib lib/libonnxruntime.dylib
            fi
            
            cd - > /dev/null
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

          buildInputs = [ ];

          # Build-time environment variables
          preBuild = ''
            # Run full CI checks before build
            ${pkgs.bash}/bin/bash ${./scripts/ci.sh} --build-phase || exit 1

            # Git SSL configuration
            export GIT_SSL_CAINFO="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"

            # OpenJTalk configuration
            # Create dummy dictionary for build time
            mkdir -p $PWD/dummy_dict
            export OPENJTALK_DICT_PATH="$PWD/dummy_dict"

            # ONNX Runtime configuration
            # Link against ONNX Runtime for build
            export ORT_STRATEGY="system"
            export ORT_LIB_LOCATION="$HOME/onnxruntime/lib"
            
            # Set library paths for linking
            export LIBRARY_PATH="$ORT_LIB_LOCATION:$LIBRARY_PATH"
            export DYLD_LIBRARY_PATH="$ORT_LIB_LOCATION:$DYLD_LIBRARY_PATH"

            # CMake configuration
            export CMAKE_BUILD_PARALLEL_LEVEL="4"
          '';

          postInstall = ''
            # Install downloader for runtime model downloads
            cp ${voicevoxDownloader} $out/bin/voicevox-download
            chmod +x $out/bin/voicevox-download
            install -m755 ${./scripts/voicevox-setup-models.sh} $out/bin/voicevox-setup-models
            
            # Bundle ONNX Runtime libraries in the same package
            mkdir -p $out/lib
            cp $HOME/onnxruntime/lib/*.dylib $out/lib/
            
            # Fix rpath to use bundled libraries
            for bin in voicevox-say voicevox-daemon voicevox-mcp-server; do
              if [ -f "$out/bin/$bin" ]; then
                # Add @loader_path/../lib to rpath
                ${pkgs.cctools}/bin/install_name_tool -add_rpath @loader_path/../lib $out/bin/$bin || true
                
                # Update library references to use @rpath
                ${pkgs.cctools}/bin/install_name_tool -change @rpath/libonnxruntime.1.17.3.dylib @rpath/libonnxruntime.dylib $out/bin/$bin || true
                ${pkgs.cctools}/bin/install_name_tool -change @rpath/libvoicevox_onnxruntime.1.17.3.dylib @rpath/libvoicevox_onnxruntime.dylib $out/bin/$bin || true
              fi
            done
            
            # Create version-agnostic symlinks
            cd $out/lib
            ln -sf libvoicevox_onnxruntime.1.17.3.dylib libvoicevox_onnxruntime.dylib || true
            ln -sf libvoicevox_onnxruntime.dylib libonnxruntime.dylib || true
            ln -sf libvoicevox_onnxruntime.1.17.3.dylib libonnxruntime.1.17.3.dylib || true
          '';

          meta = packageMeta;
        };

        licenseAcceptor = pkgs.runCommand "voicevox-auto-setup" { } ''
          mkdir -p $out/bin
          substitute ${./scripts/voicevox-auto-setup.sh} $out/bin/voicevox-auto-setup \
            --replace "@@BASH_PATH@@" "${pkgs.bash}/bin/bash" \
            --replace "@@EXPECT_PATH@@" "${pkgs.expect}/bin/expect" \
            --replace "@@DOWNLOADER_PATH@@" "${voicevoxDownloader}"
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
      rec {
        packages = rec {
          default = voicevox-cli;
          inherit voicevox-cli;
          voicevox-say = voicevox-cli;

          # Release archive package that creates tar.gz from voicevox-cli package
          release = pkgs.stdenv.mkDerivation {
            pname = "voicevox-cli-release-archive";
            version = "0.1.0";

            buildInputs = [ voicevox-cli ];
            nativeBuildInputs = with pkgs; [
              coreutils
              gnutar
              gzip
            ];

            phases = [ "installPhase" ];

            installPhase = ''
              mkdir -p $out
              
              # Create temporary directory with proper structure
              mkdir -p $out/tmp/voicevox-cli
              cp -r ${voicevox-cli}/bin $out/tmp/voicevox-cli/
              cp -r ${voicevox-cli}/lib $out/tmp/voicevox-cli/ || echo "No lib directory"
              
              # Create archive with both bin and lib
              cd $out
              ${pkgs.gnutar}/bin/tar -czf voicevox-cli-release-aarch64-darwin.tar.gz -C tmp voicevox-cli
              
              # Clean up temp directory
              rm -rf $out/tmp
              
              # Create SHA256 checksum
              ${pkgs.coreutils}/bin/sha256sum voicevox-cli-release-aarch64-darwin.tar.gz > voicevox-cli-release-aarch64-darwin.tar.gz.sha256
              
              echo "Release archive created: $out/voicevox-cli-release-aarch64-darwin.tar.gz"
              echo "Archive contains bin/ and lib/ directories"
            '';
          };
        };

        apps = {
          default = {
            type = "app";
            program = "${packages.voicevox-cli}/bin/voicevox-say";
          };
          voicevox-say = {
            type = "app";
            program = "${packages.voicevox-cli}/bin/voicevox-say";
          };
          voicevox-daemon = {
            type = "app";
            program = "${packages.voicevox-cli}/bin/voicevox-daemon";
          };
          voicevox-mcp-server = {
            type = "app";
            program = "${packages.voicevox-cli}/bin/voicevox-mcp-server";
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

      overlays.default =
        final: prev:
        let
          pkg = (self.packages.${final.system} or self.packages.aarch64-darwin).voicevox-cli;
        in
        {
          voicevox-cli = pkg;
          voicevox-say = pkg;
        };

      overlays.voicevox-cli = self.overlays.default;

      # Project metadata (not a standard flake output)
      # This information is available via:
      # - Individual package meta attributes
      # - README.md and LICENSE files
    };
}
