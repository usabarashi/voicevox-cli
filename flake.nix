{
  description = "VOICEVOX CLI";

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
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
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
            rustToolchain.rust-analyzer

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

            # Simplified build configuration - no ONNX or OpenJTalk dependencies needed
            # Resources will be downloaded at runtime
          '';

          postInstall = ''
            # Install download utility
            cp ${voicevoxResources}/bin/voicevox-download $out/bin/

            # Install setup script
            install -m755 ${./scripts/voicevox-setup.sh} $out/bin/voicevox-setup

            # Install VOICEVOX.md for MCP server
            install -m644 ${./VOICEVOX.md} $out/bin/VOICEVOX.md

            # Note: All resources (ONNX, dict, models) will be downloaded at runtime
          '';

          meta = packageMeta;
        };

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
            rustToolchain.rust-analyzer
            cargo-audit

            # Build tools
            pkg-config
            cmake

            # UV for Python package management (for Serena MCP)
            nixd
            uv
          ];
        };

        lib = {
          mkVoicevoxCli = pkgs: voicevox-cli;
          getPackage = voicevox-cli;
          meta = packageMeta;
        };
      }
    )
    // {
      overlays.default = final: prev: {
        voicevox-cli = (self.packages.${final.system} or self.packages.aarch64-darwin).voicevox-cli;
        voicevox-say = final.voicevox-cli;
      };

      overlays.voicevox-cli = self.overlays.default;
    };
}
