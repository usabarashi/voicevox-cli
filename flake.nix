{
  description = "VOICEVOX CLI";

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
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      crane,
    }:
    let
      systems = [ "aarch64-darwin" ];
    in
    flake-utils.lib.eachSystem systems (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.package.version;

        # Fenix stable toolchain
        rustToolchain = fenix.packages.${system}.stable;

        # Minimal toolchain for builds and checks (no rust-src/rust-docs)
        buildToolchain = fenix.packages.${system}.combine [
          rustToolchain.rustc
          rustToolchain.cargo
          rustToolchain.rustfmt
          rustToolchain.clippy
        ];

        # Crane library with minimal fenix toolchain
        craneLib = (crane.mkLib pkgs).overrideToolchain buildToolchain;

        # Source filtering
        src = lib.cleanSourceWith {
          src = craneLib.cleanCargoSource ./.;
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

        # ONNX Runtime library search path for build.rs (voicevox-ort-sys).
        # Actual library is loaded at runtime via dlopen (load-dynamic),
        # so only the path needs to exist at build time.
        onnxruntimeLibDir = pkgs.runCommand "onnxruntime-lib" { } ''
          mkdir -p $out/lib
        '';

        # Vendor cargo dependencies (git deps fetched at eval time)
        cargoVendorDir = craneLib.vendorCargoDeps {
          inherit src;
          overrideVendorGitCheckout =
            ps: drv:
            # VOICEVOX/ort is a workspace with excluded members (backends,
            # examples, tests) whose Cargo.toml files confuse crane's
            # package discovery. Vendor the two needed crates manually.
            if lib.any (p: p.name == "ort") ps then
              let
                pkg = name: (lib.findFirst (p: p.name == name) null ps);
                dir = p: "${p.name}-${p.version}";
                ortPkg = pkg "ort";
                sysPkg = pkg "ort-sys";
              in
              assert ortPkg != null && sysPkg != null;
              drv.overrideAttrs {
                installPhase =
                  let
                    ort = dir ortPkg;
                    sys = dir sysPkg;
                  in
                  ''
                    mkdir -p $out

                    # Root crate (copy without workspace members)
                    cp -r . $out/${ort}
                    rm -rf $out/${ort}/{ort-sys,backends,examples,tests}
                    echo '{"files":{}}' > $out/${ort}/.cargo-checksum.json

                    # ort-sys sub-crate
                    cp -r ort-sys $out/${sys}
                    echo '{"files":{}}' > $out/${sys}/.cargo-checksum.json
                  '';
              }
            else
              drv;
        };

        # Shared build arguments for all crane derivations
        commonArgs = {
          inherit src cargoVendorDir version;
          pname = "voicevox-cli";
          strictDeps = true;
          doCheck = false;
          cargoExtraArgs = "--locked --all-features";

          CARGO_NET_OFFLINE = true;
          ORT_LIB_LOCATION = "${onnxruntimeLibDir}";

          preConfigure = ''
            export HOME=$PWD/build-home
            mkdir -p $HOME
          '';

          preBuild = ''
            export GIT_SSL_CAINFO="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';

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
        };

        # Build only cargo dependencies (shared by build, clippy, tests)
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Voice models and resources downloader
        voicevoxDownloader = pkgs.fetchurl {
          url = "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.3/download-osx-arm64";
          hash = "sha256-7GMosxM4HRDAix6BImNP5Q5PNpWJYEvMLNApKjNht+k=";
        };

        voicevoxResources = pkgs.stdenv.mkDerivation {
          name = "voicevox-resources";

          dontUnpack = true;

          installPhase = ''
            mkdir -p $out/bin
            cp ${voicevoxDownloader} $out/bin/voicevox-download
            chmod +x $out/bin/voicevox-download
          '';
        };

        packageMeta = {
          description = "VOICEVOX CLI for Apple Silicon - Dynamic voice detection system";
          homepage = "https://github.com/usabarashi/voicevox-cli";
          license = with lib.licenses; [
            mit
            asl20
          ];
          platforms = systems;
        };

        # Final package
        voicevoxCli = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;

            postInstall = ''
              cp ${voicevoxResources}/bin/voicevox-download $out/bin/
              install -m755 ${./scripts/voicevox-setup.sh} $out/bin/voicevox-setup
              install -m644 ${./VOICEVOX.md} $out/bin/VOICEVOX.md
            '';

            meta = packageMeta;
          }
        );

        # Development utility: reset daemon state
        voicevoxResetWrapper = pkgs.writeShellScriptBin "voicevox-reset" (
          builtins.readFile ./scripts/voicevox-reset.sh
        );

        mkApp = program: {
          type = "app";
          inherit program;
        };
        appBins = [
          "voicevox-say"
          "voicevox-daemon"
          "voicevox-mcp-server"
        ];
        appAttrs = lib.genAttrs appBins (bin: mkApp "${voicevoxCli}/bin/${bin}");

      in
      {
        packages = rec {
          default = voicevoxCli;
          voicevox-cli = default;
          voicevox-say = default;
          inherit voicevoxResources;
        };

        checks = {
          # Code formatting
          formatting = craneLib.cargoFmt {
            inherit src;
          };

          # Shell script syntax validation
          scripts =
            pkgs.runCommand "check-scripts"
              {
                nativeBuildInputs = with pkgs; [
                  bash
                  gnused
                  gnugrep
                ];
                src = lib.cleanSourceWith {
                  src = ./.;
                  filter =
                    path: type:
                    let
                      baseName = baseNameOf path;
                    in
                    (type == "directory" && baseName == "scripts")
                    || (type == "regular" && lib.hasSuffix ".sh" baseName);
                };
              }
              ''
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
          build = voicevoxCli;

          # Static analysis (reuses cargoArtifacts)
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            }
          );

          # Test suite (reuses cargoArtifacts)
          tests = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              doCheck = true;
            }
          );
        };

        apps = appAttrs // {
          default = appAttrs.voicevox-say;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          # ONNX Runtime library search path (actual library loaded at runtime via dlopen)
          ORT_LIB_LOCATION = "${onnxruntimeLibDir}";

          packages = with pkgs; [
            rustToolchain.rust-analyzer
            cargo-audit

            # Build tools
            pkg-config
            cmake
            gcc

            # Development utilities
            voicevoxResetWrapper

            # TLC model checker for modeling/*.tla
            tlaplus
          ];

          shellHook = ''
            # Work around Nix Darwin cc-wrapper linker conflicts when host/build SDK env vars differ.
            unset DEVELOPER_DIR_FOR_BUILD
            unset SDKROOT_FOR_BUILD
            unset NIX_APPLE_SDK_VERSION_FOR_BUILD

            # Create project-home directory for CARGO_HOME
            mkdir -p .project-home
            export CARGO_HOME="$PWD/.project-home/.cargo"

            echo "VOICEVOX CLI Development Environment (Apple Silicon)"
            echo "Available commands:"
            echo "  cargo build --bin voicevox-say     - Build client"
            echo "  cargo build --bin voicevox-daemon  - Build daemon"
            echo "  cargo run --bin voicevox-say       - Run client"
            echo "  nix build                          - Build with Nix"
            echo "  nix flake check                    - Run formatting/scripts/build/clippy/tests"
            echo "  nix run                            - Run voicevox-say directly"
            echo "  voicevox-reset                     - Reset daemon state (kill processes + remove socket)"
            echo "  cargo test                         - Also works in this shell (FOR_BUILD SDK vars sanitized)"
            echo "  cargo kani                         - Run Kani proofs (install: cargo install --locked kani-verifier && cargo kani setup)"
            echo "  tlc -deadlock -config X.cfg X.tla  - Run TLC model checker (in modeling/)"
            echo ""
            echo "Dynamic voice detection system - no hardcoded voice names"
          '';
        };

      }
    )
    // {
      overlays.default = final: _prev: {
        voicevox-cli = (self.packages.${final.system} or self.packages.aarch64-darwin).voicevox-cli;
        voicevox-say = final.voicevox-cli;
      };

      overlays.voicevox-cli = self.overlays.default;
    };
}
