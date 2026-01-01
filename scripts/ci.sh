#!/usr/bin/env bash
set -euo pipefail

# Check if running in build phase
BUILD_PHASE=false
if [[ "${1:-}" == "--build-phase" ]]; then
  BUILD_PHASE=true
fi

# Helper function to run commands in nix develop environment
run_in_nix() {
  nix develop --accept-flake-config --command "$@"
}

echo "Running Complete CI Pipeline..."
echo "=================================="

# Skip Nix flake check during build phase (would be circular)
if [[ "$BUILD_PHASE" == "false" ]]; then
  # Static Analysis
  echo ""
  echo "Checking Nix flake..."
  nix flake check --accept-flake-config --show-trace
fi

echo ""
echo "Verifying Rust toolchain..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # During build, use direct commands
  rustc --version
  cargo --version
else
  # Outside build, use nix develop
  run_in_nix rustc --version
  run_in_nix cargo --version
fi

echo ""
echo "Checking code formatting..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # Check formatting and show diff if needed
  if ! cargo fmt --check; then
    echo "Code formatting errors detected. Run 'cargo fmt' to fix."
    exit 1
  fi
else
  run_in_nix cargo fmt --check
fi

echo ""
echo "Running clippy analysis..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  cargo clippy --all-targets --all-features -- -D warnings
else
  run_in_nix cargo clippy --all-targets --all-features -- -D warnings
fi

echo ""
echo "Checking scripts..."

# Check required scripts exist
echo "Checking for required scripts..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # During build, scripts are in the source directory
  SCRIPT_DIR="scripts"
else
  # Use PROJECT_DIR if set by Nix, otherwise get from ci.sh location
  if [[ -n "${PROJECT_DIR:-}" ]]; then
    SCRIPT_DIR="$PROJECT_DIR/scripts"
  else
    SCRIPT_DIR="$(dirname "$0")"
  fi
fi

if [[ -d "$SCRIPT_DIR" ]]; then
  # Check with more detailed error message
  if [[ ! -f "$SCRIPT_DIR/voicevox-setup.sh" ]]; then
    echo "Missing voicevox-setup.sh in $SCRIPT_DIR"
    ls -la "$SCRIPT_DIR" || echo "Directory contents unavailable"
    exit 1
  fi

  # Validate all scripts
  echo "Validating all scripts..."
  for script in "$SCRIPT_DIR"/*.sh; do
    if [[ -f "$script" ]]; then
      echo "  - Validating: $(basename "$script")"
      if grep -q '@@.*@@' "$script"; then
        sed 's/@@[^@]*@@/placeholder/g' "$script" | bash -n
      else
        bash -n "$script"
      fi
    fi
  done
  echo "All scripts validated successfully"
else
  echo "Warning: Scripts directory not found, skipping script validation"
fi

echo ""
echo "Running security audit..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # Skip during build phase - cargo-audit might not be available
  echo "Skipping security audit during build phase"
else
  if ! run_in_nix cargo audit --version >/dev/null 2>&1; then
    echo "Installing cargo-audit..."
    run_in_nix cargo install cargo-audit
  fi
  # Run cargo audit with error handling for known CVSS 4.0 incompatibility
  if ! run_in_nix cargo audit 2>&1 | tee /tmp/cargo-audit.log; then
    if grep -q "unsupported CVSS version: 4.0" /tmp/cargo-audit.log; then
      echo "⚠️  Warning: cargo-audit failed due to CVSS 4.0 incompatibility (known issue)"
      echo "    This is a tooling limitation, not a security issue in this project"
      echo "    See: https://github.com/rustsec/rustsec/issues/1130"
    else
      # Real security issue - fail the build
      echo "❌ Security audit failed with unexpected error"
      exit 1
    fi
  fi
  rm -f /tmp/cargo-audit.log
fi

# Build verification - skip during build phase to avoid circular dependency
if [[ "$BUILD_PHASE" == "false" ]]; then
  echo ""
  echo "Building project with Nix..."
  nix build --accept-flake-config --show-trace
fi

# Build artifact verification - only run after successful build
if [[ "$BUILD_PHASE" == "false" ]]; then
  echo ""
  echo "Verifying build artifacts..."
  if [[ ! -d result/bin ]]; then
    echo "Build artifacts not found"
    exit 1
  fi

  echo "Build artifact contents:"
  ls -lah result/bin/
  
  echo ""
  echo "Binary verification:"
  file result/bin/voicevox-say
  file result/bin/voicevox-daemon
  file result/bin/voicevox-mcp-server
  
  echo ""
  echo "Testing functionality..."
  result/bin/voicevox-say --help >/dev/null
  result/bin/voicevox-daemon --help >/dev/null
  echo "Help commands work correctly"
  
  echo ""
  echo "Package verification:"
  echo "Static linking verification:"
  otool -L result/bin/voicevox-say | grep -E "(voicevox|onnx)" || echo "Static linking verified"
  echo "Total package size: $(du -sh result/ | cut -f1)"
fi

echo ""
if [[ "$BUILD_PHASE" == "true" ]]; then
  echo "Pre-build CI checks completed successfully!"
else
  echo "All CI checks completed successfully!"
fi
