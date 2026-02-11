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
    extra-substituters = [
      "https://voicevox-cli.cachix.org"
      "https://nix-community.cachix.org"
    ];

    extra-trusted-public-keys = [
      "voicevox-cli.cachix.org-1:mgBVkErTVM4g1h08Bz86D73qhB4Jew/+JQ4iCjaPzj0="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
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

        # Shared cargo lock configuration (used by build and checks)
        cargoLockConfig = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "open_jtalk-0.1.25" = "sha256-sdUWHHY+eY3bWMGSPu/+0jGz1f4HMHq3D17Tzbwt0Nc=";
            "voicevox_core-0.0.0" = "sha256-QmnZSHB5tBxjVMEU5n0GVeV7W9c0/THXfsaN6Tu4R4Q=";
            "voicevox-ort-2.0.0-rc.4" = "sha256-ZGT3M4GkmSgAqXwuzBvnF+Zs37TPNfKXoEqTsqoT6R4=";
          };
        };

        # Shared source filter
        srcFiltered = pkgs.lib.cleanSourceWith {
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

        # Shared native build inputs for Rust compilation
        commonNativeBuildInputs = with pkgs; [
          rustToolchain.defaultToolchain
          pkg-config
          cmake
          gnumake
          autoconf
          automake
          libtool
          git
          cacert
        ];

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

        voicevox-cli = pkgs.rustPlatform.buildRustPackage {
          pname = "voicevox-cli";
          version = "0.1.0";

          src = srcFiltered;
          cargoLock = cargoLockConfig;

          doCheck = false;

          # Force offline mode to ensure reproducible builds
          CARGO_NET_OFFLINE = true;

          # Minimal pre-configure setup
          preConfigure = ''
            # Create a temporary HOME for build process
            export HOME=$PWD/build-home
            mkdir -p $HOME
          '';

          nativeBuildInputs = commonNativeBuildInputs;

          buildInputs = [ ];

          # Build-time environment variables
          preBuild = ''
            # Git SSL configuration
            export GIT_SSL_CAINFO="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';

          # Clippy analysis runs after cargo build --release completes.
          # Reuses build artifacts from the same sandbox environment,
          # avoiding double compilation from a separate checks.clippy derivation.
          postBuild = ''
            cargo clippy --release --all-targets --all-features -- -D warnings
          '';

          postInstall = ''
            # Install download utility
            cp ${voicevoxResources}/bin/voicevox-download $out/bin/

            # Install setup script
            install -m755 ${./scripts/voicevox-setup.sh} $out/bin/voicevox-setup

            # Install INSTRUCTIONS.md for MCP server
            install -m644 ${./INSTRUCTIONS.md} $out/bin/INSTRUCTIONS.md
          '';

          meta = packageMeta;
        };

        # Development utility: reset daemon state
        voicevoxResetWrapper = pkgs.writeShellScriptBin "voicevox-reset" (builtins.readFile ./scripts/voicevox-reset.sh);

      in
      {
        packages = {
          default = voicevox-cli;
          voicevox-cli = voicevox-cli;
          voicevox-say = voicevox-cli;
          voicevoxResources = voicevoxResources;
        };

        checks = {
          # Code formatting check
          formatting = pkgs.runCommand "check-formatting" {
            nativeBuildInputs = [ rustToolchain.defaultToolchain ];
            src = srcFiltered;
          } ''
            cd $src
            export HOME=$TMPDIR
            cargo fmt --check
            touch $out
          '';

          # Shell script syntax validation
          scripts = pkgs.runCommand "check-scripts" {
            nativeBuildInputs = with pkgs; [
              bash
              gnused
              gnugrep
            ];
            src = ./.;
          } ''
            test -f $src/scripts/voicevox-setup.sh || (echo "Missing voicevox-setup.sh" && exit 1)

            for script in $src/scripts/*.sh; do
              if [ -f "$script" ]; then
                echo "Validating: $(basename "$script")"
                if grep -q '@@.*@@' "$script"; then
                  sed 's/@@[^@]*@@/placeholder/g' "$script" | bash -n
                else
                  bash -n "$script"
                fi
              fi
            done
            touch $out
          '';

          # Build verification
          build = voicevox-cli;
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

            # Development utilities
            voicevoxResetWrapper
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
            echo "  voicevox-reset                     - Reset daemon state (kill processes + remove socket)"
            echo ""
            echo "Dynamic voice detection system - no hardcoded voice names"
          '';
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
    };
}
