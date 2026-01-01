# Contributing to VOICEVOX CLI

Thank you for your interest in contributing to VOICEVOX CLI! This document provides guidelines and information for contributors.

## Branch Strategy

We use a Git Flow-inspired branching model:

### Main Branches

- **`main`**: Stable releases (default branch for users)
  - Used by Nix flake inputs: `inputs.voicevox-cli.url = "github:usabarashi/voicevox-cli"`
  - Tagged releases are created from this branch
  - Only accepts PRs from `develop` or `hotfix/*` branches
  - Protected by branch restrictions workflow

- **`develop`**: Active development (default for contributors)
  - Use `git checkout develop` when starting development
  - All feature branches merge into `develop`
  - Dependabot and automated updates target this branch
  - Periodically merged into `main` for releases

### Supporting Branches

- **Feature branches**: `feature/<description>` or `<type>/<description>`
  - Created from `develop`
  - Merged back into `develop` via Pull Request

- **Hotfix branches**: `hotfix/<description>`
  - Created from `main` for urgent production fixes
  - Can be merged directly to `main`

## Development Workflow

### 1. Setup Development Environment

```bash
# Clone repository
git clone https://github.com/usabarashi/voicevox-cli
cd voicevox-cli

# Checkout develop branch
git checkout develop

# Enter Nix development shell
nix develop

# Build and test
nix build
nix run . -- "テストメッセージなのだ"
```

### 2. Create a Feature Branch

```bash
# Create a new branch from develop
git checkout develop
git pull origin develop
git checkout -b feature/your-feature-name
```

### 3. Make Changes

- Follow existing code style and conventions
- Write clear, descriptive commit messages
- Add tests for new functionality
- Update documentation as needed

### 4. Pre-commit Checks

**CRITICAL**: Run these checks before every commit to catch errors early:

```bash
# Format check (auto-fix with: cargo fmt)
nix develop -c cargo fmt --check

# Clippy with all targets and features (same as CI)
nix develop -c cargo clippy --all-targets --all-features -- -D warnings

# Unit tests
nix develop -c cargo test --lib
```

### 5. Submit a Pull Request

**PR Title Format:**
```
<type>: <description>

Examples:
- feat: add new voice synthesis feature
- fix: resolve daemon startup issue
- ci: improve release workflow
- docs: update installation guide
```

**PR Template:**
```markdown
# Why
<!-- Why are these changes needed? -->

# What
<!-- What changes are being made? -->

# References
<!-- Related issues, discussions, or PRs -->
```

**Target Branch:**
- Feature PRs → `develop`
- Hotfix PRs → `main` (emergency fixes only)

### 6. Code Review

- Address review comments promptly
- Keep discussions focused and constructive
- Update your PR based on feedback

## Testing

### Running Tests Locally

```bash
# 1. Pre-commit checks (required before every commit)
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets --all-features -- -D warnings
nix develop -c cargo test --lib

# 2. Build with Nix
nix build

# 3. Verify binaries
cargo test --test verify_binaries -- --nocapture

# 4. Run protocol tests
cargo test --test mcp_protocol

# 5. Start daemon for synthesis tests
./target/debug/voicevox-daemon --start --detach
sleep 2

# 6. Run synthesis tests (requires daemon)
cargo test --test synthesis_modes --ignored
```

### CI/CD Pipeline

All PRs run through automated checks:

1. **Format check**: `cargo fmt --check`
2. **Clippy analysis**: `cargo clippy --all-targets --all-features -- -D warnings`
3. **Unit tests**: `cargo test --lib`
4. **Build verification**: `nix build` produces release artifacts
5. **Integration tests**: MCP protocol tests against release build
6. **Script validation**: Shell script syntax checks
7. **Security audit**: `cargo audit`

## Release Process

Releases are automated when changes are merged to `main`:

1. Changes merge from `develop` to `main` (via sync PR)
2. Release workflow creates timestamped tag (vYYYYMMDDHHMMSS)
3. Nix build produces release artifacts
4. Release tarball is created and tested
5. GitHub Release is published with auto-generated notes

## Code Style

- **Rust**: Follow standard Rust conventions (enforced by `rustfmt` and `clippy`)
- **Nix**: Follow Nixpkgs style guide
- **Shell scripts**: Use ShellCheck-compliant syntax
- **Commit messages**: Use conventional commits format

## Questions or Issues?

- Open an issue for bugs or feature requests
- Start a discussion for questions or ideas
- Check existing issues before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (MIT OR Apache-2.0).

---

Thank you for contributing to VOICEVOX CLI! 🎉
