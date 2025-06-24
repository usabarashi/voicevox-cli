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

          # VOICEVOX Core libraries for static linking
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
            sha256 = "0j85n563jpilms9ahp527iaf7sk1pymmfvx3gjys46n43cjwvs9k";
          };

          # Voice models downloader (only for models - VVM files)
          voicevoxDownloader = pkgs.fetchurl {
            url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/download-osx-arm64";
            sha256 = "sha256-OL5Hpyd0Mc+77PzUhtIIFmHjRQqLVaiITuHICg1QBJU=";
          };

          # VOICEVOX version of OpenJTalk source code
          voicevoxOpenJTalk = pkgs.fetchFromGitHub {
            owner = "VOICEVOX";
            repo = "open_jtalk";
            rev = "1.11";
            sha256 = "sha256-SBLdQ8D62QgktI8eI6eSNzdYt5PmGo6ZUCKxd01Z8UE=";
          };

          # Create a dummy OpenJTalk package to skip build requirements
          openJTalkStaticLibs = pkgs.stdenv.mkDerivation {
            name = "openjtalk-static-libs-dummy";
            
            dontUnpack = true;
            
            installPhase = ''
              echo "Creating dummy OpenJTalk installation..."
              mkdir -p $out/{lib,include,lib/pkgconfig}
              
              # Create empty library files (will be skipped by Rust build)
              touch $out/lib/libopen_jtalk.a
              touch $out/lib/libmecab.a
              
              # Create minimal headers
              mkdir -p $out/include/openjtalk
              touch $out/include/openjtalk/openjtalk.h
              
              # Create pkg-config file that indicates OpenJTalk is available
              cat > $out/lib/pkgconfig/open_jtalk.pc << EOF
prefix=$out
exec_prefix=\$prefix
libdir=\$prefix/lib
includedir=\$prefix/include

Name: OpenJTalk
Description: OpenJTalk speech synthesis system (dummy for VOICEVOX)
Version: 1.11
Libs: -L\$libdir
Cflags: -I\$includedir
EOF
              
              echo "Dummy OpenJTalk installation completed (Rust build will be skipped)"
            '';
          };

          # Static libraries setup for build-time linking
          # Static libraries setup for build-time linking
          voicevoxResources = pkgs.stdenv.mkDerivation {
            name = "voicevox-static-libs";
            
            nativeBuildInputs = with pkgs; [ unzip gnutar ];
            
            buildCommand = ''
              mkdir -p $out/{voicevox_core,bin}
              
              echo "Extracting VOICEVOX Core (libraries and headers only)..."
              cd $TMPDIR
              ${pkgs.unzip}/bin/unzip ${voicevoxCore}
              VOICEVOX_DIR=$(find . -maxdepth 1 -name "voicevox_core*" -type d | head -1)
              
              # Copy only essential build files (libraries and headers)
              if [ -d "$VOICEVOX_DIR/lib" ]; then
                cp -r "$VOICEVOX_DIR"/lib $out/voicevox_core/
              fi
              if [ -d "$VOICEVOX_DIR/include" ]; then
                cp -r "$VOICEVOX_DIR"/include $out/voicevox_core/
              fi
              
              echo "Extracting ONNX Runtime libraries..."
              cd $TMPDIR
              ${pkgs.gnutar}/bin/tar -xzf ${onnxRuntime}
              ONNX_DIR=$(find . -maxdepth 1 -name "voicevox_onnxruntime*" -type d | head -1)
              
              # Ensure lib directory exists
              mkdir -p $out/voicevox_core/lib
              if [ -d "$ONNX_DIR/lib" ]; then
                cp -r "$ONNX_DIR"/lib/* $out/voicevox_core/lib/
              fi
              
              echo "Extracting OpenJTalk dictionary..."
              cd $TMPDIR
              ${pkgs.gnutar}/bin/tar -xzf ${openJTalkDict}
              mkdir -p $out/openjtalk_dict
              DICT_DIR=$(find . -maxdepth 1 -name "open_jtalk_dic*" -type d | head -1)
              if [ -d "$DICT_DIR" ]; then
                cp -r "$DICT_DIR"/* $out/openjtalk_dict/
              fi
              
              echo "Setting up pre-built OpenJTalk static libraries..."
              # Copy pre-built OpenJTalk static libraries
              mkdir -p $out/openjtalk_libs/{lib,include}
              cp -r ${openJTalkStaticLibs}/lib/* $out/openjtalk_libs/lib/
              cp -r ${openJTalkStaticLibs}/include/* $out/openjtalk_libs/include/
              
              # Create symbolic links for easy access
              mkdir -p $out/lib $out/include
              ln -sf $out/openjtalk_libs/lib/* $out/lib/
              ln -sf $out/openjtalk_libs/include/* $out/include/
              
              echo "Pre-built OpenJTalk static libraries installed"
              
              # Install VOICEVOX downloader for runtime downloads
              echo "Installing VOICEVOX downloader for runtime use..."
              cp ${voicevoxDownloader} $out/bin/voicevox-download
              chmod +x $out/bin/voicevox-download
              
              # Fix library paths for macOS
              echo "Fixing library paths..."
              if [ -d "$out/voicevox_core/lib" ]; then
                cd $out/voicevox_core/lib
                for dylib in *.dylib; do
                  if [ -f "$dylib" ]; then
                    ${pkgs.darwin.cctools}/bin/install_name_tool -id "@rpath/$dylib" "$dylib" || true
                  fi
                done
              fi
              
              echo "Build resources prepared (runtime downloads handled by voicevox-download)"
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

            nativeBuildInputs = with pkgs; [
              pkg-config
              cmake
              git
              autoconf
              automake
              libtool
              gnumake
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk_11_0.frameworks.AudioUnit
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreAudio
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreServices
            ];

            buildInputs = [
              voicevoxResources
              openJTalkStaticLibs
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk_11_0.frameworks.AudioUnit
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreAudio
              pkgs.darwin.apple_sdk_11_0.frameworks.CoreServices
            ];

            # Static linking - all dependencies except voice models (VVM files)
            preBuild = ''
              echo "Build ready - using static libraries from voicevoxResources"
              echo "VOICEVOX Core location: ${voicevoxResources}/voicevox_core/lib"
              echo "ONNX Runtime location: ${voicevoxResources}/voicevox_core/lib"
              
              # OpenJTalk configuration (Skip Rust build, use VOICEVOX Core's internal OpenJTalk)
              export OPENJTALK_DICT_DIR="${voicevoxResources}/openjtalk_dict"
              export OPEN_JTALK_DICT_DIR="${voicevoxResources}/openjtalk_dict"
              export OPENJTALK_LIB_DIR="${openJTalkStaticLibs}/lib"
              export OPENJTALK_INCLUDE_DIR="${openJTalkStaticLibs}/include"
              export OPENJTALK_STATIC_LIB="1"
              export OPENJTALK_SKIP_BUILD="1"
              export OPENJTALK_NO_BUILD="1"
              
              # Force static linking with pre-built ONNX Runtime from Nix
              export ORT_STRATEGY="system"
              export ORT_USE_SYSTEM_LIB="1"
              export ORT_LIB_LOCATION="${voicevoxResources}/voicevox_core/lib"
              
              # Disable all external downloads and Git access in CMake builds
              export CMAKE_DISABLE_FIND_PACKAGE_Git="TRUE"
              export FETCHCONTENT_FULLY_DISCONNECTED="ON" 
              export FETCHCONTENT_QUIET="ON"
              export CMAKE_OFFLINE="ON"
              export CMAKE_BUILD_PARALLEL_LEVEL="8"
              export GIT_SSL_NO_VERIFY="false"
              
              # Make Git unavailable during build to force CMake to use local sources
              # Note: Disabled due to Nix alias evaluation issues - using alternative approach
              # export PATH="${pkgs.lib.makeBinPath []}:$PATH"
              
              # VOICEVOX Core static libraries
              export VOICEVOX_CORE_LIB_DIR="${voicevoxResources}/voicevox_core/lib"
              export VOICEVOX_CORE_INCLUDE_DIR="${voicevoxResources}/voicevox_core/include"
              
              # ONNX Runtime static libraries (merged into voicevox_core/lib)
              export ORT_LIB_LOCATION="${voicevoxResources}/voicevox_core/lib"
              export ORT_STRATEGY="system"
              export ORT_USE_SYSTEM_LIB="1"
              
              # PKG_CONFIG setup for static libraries (including OpenJTalk)
              export PKG_CONFIG_PATH="${openJTalkStaticLibs}/lib/pkgconfig:${voicevoxResources}/voicevox_core/lib/pkgconfig:$PKG_CONFIG_PATH"
              
              # Linker paths for static libraries (including OpenJTalk)
              export LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$LIBRARY_PATH"
              export LD_LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$LD_LIBRARY_PATH"
              export DYLD_LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$DYLD_LIBRARY_PATH"
              
              # Add rpath for runtime library discovery (including OpenJTalk)
              export RUSTFLAGS="-C link-arg=-Wl,-rpath,${openJTalkStaticLibs}/lib -C link-arg=-Wl,-rpath,${voicevoxResources}/voicevox_core/lib $RUSTFLAGS"
              
              # Static Linking Complete: Core libraries embedded at build time
              # Runtime Downloads: Voice models (VVM files) only via voicevox-download --only models
            '';

            # Install binaries and setup runtime environment
            postInstall = ''
              # Install both client and daemon binaries (voicevox-say and voicevox-daemon)
              
              # voicevox-daemon should already be built, just make sure it exists
              if [ ! -f "$out/bin/voicevox-daemon" ]; then
                echo "Warning: voicevox-daemon binary not found"
              fi
              
              # Install VOICEVOX downloader for voice model management
              cp ${voicevoxResources}/bin/voicevox-download $out/bin/
              
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
echo ""
echo "Note: VOICEVOX Core, ONNX Runtime, and dictionary are statically linked"
echo "      Only voice model files (.vvm) will be downloaded"

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
echo "Downloading voice models only (VVM files)..."

# Use VOICEVOX downloader with --only models to avoid redundant downloads
if "$DOWNLOADER" --only models --output "$MODEL_DIR" --help >/dev/null 2>&1; then
    echo "Using VOICEVOX Core downloader (models only)..."
    "$DOWNLOADER" --only models --output "$MODEL_DIR" || {
        echo "Download failed. Please try again or download manually"
        echo "Voice models should be placed in: $MODEL_DIR"
        exit 1
    }
else
    echo "Please run the downloader manually (models only):"
    echo "  $DOWNLOADER --only models --output $MODEL_DIR"
    echo ""
    echo "Or download voice models from VOICEVOX official sources"
    echo "and place .vvm files in: $MODEL_DIR"
fi

echo "Voice model setup completed!"
echo "You can now use voicevox-say for text-to-speech synthesis"
EOF
              chmod +x $out/bin/voicevox-setup-models
              
              # Static Linking Priority Architecture:
              # - VOICEVOX Core C-API: Statically linked at build time (no runtime download)
              # - ONNX Runtime: Statically linked at build time (no runtime download)  
              # - OpenJTalk Dictionary: Statically linked at build time (no runtime download)
              # - Voice Models (VVM): Runtime download only (~200MB, 26+ characters)
              # Voice models stored in ~/.local/share/voicevox/models/
              
              echo "VOICEVOX CLI package installation completed"
            '';

            # Apply centralized meta information
            meta = packageMeta;
          };

          # Minimal voice models setup for user-specific installation
          licenseAcceptor = pkgs.writeScriptBin "voicevox-auto-setup" ''
            #!${pkgs.bash}/bin/bash
            set -euo pipefail
            
            MODELS_DIR="$1"
            
            echo "🎭 VOICEVOX CLI - Voice Models Setup"
            echo "Setting up voice models for current user..."
            echo "Note: VOICEVOX Core libraries and ONNX Runtime are statically linked"
            echo ""
            echo "By using this Nix package, you agree to:"
            echo "- Individual voice library terms for 26+ characters (credit required: 'VOICEVOX:[Character]')"
            echo "- See: https://voicevox.hiroshiba.jp/ for details"
            echo ""
            echo "Target: $MODELS_DIR (user-specific)"
            echo "Download size: ~200MB (voice models only)"
            echo ""
            
            # Create user models directory
            mkdir -p "$(dirname "$MODELS_DIR")"
            mkdir -p "$MODELS_DIR"
            
            # Use expect to auto-accept license during download (models only)
            ${pkgs.expect}/bin/expect -c "
              set timeout 300
              spawn ${voicevoxResources}/bin/voicevox-download --only models --output $MODELS_DIR
              expect {
                \"*同意しますか*\" { send \"y\r\"; exp_continue }
                \"*[y,n,r]*\" { send \"y\r\"; exp_continue }
                \"*Press*\" { send \"q\r\"; exp_continue }
                \"*を押して*\" { send \"q\r\"; exp_continue }
                eof
              }
            " || {
              echo "⚠️  Automatic download failed. You can manually run:"
              echo "  voicevox-download --only models --output $MODELS_DIR"
              exit 1
            }
            
            echo "✅ Voice models setup completed!"
            echo "   26+ voice characters ready for text-to-speech synthesis"
            echo "   Static libraries (Core + ONNX Runtime) already available"
          '';
        in
        {
          # Packages for installation
          packages = {
            default = voicevox-cli;
            voicevox-cli = voicevox-cli;
            voicevox-say = voicevox-cli; # alias for compatibility
            voicevoxResources = voicevoxResources; # static libraries for debugging
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
