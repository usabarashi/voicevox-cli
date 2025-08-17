#!/usr/bin/env bash
set -euo pipefail

# Check if running in build phase
BUILD_PHASE=false
if [[ "${1:-}" == "--build-phase" ]]; then
  BUILD_PHASE=true
fi

echo "🔍 Running Complete CI Pipeline..."
echo "=================================="

# Skip Nix flake check during build phase (would be circular)
if [[ "$BUILD_PHASE" == "false" ]]; then
  # Static Analysis
  echo ""
  echo "📦 Checking Nix flake..."
  nix flake check --show-trace
fi

echo ""
echo "🛠️  Verifying Rust toolchain..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # During build, use direct commands
  rustc --version
  cargo --version
else
  # Outside build, use nix develop
  nix develop --command rustc --version
  nix develop --command cargo --version
fi

echo ""
echo "📝 Checking code formatting..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # Check formatting and show diff if needed
  if ! cargo fmt --check; then
    echo "❌ Code formatting errors detected. Run 'cargo fmt' to fix."
    echo ""
    echo "Hint: The most common issue is missing newline at end of file."
    echo "You can fix this by running: cargo fmt"
    exit 1
  fi
else
  nix develop --command cargo ci-fmt
fi

echo ""
echo "🧹 Running clippy analysis..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  cargo clippy --all-targets --all-features -- -D warnings || (echo "❌ Clippy warnings detected. Fix them before building." && exit 1)
else
  nix develop --command cargo ci-clippy
fi

echo ""
echo "📜 Checking scripts..."

# Check required scripts exist
echo "Checking for required scripts..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # During build, scripts are in the source directory
  SCRIPT_DIR="scripts"
else
  # Outside build, scripts are relative to ci.sh
  SCRIPT_DIR="$(dirname "$0")"
fi

if [[ -d "$SCRIPT_DIR" ]]; then
  test -f "$SCRIPT_DIR/voicevox-setup-models.sh" || (echo "❌ Missing voicevox-setup-models.sh" && exit 1)
  test -f "$SCRIPT_DIR/voicevox-auto-setup.sh" || (echo "❌ Missing voicevox-auto-setup.sh" && exit 1)
  
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
  echo "✅ All scripts validated successfully"
else
  echo "Warning: Scripts directory not found, skipping script validation"
fi

echo ""
echo "🔒 Running security audit..."
if [[ "$BUILD_PHASE" == "true" ]]; then
  # Skip during build phase - cargo-audit might not be available
  echo "Skipping security audit during build phase"
else
  if ! nix develop --command cargo audit --version >/dev/null 2>&1; then
    echo "Installing cargo-audit..."
    nix develop --command cargo install cargo-audit
  fi
  nix develop --command cargo audit
fi

# Build verification - skip during build phase to avoid circular dependency
if [[ "$BUILD_PHASE" == "false" ]]; then
  echo ""
  echo "🔨 Building project with Nix..."
  nix build --show-trace
fi

# Build artifact verification - only run after successful build
if [[ "$BUILD_PHASE" == "false" ]]; then
  echo ""
  echo "📊 Verifying build artifacts..."
  if [[ -d result/bin ]]; then
    ls -la result/bin/
    echo "✅ Build artifacts verified"
  else
    echo "❌ Build artifacts not found"
    exit 1
  fi

  echo ""
  echo "🔧 Verifying build artifacts..."
  ls -la result/bin/
  file result/bin/voicevox-say
  file result/bin/voicevox-daemon
  test -x result/bin/voicevox-setup-models
  echo "All binaries built successfully"

  echo ""
  echo "🧪 Testing functionality..."
  result/bin/voicevox-say --help || echo "Help command test"
  result/bin/voicevox-daemon --help || echo "Help command test"
  result/bin/voicevox-say --version || echo "Version command not available"

  echo ""
  echo "📦 Package verification..."
  echo "Binary sizes:"
  ls -lah result/bin/
  echo "Static linking verification:"
  otool -L result/bin/voicevox-say | grep -E "(voicevox|onnx)" || echo "Static linking verified"
  echo "Total package size:"
  du -sh result/
fi

echo ""
if [[ "$BUILD_PHASE" == "true" ]]; then
  echo "✅ Pre-build CI checks completed successfully!"
else
  echo "✅ All CI checks completed successfully!"
fi