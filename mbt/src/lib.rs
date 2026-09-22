// Model-based tests for voicevox-cli, driven by Quint Connect.
//
// This crate holds only integration-test drivers (in `tests/`): each file
// defines a `Driver` (maps spec actions to production calls), a `State`
// (maps production observations to spec state), and the `#[quint_run]` entry
// point. There is no library code here.
//
// It is a member of the root workspace (sharing `Cargo.lock`) but is excluded
// from `default-members`, so the normal root build and `nix flake check` never
// need `quint`. The tests invoke the `quint` CLI at runtime, which is provided
// by the Nix devShell. Run them with:
//
//   nix develop --accept-flake-config --command cargo test --manifest-path mbt/Cargo.toml
