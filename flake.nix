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
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      crane,
      advisory-db,
    }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" ] (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;

        # Read rust-toolchain.toml to ensure consistency
        rustToolchain = fenix.packages.${system}.stable;

        # Crane with fenix integration
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain.toolchain;

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

        packageMeta = with lib; {
          description = "VOICEVOX CLI for Apple Silicon - Dynamic voice detection system";
          homepage = "https://github.com/usabarashi/voicevox-cli";
          license = with licenses; [
            mit
            asl20
          ];
          maintainers = [ "usabarashi" ];
          platforms = [ "aarch64-darwin" ];
        };

        # Git dependency hashes for offline evaluation
        outputHashes = {
          "git+https://github.com/VOICEVOX/open_jtalk-rs.git?rev=7c87b4227bb005b439a3ad473b48ce8975829576#7c87b4227bb005b439a3ad473b48ce8975829576" = "sha256-sdUWHHY+eY3bWMGSPu/+0jGz1f4HMHq3D17Tzbwt0Nc=";
          "git+https://github.com/VOICEVOX/voicevox_core.git?rev=711d8b4b464ea9b1161db093e4a1feed763b9611#711d8b4b464ea9b1161db093e4a1feed763b9611" = "sha256-QmnZSHB5tBxjVMEU5n0GVeV7W9c0/THXfsaN6Tu4R4Q=";
          "git+https://github.com/VOICEVOX/ort.git?rev=1ebb5768a78313f9db70a35c497816ec2ffae18b#1ebb5768a78313f9db70a35c497816ec2ffae18b" = "sha256-ZGT3M4GkmSgAqXwuzBvnF+Zs37TPNfKXoEqTsqoT6R4=";
        };

        # Source with custom filtering
        src =
          let
            craneFiltered = craneLib.cleanCargoSource ./.;
          in
          lib.cleanSourceWith {
            src = craneFiltered;
            filter =
              path: type:
              let
                baseName = baseNameOf path;
              in
              !(
                (type == "directory" && lib.hasSuffix "-extract" baseName)
                || (type == "regular" && lib.hasSuffix ".tar.gz" baseName)
              );
          };

        # Common build arguments
        commonArgs = {
          inherit src;
          strictDeps = true;
          cargoExtraArgs = "--locked";
          inherit outputHashes;

          nativeBuildInputs = with pkgs; [
            pkg-config
            cmake
            gnumake
            autoconf
            automake
            libtool
            git
            cacert
          ];

          buildInputs = [ ];

          CARGO_NET_OFFLINE = true;

          preConfigure = ''
            export HOME=$PWD/build-home
            mkdir -p $HOME
          '';

          preBuild = ''
            export GIT_SSL_CAINFO="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';
        };

        # Cached dependency artifacts
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "voicevox-cli-deps";
          version = "0.1.0";
          doCheck = false;
        });

        # Main package
        voicevox-cli = craneLib.buildPackage (commonArgs // {
          pname = "voicevox-cli";
          version = "0.1.0";
          inherit cargoArtifacts;
          doCheck = false;

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
        });

        # CI Checks as separate derivations
        voicevox-cli-clippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets --all-features -- -D warnings";
        });

        voicevox-cli-fmt = craneLib.cargoFmt { inherit src; };

        voicevox-cli-doc = craneLib.cargoDoc (commonArgs // {
          inherit cargoArtifacts;
        });

        voicevox-cli-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };

        voicevox-cli-test = craneLib.cargoTest (commonArgs // {
          inherit cargoArtifacts;
          cargoTestExtraArgs = "--lib";
        });

      in
      {
        checks = {
          inherit
            voicevox-cli
            voicevox-cli-clippy
            voicevox-cli-fmt
            voicevox-cli-doc
            voicevox-cli-audit
            voicevox-cli-test
            ;
        };

        packages = {
          default = voicevox-cli;
          voicevox-cli = voicevox-cli;
          voicevox-say = voicevox-cli;
          voicevoxResources = voicevoxResources;
          # Expose artifacts for debugging
          deps = cargoArtifacts;
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

        devShells.default = craneLib.devShell {
          # Include checks in shell for IDE integration
          checks = self.checks.${system};

          # Additional dev tools
          packages = with pkgs; [
            rustToolchain.rust-analyzer
            cargo-audit
            nixd
            uv
          ];

          CARGO_HOME = "./.project-home/.cargo";
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
