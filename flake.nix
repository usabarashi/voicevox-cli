{
  description = ''
    VOICEVOX CLI for Apple Silicon Macs - Dynamic voice detection system

    Zero-configuration Japanese text-to-speech with automatic voice model discovery.
    Supports 26+ voice characters with dynamic detection and daemon-client architecture.

    Platform: Apple Silicon (aarch64-darwin) only
    
    License Information:
    - CLI Tool: MIT License + Apache License 2.0
    - VOICEVOX Core: MIT License (Copyright 2021 Hiroshiba Kazuyuki)
    - ONNX Runtime: Custom Terms (Commercial use allowed, Credit required)

    Usage Requirements:
    - Credit VOICEVOX when using generated audio
    - Follow individual voice library terms
    - See voicevox_core/onnxruntime/TERMS.txt for details
  '';

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" ]
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          # VOICEVOX downloader for runtime setup
          # Note: Libraries and models are downloaded at runtime by voicevox-download

          voicevoxDownloader = pkgs.fetchurl {
            url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/download-osx-arm64";
            sha256 = "sha256-OL5Hpyd0Mc+77PzUhtIIFmHjRQqLVaiITuHICg1QBJU=";
          };

          # Minimal setup for voicevox-download
          voicevoxSetup = pkgs.stdenv.mkDerivation {
            name = "voicevox-download-setup";
            
            buildCommand = ''
              mkdir -p $out/bin
              
              # Install VOICEVOX downloader for runtime use
              echo "Installing VOICEVOX downloader..."
              cp ${voicevoxDownloader} $out/bin/voicevox-download
              chmod +x $out/bin/voicevox-download
              
              echo "VOICEVOX downloader ready (all libraries downloaded at runtime)"
            '';
          };

          # Centralized meta information
          packageMeta = with pkgs.lib; {
            description = "VOICEVOX CLI for Apple Silicon - Dynamic voice detection system";
            homepage = "https://github.com/usabarashi/voicevox-cli";
            license = with licenses; [ mit asl20 ];
            maintainers = [ "usabarashi" ];
            platforms = [ "aarch64-darwin" ];
          };

          # Package definition using voicevoxResources
          voicevox-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "voicevox-cli";
            version = "0.1.0";

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "open_jtalk-0.1.25" = "sha256-sdUWHHY+eY3bWMGSPu/+0jGz1f4HMHq3D17Tzbwt0Nc=";
                "voicevox_core-0.0.0" = "sha256-Ud/D3k8J8wOJiNiQ1bWi2RTS+Ix+ImqNEiyMHcCud78=";
                "voicevox-ort-2.0.0-rc.4" = "sha256-ZGT3M4GkmSgAqXwuzBvnF+Zs37TPNfKXoEqTsqoT6R4=";
              };
            };

            # Skip tests since they require VOICEVOX runtime libraries
            doCheck = false;
            
            # Disable network access during build to force system library usage
            __noChroot = false;

            nativeBuildInputs = with pkgs; [
              pkg-config
              cmake
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk_11_0.frameworks.AudioUnit
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreAudio
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreServices
            ];

            buildInputs = with pkgs; [
              onnxruntime
              open-jtalk
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk_11_0.frameworks.AudioUnit
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreAudio
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreServices
            ];

            # Note: All libraries downloaded at runtime by voicevox-download
            preBuild = ''
              echo "Build ready - using system libraries"
              echo "ONNX Runtime location: ${pkgs.onnxruntime}/lib"
              echo "OpenJTalk location: ${pkgs.open-jtalk}/lib"
              
              # ONNX Runtime configuration
              export ORT_LIB_LOCATION="${pkgs.onnxruntime}/lib"
              export ORT_STRATEGY="system"
              export ORT_USE_SYSTEM_LIB="1"
              
              # OpenJTalk configuration
              export OPENJTALK_LIB_DIR="${pkgs.open-jtalk}/lib"
              export OPENJTALK_INCLUDE_DIR="${pkgs.open-jtalk}/include"
              
              # PKG_CONFIG setup
              export PKG_CONFIG_PATH="${pkgs.onnxruntime}/lib/pkgconfig:${pkgs.open-jtalk}/lib/pkgconfig:$PKG_CONFIG_PATH"
              
              # CMake configuration to disable external downloads
              export CMAKE_ARGS="-DFETCHCONTENT_FULLY_DISCONNECTED=ON -DFETCHCONTENT_QUIET=OFF"
              export CMAKE_BUILD_PARALLEL_LEVEL=$(nproc)
              
              # Force system library usage for open_jtalk dependency
              export OPENJTALK_NO_DOWNLOAD=1
              export OPENJTALK_SYS_USE_PKG_CONFIG=1
              
              # Additional cmake configuration for cargo build scripts
              export CMAKE_PREFIX_PATH="${pkgs.onnxruntime}:${pkgs.open-jtalk}:$CMAKE_PREFIX_PATH"
            '';

            # Install binaries and setup runtime environment
            postInstall = ''
              # Install both client and daemon binaries (voicevox-say and voicevox-daemon)
              
              # voicevox-daemon should already be built, just make sure it exists
              if [ ! -f "$out/bin/voicevox-daemon" ]; then
                echo "Warning: voicevox-daemon binary not found"
              fi
              
              # Install VOICEVOX downloader for model management
              cp ${voicevoxSetup}/bin/voicevox-download $out/bin/
              
              # Install automatic setup script
              cp ${licenseAcceptor}/bin/voicevox-auto-setup $out/bin/
              
              # Create model setup script for users
              cat > $out/bin/voicevox-setup-models << 'EOF'
#!/bin/bash
set -euo pipefail

DOWNLOADER="voicevox-download"

# Determine target directory - prefer system-wide if writable
if [ -d "/usr/local/share/voicevox" ] && [ -w "/usr/local/share/voicevox" ]; then
    MODEL_DIR="/usr/local/share/voicevox/models"
    INSTALL_TYPE="system-wide"
elif [ -d "/opt/voicevox" ] && [ -w "/opt/voicevox" ]; then
    MODEL_DIR="/opt/voicevox/models"
    INSTALL_TYPE="system-wide"
else
    MODEL_DIR="$HOME/.local/share/voicevox/models"
    INSTALL_TYPE="user-specific"
fi

echo "Setting up VOICEVOX voice models..."
echo "Models will be downloaded to: $MODEL_DIR ($INSTALL_TYPE)"

# Create models directory
mkdir -p "$MODEL_DIR"

# Check if downloader is available
if ! command -v "$DOWNLOADER" >/dev/null 2>&1; then
    echo "Error: voicevox-download not found in PATH"
    echo "Please ensure VOICEVOX CLI is properly installed"
    exit 1
fi

# Check if any models are already present
VVM_COUNT=$(find "$MODEL_DIR" -name "*.vvm" 2>/dev/null | wc -l)

if [ "$VVM_COUNT" -gt 0 ]; then
    echo "Voice models already installed ($VVM_COUNT models found)"
    echo "Models found in: $MODEL_DIR"
    ls -la "$MODEL_DIR"/*.vvm 2>/dev/null
    echo "Use --list-models to see available models"
    exit 0
fi

echo "No voice models found. Starting download..."
echo "Downloading voice models (discovered dynamically)..."

# Use VOICEVOX downloader
if "$DOWNLOADER" --output "$MODEL_DIR" --help >/dev/null 2>&1; then
    echo "Using VOICEVOX Core downloader..."
    "$DOWNLOADER" --output "$MODEL_DIR" || {
        echo "Download failed. Please try again or download manually"
        echo "Voice models should be placed in: $MODEL_DIR"
        exit 1
    }
else
    echo "Please run the downloader manually:"
    echo "  $DOWNLOADER --output $MODEL_DIR"
    echo ""
    echo "Or download voice models from VOICEVOX official sources"
    echo "and place .vvm files in: $MODEL_DIR"
fi

echo "Voice model setup completed!"
echo "You can now use voicevox-say for text-to-speech synthesis"
EOF
              chmod +x $out/bin/voicevox-setup-models
              
              # Note: All libraries and models downloaded at runtime by voicevox-download
              # to ~/.local/share/voicevox/ (no Nix-managed libraries needed)
              
              echo "VOICEVOX CLI package installation completed"
            '';

            # Apply centralized meta information
            meta = packageMeta;
          };

          # Auto-license acceptance script for user-specific setup
          licenseAcceptor = pkgs.writeScriptBin "voicevox-auto-setup" ''
            #!${pkgs.bash}/bin/bash
            set -euo pipefail
            
            MODELS_DIR="$1"
            
            echo "🎭 VOICEVOX CLI - System Setup"
            echo "Installing VOICEVOX Core system for current user..."
            echo "Includes: VOICEVOX Core libraries, ONNX Runtime, 26+ voice models, and dictionary"
            echo ""
            echo "By using this Nix package, you agree to:"
            echo "- VOICEVOX Core Library License (commercial/non-commercial use allowed)"
            echo "- Individual voice library terms for 26+ characters (credit required: 'VOICEVOX:[Character]')"
            echo "- See: https://voicevox.hiroshiba.jp/ for details"
            echo ""
            echo "Target: $MODELS_DIR (user-specific)"
            echo "No sudo privileges required"
            echo ""
            
            # Create user models directory
            mkdir -p "$(dirname "$MODELS_DIR")"
            mkdir -p "$MODELS_DIR"
            
            # Use expect to auto-accept license during download
            ${pkgs.expect}/bin/expect -c "
              set timeout 300
              spawn ${voicevoxSetup}/bin/voicevox-download --output $MODELS_DIR
              expect {
                \"*同意しますか*\" { send \"y\r\"; exp_continue }
                \"*[y,n,r]*\" { send \"y\r\"; exp_continue }
                \"*Press*\" { send \"q\r\"; exp_continue }
                \"*を押して*\" { send \"q\r\"; exp_continue }
                eof
              }
            " || {
              echo "⚠️  Automatic download failed. You can manually run:"
              echo "  voicevox-download --output $MODELS_DIR"
              exit 1
            }
            
            echo "✅ VOICEVOX Core system setup completed!"
            echo "   VOICEVOX Core libraries, ONNX Runtime, voice models, and dictionary are available"
            echo "   26+ voice characters ready for text-to-speech synthesis"
          '';
        in
        {
          # Packages for installation
          packages = {
            default = voicevox-cli;
            voicevox-cli = voicevox-cli;
            voicevox-say = voicevox-cli; # alias for compatibility
            voicevoxSetup = voicevoxSetup; # for debugging setup
            licenseAcceptor = licenseAcceptor; # automatic license acceptance
          };

          # Apps for direct execution
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
          };

          # Development shell
          devShells.default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              pkg-config
              cmake
            ];

            shellHook = ''
              echo "VOICEVOX CLI Development Environment (Apple Silicon)"
              echo "Available commands:"
              echo "  cargo build --bin voicevox-say     - Build client"
              echo "  cargo build --bin voicevox-daemon  - Build daemon" 
              echo "  cargo run --bin voicevox-say       - Run client"
              echo "  nix build                          - Build with Nix"
              echo "  nix run                            - Run voicevox-say directly"
              echo ""
              echo "Dynamic voice detection system - no hardcoded voice names"
            '';
          };

          # Library functions for other flakes
          lib = {
            # Function to create voicevox-cli package with custom nixpkgs
            mkVoicevoxCli = pkgs: voicevox-cli;

            # Get the package derivation
            getPackage = voicevox-cli;

            # Export centralized meta information
            meta = packageMeta;
          };
        }) // {
      # Overlay for integration with nixpkgs
      overlays.default = final: prev: {
        voicevox-cli = (self.packages.${final.system} or self.packages.aarch64-darwin).voicevox-cli;
        voicevox-say = final.voicevox-cli; # alias
      };

      # Export overlay with descriptive name
      overlays.voicevox-cli = self.overlays.default;

      # Extended meta information with VOICEVOX-specific details
      meta = {
        # Basic package information (same as packageMeta)
        description = "VOICEVOX CLI for Apple Silicon - Dynamic voice detection system";
        homepage = "https://github.com/usabarashi/voicevox-cli";
        maintainers = [ "usabarashi" ];
        platforms = [ "aarch64-darwin" ];

        # Extended license information for the complete package
        license = {
          # CLI tool itself
          cli = [ "MIT" "Apache-2.0" ];

          # VOICEVOX Core component
          voicevoxCore = {
            type = "MIT";
            copyright = "2021 Hiroshiba Kazuyuki";
            url = "https://github.com/VOICEVOX/voicevox_core";
          };

          # ONNX Runtime component
          onnxRuntime = {
            type = "Custom-Terms";
            file = "./voicevox_core/onnxruntime/TERMS.txt";
            creditRequired = true;
            commercialUse = true;
          };
        };

        # Usage requirements and attribution
        attribution = {
          required = true;
          text = "Audio generated using VOICEVOX";
          voicevoxProject = "https://voicevox.hiroshiba.jp/";
          coreProject = "https://github.com/VOICEVOX/voicevox_core";
        };

        # Important notices for users
        notices = [
          "Credit VOICEVOX when using generated audio"
          "Follow individual voice library terms"
          "See voicevox_core/onnxruntime/TERMS.txt for details"
          "Reverse engineering prohibited"
        ];
      };
    };
}
