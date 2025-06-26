#!/bin/bash
set -euo pipefail

DOWNLOADER="voicevox-download"

# Determine target directory - prefer system-wide if writable
if [ -d "/usr/local/share/voicevox" ] && [ -w "/usr/local/share/voicevox" ]; then
    VOICEVOX_DIR="/usr/local/share/voicevox"
    MODEL_DIR="$VOICEVOX_DIR/models"
    INSTALL_TYPE="system-wide"
elif [ -d "/opt/voicevox" ] && [ -w "/opt/voicevox" ]; then
    VOICEVOX_DIR="/opt/voicevox"
    MODEL_DIR="$VOICEVOX_DIR/models"
    INSTALL_TYPE="system-wide"
else
    VOICEVOX_DIR="$HOME/.local/share/voicevox"
    MODEL_DIR="$VOICEVOX_DIR/models"
    INSTALL_TYPE="user-specific"
fi

echo "Setting up VOICEVOX voice models..."
echo "Models will be downloaded to: $MODEL_DIR ($INSTALL_TYPE)"
echo ""
echo "Note: VOICEVOX Core, ONNX Runtime, and dictionary are statically linked"
echo "      Only voice model files (.vvm) will be downloaded"

# Create VOICEVOX directory (models will be created within)
mkdir -p "$VOICEVOX_DIR"

# Check if downloader is available
if ! command -v "$DOWNLOADER" >/dev/null 2>&1; then
    echo "Error: voicevox-download not found in PATH"
    echo "Please ensure VOICEVOX CLI is properly installed"
    exit 1
fi

# Check if any models are already present
VVM_COUNT=$(find "$MODEL_DIR" -name "*.vvm" 2>/dev/null | wc -l)

if [ "$VVM_COUNT" -gt 0 ]; then
    echo "Voice models already installed ($VVM_COUNT models found)"
    echo "Models found in: $MODEL_DIR"
    ls -la "$MODEL_DIR"/*.vvm 2>/dev/null
    echo "Use --list-models to see available models"
    exit 0
fi

echo "No voice models found. Starting download..."
echo "Downloading voice models only (VVM files)..."

# Use VOICEVOX downloader with --only models to download voice models only
echo "Using VOICEVOX Core downloader (models only)..."
"$DOWNLOADER" --only models --output "$VOICEVOX_DIR" || {
    echo "Download failed. Please try again or download manually"
    echo "Voice models should be placed in: $MODEL_DIR"
    exit 1
}

echo "Voice model setup completed!"
echo "You can now use voicevox-say for text-to-speech synthesis"