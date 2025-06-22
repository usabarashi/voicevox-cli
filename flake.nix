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

          # Centralized meta information
          packageMeta = with pkgs.lib; {
            description = "VOICEVOX CLI tool for text-to-speech synthesis";
            homepage = "https://github.com/usabarashi/voicevox-cli";
            license = with licenses; [ mit asl20 ];
            maintainers = [ "usabarashi" ];
            platforms = [ "aarch64-darwin" ];
          };

          # Package definition (previously in default.nix)
          voicevox-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "voicevox-cli";
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

            # Copy VOICEVOX Core libraries and include files
            preBuild = ''
              if [ ! -d "voicevox_core" ]; then
                echo "Error: voicevox_core directory not found"
                echo "Please place VOICEVOX Core libraries in voicevox_core/"
                exit 1
              fi
            '';

            # Install the binary as voicevox-say and setup runtime library paths
            postInstall = ''
              mv $out/bin/voicevox-cli $out/bin/voicevox-say || true

              # Copy VOICEVOX libraries to output
              mkdir -p $out/lib
              cp -r voicevox_core/c_api/lib/* $out/lib/
              cp -r voicevox_core/onnxruntime/lib/* $out/lib/

              # Create directories for VOICEVOX data
              mkdir -p $out/share/voicevox/{dict,models}

              # Copy OpenJTalk dictionary from project
              if [ -d "dict" ]; then
                echo "📚 Copying OpenJTalk dictionary from project dict/"
                cp -r dict/* $out/share/voicevox/dict/
                echo "📚 Included OpenJTalk dictionary in package"
              else
                echo "⚠️  OpenJTalk dictionary not found in project dict/"
              fi

              # Copy VVM models from project
              if [ -d "models" ]; then
                echo "🎭 Copying VVM models from project models/"
                cp -r models/* $out/share/voicevox/models/
                echo "🎭 Included VVM models in package"
              else
                echo "⚠️  VVM models not found in project models/"
              fi

              # Set up library path for runtime
              export DYLD_LIBRARY_PATH="$out/lib:''${DYLD_LIBRARY_PATH}"

              # Fix runtime library paths on macOS
              if [[ "$OSTYPE" == "darwin"* ]]; then
                install_name_tool -change \
                  "/Users/runner/work/voicevox_core/voicevox_core/target/aarch64-apple-darwin/release/deps/libvoicevox_core.dylib" \
                  "$out/lib/libvoicevox_core.dylib" \
                  $out/bin/voicevox-say

                # Add rpath for runtime library discovery
                install_name_tool -add_rpath "$out/lib" $out/bin/voicevox-say
              fi
            '';

            # Apply centralized meta information
            meta = packageMeta;
          };
        in
        {
          # Packages for installation
          packages = {
            default = voicevox-cli;
            voicevox-cli = voicevox-cli;
            voicevox-say = voicevox-cli; # alias for compatibility
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
              echo "🫛 VOICEVOX CLI Development Environment"
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
          "🫛 Credit VOICEVOX when using generated audio"
          "📄 Follow individual voice library terms"
          "⚖️  See voicevox_core/onnxruntime/TERMS.txt for details"
          "🚫 Reverse engineering prohibited"
        ];
      };
    };
}
