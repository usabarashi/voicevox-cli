#!/usr/bin/env bash
set -euo pipefail

DOWNLOADER="voicevox-download"
VOICEVOX_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/voicevox"
MODEL_DIR="$VOICEVOX_DIR/models"
ONNX_DIR="$VOICEVOX_DIR/onnxruntime"
DICT_DIR="$VOICEVOX_DIR/openjtalk_dict"
INSTALL_TYPE="user-specific"

echo "Setting up VOICEVOX components..."
echo "Installation directory: $VOICEVOX_DIR ($INSTALL_TYPE)"
echo ""
echo "Note: Checking for required components (models, ONNX Runtime, dictionary)..."
echo "      Missing components will be downloaded automatically"

# Create directories
mkdir -p "$VOICEVOX_DIR"
mkdir -p "$MODEL_DIR"

# Check if downloader is available
if ! command -v "$DOWNLOADER" >/dev/null 2>&1; then
    echo "Error: voicevox-download not found in PATH"
    echo "Please ensure VOICEVOX CLI is properly installed"
    exit 1
fi

# Check each component
NEED_MODELS=false
NEED_ONNX=false
NEED_DICT=false

# Check voice models
if [ ! -d "$MODEL_DIR" ] || [ $(find "$MODEL_DIR" -name "*.vvm" 2>/dev/null | wc -l) -eq 0 ]; then
    NEED_MODELS=true
    echo "Voice models: Not found"
else
    VVM_COUNT=$(find "$MODEL_DIR" -name "*.vvm" 2>/dev/null | wc -l)
    echo "Voice models: $VVM_COUNT files found"
fi

# Check ONNX Runtime
if [ ! -f "$ONNX_DIR/lib/libvoicevox_onnxruntime.dylib" ] && [ ! -f "$ONNX_DIR/lib/libvoicevox_onnxruntime.so" ]; then
    NEED_ONNX=true
    echo "ONNX Runtime: Not found"
else
    echo "ONNX Runtime: Found"
fi

# Check dictionary
if [ ! -d "$DICT_DIR" ] || [ ! -f "$DICT_DIR/sys.dic" ]; then
    NEED_DICT=true
    echo "OpenJTalk dictionary: Not found"
else
    echo "OpenJTalk dictionary: Found"
fi

# Download missing components
ONLY_ARGS=""
if [ "$NEED_MODELS" = true ]; then
    ONLY_ARGS="$ONLY_ARGS --only models"
fi
if [ "$NEED_ONNX" = true ]; then
    ONLY_ARGS="$ONLY_ARGS --only onnxruntime"
fi
if [ "$NEED_DICT" = true ]; then
    ONLY_ARGS="$ONLY_ARGS --only dict"
fi

if [ -n "$ONLY_ARGS" ]; then
    echo ""
    echo "Downloading missing components..."
    "$DOWNLOADER" $ONLY_ARGS --output "$VOICEVOX_DIR" || {
        echo "Download failed. Please try again or download manually"
        exit 1
    }
    echo ""
    echo "Setup completed!"
else
    echo ""
    echo "All components already installed"
fi

echo "You can now use voicevox-say for text-to-speech synthesis"
