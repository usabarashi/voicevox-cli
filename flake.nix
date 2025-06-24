{
  description = ''
    VOICEVOX CLI tool for text-to-speech synthesis

    This tool uses VOICEVOX Core (MIT License) and requires proper attribution.
    When using generated audio, please credit VOICEVOX appropriately.

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

          # Fixed-output derivations for VOICEVOX dependencies with SHA256 hashes
          voicevoxCore = pkgs.fetchurl {
            url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/voicevox_core-osx-arm64-0.16.0.zip";
            sha256 = "sha256-vCAvITP9j5tNa/5yWkcmdthAy0gPya9IpZ8NGm/LDhQ=";
          };

          onnxRuntime = pkgs.fetchurl {
            url = "https://github.com/VOICEVOX/onnxruntime-builder/releases/download/voicevox_onnxruntime-1.17.3/voicevox_onnxruntime-osx-arm64-1.17.3.tgz";
            sha256 = "sha256-ltfqGSigoVSFSS03YhOH31D0CnkuKmgX1N9z7NGFcfI=";
          };

          openJTalkDict = pkgs.fetchurl {
            url = "https://sourceforge.net/projects/open-jtalk/files/Dictionary/open_jtalk_dic-1.11/open_jtalk_dic_utf_8-1.11.tar.gz/download";
            sha256 = "sha256-M+nNJRvEGqK9fKNvV6u/YerjVDyiXKiSrjReOUyxBUk=";
          };

          voicevoxDownloader = pkgs.fetchurl {
            url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/download-osx-arm64";
            sha256 = "sha256-OL5Hpyd0Mc+77PzUhtIIFmHjRQqLVaiITuHICg1QBJU=";
          };

          # Prepare VOICEVOX resources with fixed hashes and download voice models using VOICEVOX Core tools
          voicevoxResources = pkgs.stdenv.mkDerivation {
            name = "voicevox-resources";
            
            nativeBuildInputs = with pkgs; [ unzip gnutar curl ];
            
            buildCommand = ''
              mkdir -p $out/{voicevox_core,models,dict}
              
              echo "Extracting VOICEVOX Core..."
              cd $TMPDIR
              ${pkgs.unzip}/bin/unzip ${voicevoxCore}
              VOICEVOX_DIR=$(find . -maxdepth 1 -name "voicevox_core*" -type d | head -1)
              cp -r "$VOICEVOX_DIR"/* $out/voicevox_core/
              
              echo "Extracting ONNX Runtime..."
              cd $TMPDIR
              ${pkgs.gnutar}/bin/tar -xzf ${onnxRuntime}
              ONNX_DIR=$(find . -maxdepth 1 -name "voicevox_onnxruntime*" -type d | head -1)
              cp -r "$ONNX_DIR"/lib/* $out/voicevox_core/lib/
              
              echo "Downloading voice models using VOICEVOX Core downloader..."
              cd $TMPDIR
              
              # Copy and setup VOICEVOX downloader
              cp ${voicevoxDownloader} ./download
              chmod +x ./download
              
              echo "Using VOICEVOX Core official downloader..."
              ./download --help || true
              
              # Install downloader for runtime use
              echo "Installing VOICEVOX downloader for runtime model downloads..."
              mkdir -p $out/bin
              cp ./download $out/bin/voicevox-download
              chmod +x $out/bin/voicevox-download
              
              # Create empty models directory (actual models will be in user directories)
              mkdir -p $out/models
              echo "Voice models will be downloaded to user directories at runtime"
              
              echo "Extracting OpenJTalk dictionary..."
              cd $TMPDIR
              ${pkgs.gnutar}/bin/tar -xzf ${openJTalkDict}
              DICT_DIR=$(find . -maxdepth 1 -name "open_jtalk_dic*" -type d | head -1)
              cp -r "$DICT_DIR"/* $out/dict/
              
              echo "Fixing library paths..."
              cd $out/voicevox_core/lib
              for dylib in *.dylib; do
                if [ -f "$dylib" ]; then
                  ${pkgs.darwin.cctools}/bin/install_name_tool -id "@rpath/$dylib" "$dylib" || true
                fi
              done
              
              echo "VOICEVOX resources prepared successfully"
            '';
          };

          # Centralized meta information
          packageMeta = with pkgs.lib; {
            description = "VOICEVOX CLI tool for text-to-speech synthesis";
            homepage = "https://github.com/usabarashi/voicevox-cli";
            license = with licenses; [ mit asl20 ];
            maintainers = [ "usabarashi" ];
            platforms = [ "aarch64-darwin" ];
          };

          # Package definition using voicevoxResources
          voicevox-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "voicevox-tts";
            version = "0.1.0";

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            # Skip tests since they require VOICEVOX runtime libraries
            doCheck = false;

            nativeBuildInputs = with pkgs; [
              pkg-config
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.AudioUnit
              pkgs.darwin.apple_sdk.frameworks.CoreAudio
              pkgs.darwin.apple_sdk.frameworks.CoreServices
            ];

            buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.AudioUnit
              pkgs.darwin.apple_sdk.frameworks.CoreAudio
              pkgs.darwin.apple_sdk.frameworks.CoreServices
            ];

            # Setup VOICEVOX resources before build
            preBuild = ''
              echo "Setting up VOICEVOX resources from Nix store..."
              
              # Copy VOICEVOX Core
              cp -r ${voicevoxResources}/voicevox_core ./
              chmod -R u+w voicevox_core
              
              # Copy voice models
              cp -r ${voicevoxResources}/models ./
              chmod -R u+w models
              
              # Copy OpenJTalk dictionary
              cp -r ${voicevoxResources}/dict ./
              chmod -R u+w dict
              
              echo "VOICEVOX resources ready for build"
            '';

            # Install binaries and setup runtime environment
            postInstall = ''
              # Install both client and daemon binaries (voicevox-say and voicevox-daemon are already correct)
              # Remove legacy voicevox-cli if it exists
              if [ -f "$out/bin/voicevox-cli" ]; then
                rm $out/bin/voicevox-cli
              fi
              
              # voicevox-daemon should already be built, just make sure it exists
              if [ ! -f "$out/bin/voicevox-daemon" ]; then
                echo "Warning: voicevox-daemon binary not found"
              fi
              
              # Install VOICEVOX downloader for model management
              cp ${voicevoxResources}/bin/voicevox-download $out/bin/
              
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
    echo "Please ensure VOICEVOX TTS is properly installed"
    exit 1
fi

# Download essential models if not present
ESSENTIAL_MODELS=("3.vvm" "2.vvm" "8.vvm")
MISSING_MODELS=()

for model in "''${ESSENTIAL_MODELS[@]}"; do
    if [ ! -f "$MODEL_DIR/$model" ]; then
        MISSING_MODELS+=("$model")
    fi
done

if [ ''${#MISSING_MODELS[@]} -eq 0 ]; then
    echo "All essential voice models are already installed"
    echo "Models found in: $MODEL_DIR"
    ls -la "$MODEL_DIR"/*.vvm 2>/dev/null || echo "No .vvm files found"
    exit 0
fi

echo "Missing models: ''${MISSING_MODELS[*]}"
echo "Downloading essential voice models (Zundamon, Metan, Tsumugi)..."

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
              
              # Copy VOICEVOX libraries to output
              mkdir -p $out/lib
              cp -r ${voicevoxResources}/voicevox_core/lib/* $out/lib/

              # Create directories for VOICEVOX data and copy resources
              mkdir -p $out/share/voicevox/{dict,models}
              cp -r ${voicevoxResources}/dict/* $out/share/voicevox/dict/
              
              # Copy models if they exist
              if [ "$(ls -A ${voicevoxResources}/models/ 2>/dev/null)" ]; then
                cp -r ${voicevoxResources}/models/* $out/share/voicevox/models/
              else
                echo "No pre-downloaded models - models will be downloaded at runtime"
              fi

              # Fix runtime library paths on macOS
              if [[ "$OSTYPE" == "darwin"* ]]; then
                # Add rpath for runtime library discovery
                ${pkgs.darwin.cctools}/bin/install_name_tool -add_rpath "$out/lib" $out/bin/voicevox-say
                if [ -f "$out/bin/voicevox-daemon" ]; then
                  ${pkgs.darwin.cctools}/bin/install_name_tool -add_rpath "$out/lib" $out/bin/voicevox-daemon
                fi
              fi
              
              echo "VOICEVOX TTS package installation completed"
            '';

            # Apply centralized meta information
            meta = packageMeta;
          };

          # Auto-license acceptance script for user-specific setup
          licenseAcceptor = pkgs.writeScriptBin "voicevox-auto-setup" ''
            #!${pkgs.bash}/bin/bash
            set -euo pipefail
            
            MODELS_DIR="$1"
            
            echo "🎭 VOICEVOX TTS - User Setup"
            echo "Installing voice models for current user..."
            echo ""
            echo "By using this Nix package, you agree to:"
            echo "- VOICEVOX Audio Model License (commercial/non-commercial use allowed)"
            echo "- Individual voice library terms (credit required: 'VOICEVOX:[Character]')"
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
              spawn ${voicevoxResources}/bin/voicevox-download --output $MODELS_DIR --only models
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
            
            echo "✅ VOICEVOX models setup completed!"
            echo "   Voice models are available for current user"
          '';
        in
        {
          # Packages for installation
          packages = {
            default = voicevox-cli;
            voicevox-cli = voicevox-cli;
            voicevox-say = voicevox-cli; # alias for compatibility
            voicevoxResources = voicevoxResources; # for debugging hash values
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
            ];

            shellHook = ''
              echo "VOICEVOX CLI Development Environment"
              echo "Available commands:"
              echo "  cargo build    - Build the project"
              echo "  cargo run      - Run voicevox-say"
              echo "  nix build      - Build with Nix"
              echo "  nix run        - Run voicevox-say directly"
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
        description = "VOICEVOX CLI tool for text-to-speech synthesis";
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
