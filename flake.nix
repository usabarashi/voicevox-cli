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
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" ] (
      system:
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

        # Voice models downloader
        voicevoxDownloader = pkgs.fetchurl {
          url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/download-osx-arm64";
          sha256 = "sha256-OL5Hpyd0Mc+77PzUhtIIFmHjRQqLVaiITuHICg1QBJU=";
        };

        voicevoxOpenJTalk = pkgs.fetchFromGitHub {
          owner = "VOICEVOX";
          repo = "open_jtalk";
          rev = "1.11";
          sha256 = "sha256-SBLdQ8D62QgktI8eI6eSNzdYt5PmGo6ZUCKxd01Z8UE=";
        };

        openJTalkStaticLibs = pkgs.stdenv.mkDerivation {
          name = "openjtalk-static-libs-dummy";

          dontUnpack = true;

          installPhase = ''
            echo "Creating dummy OpenJTalk installation..."
            mkdir -p $out/{lib,include,lib/pkgconfig}

            touch $out/lib/libopen_jtalk.a
            touch $out/lib/libmecab.a

            mkdir -p $out/include/openjtalk
            touch $out/include/openjtalk/openjtalk.h

            # Generate pkg-config file from template
            substitute ${./open_jtalk.pc} $out/lib/pkgconfig/open_jtalk.pc \
              --replace "@out@" "$out"
          '';
        };

        # Static libraries setup for build-time linking
        voicevoxResources = pkgs.stdenv.mkDerivation {
          name = "voicevox-static-libs";

          nativeBuildInputs = with pkgs; [
            unzip
            gnutar
          ];

          buildCommand = ''
            mkdir -p $out/{voicevox_core,bin,openjtalk_dict}
            cd $TMPDIR
            ${pkgs.unzip}/bin/unzip ${voicevoxCore}
            VOICEVOX_DIR=$(find . -maxdepth 1 -name "voicevox_core*" -type d | head -1)
            if [ -d "$VOICEVOX_DIR/lib" ]; then
              cp -r "$VOICEVOX_DIR"/lib $out/voicevox_core/
            fi

            cd $TMPDIR
            ${pkgs.gnutar}/bin/tar -xzf ${onnxRuntime}
            ONNX_DIR=$(find . -maxdepth 1 -name "voicevox_onnxruntime*" -type d | head -1)
            mkdir -p $out/voicevox_core/lib
            if [ -d "$ONNX_DIR/lib" ]; then
              cp -r "$ONNX_DIR"/lib/* $out/voicevox_core/lib/
            fi

            cd $TMPDIR
            ${pkgs.gnutar}/bin/tar -xzf ${openJTalkDict}
            DICT_DIR=$(find . -maxdepth 1 -name "open_jtalk_dic*" -type d | head -1)
            if [ -d "$DICT_DIR" ]; then
              cp -r "$DICT_DIR"/* $out/openjtalk_dict/
              echo "OpenJTalk dictionary extracted to $out/openjtalk_dict/"
              ls -la $out/openjtalk_dict/
            else
              echo "Warning: OpenJTalk dictionary directory not found"
            fi

            cp ${voicevoxDownloader} $out/bin/voicevox-download
            chmod +x $out/bin/voicevox-download

            if [ -d "$out/voicevox_core/lib" ]; then
              cd $out/voicevox_core/lib
              for dylib in *.dylib; do
                if [ -f "$dylib" ]; then
                  ${pkgs.darwin.cctools}/bin/install_name_tool -id "@rpath/$dylib" "$dylib" || true
                fi
              done
              if [ -f "libvoicevox_onnxruntime.dylib" ]; then
                ln -sf libvoicevox_onnxruntime.dylib libonnxruntime.dylib
              fi
            fi
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

        voicevox-cli = pkgs.rustPlatform.buildRustPackage {
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
              "voicevox_core-0.0.0" = "sha256-Ud/D3k8J8wOJiNiQ1bWi2RTS+Ix+ImqNEiyMHcCud78=";
              "voicevox-ort-2.0.0-rc.4" = "sha256-ZGT3M4GkmSgAqXwuzBvnF+Zs37TPNfKXoEqTsqoT6R4=";
            };
          };

          doCheck = false;
          
          # Pre-configure phase to setup build environment
          preConfigure = ''
            # Create ORT cache directory structure that build.rs expects
            export HOME=$PWD/build-home
            mkdir -p $HOME/Library/Caches/voicevox_ort/dfbin/aarch64-apple-darwin/97B40A49637FA94D9D1090C2B1382CDDDD6747382472F763D3422D1710AAEA36/onnxruntime-osx-arm64-1.17.3/lib
            
            # Copy ONNX Runtime libraries to expected location
            if [ -d "${voicevoxResources}/voicevox_core/lib" ]; then
              cp -r ${voicevoxResources}/voicevox_core/lib/* \
                $HOME/Library/Caches/voicevox_ort/dfbin/aarch64-apple-darwin/97B40A49637FA94D9D1090C2B1382CDDDD6747382472F763D3422D1710AAEA36/onnxruntime-osx-arm64-1.17.3/lib/
            fi
            
            # Also create include directory
            mkdir -p $HOME/Library/Caches/voicevox_ort/dfbin/aarch64-apple-darwin/97B40A49637FA94D9D1090C2B1382CDDDD6747382472F763D3422D1710AAEA36/onnxruntime-osx-arm64-1.17.3/include
            if [ -d "${voicevoxResources}/voicevox_core/include" ]; then
              cp -r ${voicevoxResources}/voicevox_core/include/* \
                $HOME/Library/Caches/voicevox_ort/dfbin/aarch64-apple-darwin/97B40A49637FA94D9D1090C2B1382CDDDD6747382472F763D3422D1710AAEA36/onnxruntime-osx-arm64-1.17.3/include/
            fi
            
            # Create VERSION_NUMBER file that voicevox-ort-sys expects
            echo "1.17.3" > $HOME/Library/Caches/voicevox_ort/dfbin/aarch64-apple-darwin/97B40A49637FA94D9D1090C2B1382CDDDD6747382472F763D3422D1710AAEA36/onnxruntime-osx-arm64-1.17.3/VERSION_NUMBER
          '';

          nativeBuildInputs = with pkgs; [
            # Rust tools
            rustfmt
            clippy
            
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

          buildInputs = [
            voicevoxResources
            openJTalkStaticLibs
          ];

          # Build-time environment variables
          preBuild = ''
            # Run full CI checks before build
            ${pkgs.bash}/bin/bash ${./scripts/ci.sh} --build-phase || exit 1
            
            # Git SSL configuration
            export GIT_SSL_CAINFO="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            
            # OpenJTalk configuration
            # Used by build.rs to embed dictionary path at compile time
            export OPENJTALK_DICT_PATH="${voicevoxResources}/openjtalk_dict"

            # ONNX Runtime configuration
            export ORT_STRATEGY="system"
            export ORT_USE_SYSTEM_LIB="1"
            export ORT_LIB_LOCATION="${voicevoxResources}/voicevox_core/lib"

            # CMake configuration
            export CMAKE_DISABLE_FIND_PACKAGE_Git="TRUE"
            export FETCHCONTENT_FULLY_DISCONNECTED="ON"
            export FETCHCONTENT_QUIET="ON"
            export CMAKE_OFFLINE="ON"
            export CMAKE_BUILD_PARALLEL_LEVEL="8"
            export GIT_SSL_NO_VERIFY="false"

            # VOICEVOX Core configuration
            export VOICEVOX_CORE_LIB_DIR="${voicevoxResources}/voicevox_core/lib"
            export VOICEVOX_CORE_INCLUDE_DIR="${voicevoxResources}/voicevox_core/include"

            # Build paths
            export PKG_CONFIG_PATH="${openJTalkStaticLibs}/lib/pkgconfig:${voicevoxResources}/voicevox_core/lib/pkgconfig:$PKG_CONFIG_PATH"
            export LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$LIBRARY_PATH"
            export LD_LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$LD_LIBRARY_PATH"
            export DYLD_LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$DYLD_LIBRARY_PATH"

            # Rust flags
            export RUSTFLAGS="-C link-arg=-Wl,-rpath,${openJTalkStaticLibs}/lib -C link-arg=-Wl,-rpath,${voicevoxResources}/voicevox_core/lib $RUSTFLAGS"
          '';

          postInstall = ''
            # Install binaries
            cp ${voicevoxResources}/bin/voicevox-download $out/bin/
            install -m755 ${./scripts/voicevox-setup-models.sh} $out/bin/voicevox-setup-models
            
            # Install OpenJTalk dictionary to standard location
            mkdir -p $out/share/voicevox
            if [ -d "${voicevoxResources}/openjtalk_dict" ]; then
              cp -r ${voicevoxResources}/openjtalk_dict $out/share/voicevox/openjtalk_dict
              echo "✓ OpenJTalk dictionary installed to $out/share/voicevox/openjtalk_dict"
            else
              echo "✗ ERROR: OpenJTalk dictionary not found in build resources!"
              exit 1
            fi
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
            program = toString (pkgs.writeShellScript "ci-runner" ''
              exec ${pkgs.bash}/bin/bash ${./scripts/ci.sh}
            '');
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            
            # Build tools
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
