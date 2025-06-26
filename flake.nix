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
              
            '';
          };

          # Static libraries setup for build-time linking
          voicevoxResources = pkgs.stdenv.mkDerivation {
            name = "voicevox-static-libs";
            
            nativeBuildInputs = with pkgs; [ unzip gnutar ];
            
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
              
              # Extract OpenJTalk dictionary
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
            license = with licenses; [ mit asl20 ];
            maintainers = [ "usabarashi" ];
            platforms = [ "aarch64-darwin" ];
          };

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

            preBuild = ''
              
              export OPENJTALK_DICT_DIR="${voicevoxResources}/openjtalk_dict"
              export OPEN_JTALK_DICT_DIR="${voicevoxResources}/openjtalk_dict"
              export OPENJTALK_LIB_DIR="${openJTalkStaticLibs}/lib"
              export OPENJTALK_INCLUDE_DIR="${openJTalkStaticLibs}/include"
              export OPENJTALK_STATIC_LIB="1"
              export OPENJTALK_SKIP_BUILD="1"
              export OPENJTALK_NO_BUILD="1"
              
              export ORT_STRATEGY="system"
              export ORT_USE_SYSTEM_LIB="1"
              export ORT_LIB_LOCATION="${voicevoxResources}/voicevox_core/lib"
              
              export CMAKE_DISABLE_FIND_PACKAGE_Git="TRUE"
              export FETCHCONTENT_FULLY_DISCONNECTED="ON" 
              export FETCHCONTENT_QUIET="ON"
              export CMAKE_OFFLINE="ON"
              export CMAKE_BUILD_PARALLEL_LEVEL="8"
              export GIT_SSL_NO_VERIFY="false"
              
              
              export VOICEVOX_CORE_LIB_DIR="${voicevoxResources}/voicevox_core/lib"
              export VOICEVOX_CORE_INCLUDE_DIR="${voicevoxResources}/voicevox_core/include"
              
              export ORT_LIB_LOCATION="${voicevoxResources}/voicevox_core/lib"
              export ORT_STRATEGY="system"
              export ORT_USE_SYSTEM_LIB="1"
              
              export PKG_CONFIG_PATH="${openJTalkStaticLibs}/lib/pkgconfig:${voicevoxResources}/voicevox_core/lib/pkgconfig:$PKG_CONFIG_PATH"
              
              export LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$LIBRARY_PATH"
              export LD_LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$LD_LIBRARY_PATH"
              export DYLD_LIBRARY_PATH="${openJTalkStaticLibs}/lib:${voicevoxResources}/voicevox_core/lib:$DYLD_LIBRARY_PATH"
              
              export OPENJTALK_DICT_PATH="${voicevoxResources}/openjtalk_dict"
              export RUSTFLAGS="-C link-arg=-Wl,-rpath,${openJTalkStaticLibs}/lib -C link-arg=-Wl,-rpath,${voicevoxResources}/voicevox_core/lib --cfg openjtalk_dict_path $RUSTFLAGS"
              
            '';

            postInstall = ''
              if [ ! -f "$out/bin/voicevox-daemon" ]; then
                echo "Warning: voicevox-daemon binary not found"
              fi
              cp ${voicevoxResources}/bin/voicevox-download $out/bin/
              
              install -m755 ${./scripts/voicevox-setup-models.sh} $out/bin/voicevox-setup-models
              chmod +x $out/bin/voicevox-setup-models
              
            '';

            meta = packageMeta;
          };

          licenseAcceptor = pkgs.runCommand "voicevox-auto-setup" {} ''
            mkdir -p $out/bin
            substitute ${./scripts/voicevox-auto-setup.sh.template} $out/bin/voicevox-auto-setup \
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
            
            # CI Task Runner - All checks in one command
            ci = {
              type = "app";
              program = "${pkgs.writeShellScript "ci" ''
                set -euo pipefail
                echo "🔍 Running Complete CI Pipeline..."
                echo "=================================="
                
                # Static Analysis
                echo ""
                echo "📦 Checking Nix flake..."
                nix flake check --show-trace
                
                echo ""
                echo "🛠️  Verifying Rust toolchain..."
                nix develop --command rustc --version
                nix develop --command cargo --version
                
                echo ""
                echo "📝 Checking code formatting..."
                nix develop --command cargo fmt --check
                
                echo ""
                echo "🧹 Running clippy analysis..."
                nix develop --command cargo clippy --all-targets --all-features -- -D warnings
                
                echo ""
                echo "📜 Checking script syntax..."
                bash -n ${./.}/scripts/voicevox-setup-models.sh
                sed 's/@@[^@]*@@/placeholder/g' ${./.}/scripts/voicevox-auto-setup.sh.template | bash -n
                
                echo ""
                echo "✅ All CI checks completed successfully!"
              ''}";
            };
          };

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

          lib = {
            mkVoicevoxCli = pkgs: voicevox-cli;

            getPackage = voicevox-cli;

            meta = packageMeta;
          };
        }) // {
      overlays.default = final: prev: {
        voicevox-cli = (self.packages.${final.system} or self.packages.aarch64-darwin).voicevox-cli;
        voicevox-say = final.voicevox-cli;
      };

      overlays.voicevox-cli = self.overlays.default;

      meta = {
        description = "VOICEVOX CLI for Apple Silicon - Dynamic voice detection system";
        homepage = "https://github.com/usabarashi/voicevox-cli";
        maintainers = [ "usabarashi" ];
        platforms = [ "aarch64-darwin" ];

        license = {
          cli = [ "MIT" "Apache-2.0" ];

          voicevoxCore = {
            type = "MIT";
            copyright = "2021 Hiroshiba Kazuyuki";
            url = "https://github.com/VOICEVOX/voicevox_core";
          };

          onnxRuntime = {
            type = "Custom-Terms";
            file = "./voicevox_core/onnxruntime/TERMS.txt";
            creditRequired = true;
            commercialUse = true;
          };
        };

        attribution = {
          required = true;
          text = "Audio generated using VOICEVOX";
          voicevoxProject = "https://voicevox.hiroshiba.jp/";
          coreProject = "https://github.com/VOICEVOX/voicevox_core";
        };

        notices = [
          "Credit VOICEVOX when using generated audio"
          "Follow individual voice library terms"
          "See voicevox_core/onnxruntime/TERMS.txt for details"
          "Reverse engineering prohibited"
        ];
      };
    };
}
