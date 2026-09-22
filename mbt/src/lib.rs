// Model-based tests for voicevox-cli, driven by Quint Connect.
//
// This crate is intentionally a standalone workspace (see `[workspace]` below)
// so that it is excluded from the root crate's build and from `nix flake check`
// / the crane sandbox. The tests invoke the `quint` CLI at runtime, which is
// provided by the Nix devShell. Run them with:
//
//   nix develop --accept-flake-config --command cargo test --manifest-path mbt/Cargo.toml
