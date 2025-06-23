{ lib
, stdenv
, rustPlatform
, pkg-config
, darwin
, meta ? { }
}:

rustPlatform.buildRustPackage rec {
  pname = "voicevox-cli";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  # Skip tests since they require VOICEVOX runtime libraries
  doCheck = false;

  nativeBuildInputs = [
    pkg-config
  ] ++ lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.AudioUnit
    darwin.apple_sdk.frameworks.CoreAudio
    darwin.apple_sdk.frameworks.CoreServices
  ];

  buildInputs = lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.AudioUnit
    darwin.apple_sdk.frameworks.CoreAudio
    darwin.apple_sdk.frameworks.CoreServices
  ];

  # Copy VOICEVOX Core libraries and include files
  preBuild = ''
    if [ ! -d "voicevox_core" ]; then
      echo "Error: voicevox_core directory not found"
      echo "Please place VOICEVOX Core libraries in voicevox_core/"
      exit 1
    fi
  '';

  # Install the binary as voicevox-say
  postInstall = ''
    mv $out/bin/voicevox-cli $out/bin/voicevox-say || true
  '';

  # Meta information passed from flake.nix
  inherit meta;
}
