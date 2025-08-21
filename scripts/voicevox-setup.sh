#!/usr/bin/env bash
set -euo pipefail

# VOICEVOX Setup Script - Downloads all required resources
# This script downloads:
# - ONNX Runtime library
# - OpenJTalk dictionary
# - Voice models

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Determine the data directory following XDG Base Directory specification
if [ -n "${VOICEVOX_DIR:-}" ]; then
    DATA_DIR="$VOICEVOX_DIR"
elif [ -n "${XDG_DATA_HOME:-}" ]; then
    DATA_DIR="$XDG_DATA_HOME/voicevox"
elif [ -n "${HOME:-}" ]; then
    DATA_DIR="$HOME/.local/share/voicevox"
else
    DATA_DIR="./voicevox"
fi

echo -e "${BLUE}🎭 VOICEVOX CLI Setup${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "Data directory: $DATA_DIR"
echo "(Following XDG Base Directory specification)"
echo ""

# Check what's already installed
MISSING_RESOURCES=()

# Check ONNX Runtime
if [ -n "${ORT_DYLIB_PATH:-}" ] && [ -f "$ORT_DYLIB_PATH" ]; then
    echo -e "${GREEN}✓${NC} ONNX Runtime: Found at $ORT_DYLIB_PATH"
elif [ -f "$DATA_DIR/lib/libonnxruntime.dylib" ] || [ -f "$DATA_DIR/lib/libonnxruntime.so" ]; then
    echo -e "${GREEN}✓${NC} ONNX Runtime: Already installed"
else
    echo -e "${YELLOW}○${NC} ONNX Runtime: Not installed"
    MISSING_RESOURCES+=("onnxruntime")
fi

# Check OpenJTalk dictionary
if [ -n "${VOICEVOX_OPENJTALK_DICT:-}" ] && [ -d "$VOICEVOX_OPENJTALK_DICT" ]; then
    echo -e "${GREEN}✓${NC} OpenJTalk Dictionary: Found at $VOICEVOX_OPENJTALK_DICT"
elif [ -d "$DATA_DIR/openjtalk_dict" ]; then
    echo -e "${GREEN}✓${NC} OpenJTalk Dictionary: Already installed"
else
    echo -e "${YELLOW}○${NC} OpenJTalk Dictionary: Not installed"
    MISSING_RESOURCES+=("dict")
fi

# Check voice models
MODEL_COUNT=0
if [ -d "$DATA_DIR/models/vvms" ]; then
    MODEL_COUNT=$(find "$DATA_DIR/models/vvms" -name "*.vvm" 2>/dev/null | wc -l)
elif [ -d "$DATA_DIR/models" ]; then
    MODEL_COUNT=$(find "$DATA_DIR/models" -name "*.vvm" 2>/dev/null | wc -l)
fi

if [ "$MODEL_COUNT" -gt 0 ]; then
    echo -e "${GREEN}✓${NC} Voice Models: $MODEL_COUNT models installed"
else
    echo -e "${YELLOW}○${NC} Voice Models: Not installed"
    MISSING_RESOURCES+=("models")
fi

echo ""

# Check if everything is already installed
if [ ${#MISSING_RESOURCES[@]} -eq 0 ]; then
    echo -e "${GREEN}✅ All resources are already installed!${NC}"
    echo ""
    echo "Installation directory: $DATA_DIR"
    echo ""
    echo "You can now use:"
    echo "  voicevox-say \"こんにちは\"     # Text-to-speech"
    echo "  voicevox-daemon --start       # Start daemon"
    exit 0
fi

# Show what needs to be downloaded
echo -e "${YELLOW}The following resources need to be downloaded:${NC}"
echo ""
if [[ " ${MISSING_RESOURCES[@]} " =~ " onnxruntime " ]]; then
    echo "  • ONNX Runtime"
    echo "    Neural network inference engine"
fi
if [[ " ${MISSING_RESOURCES[@]} " =~ " dict " ]]; then
    echo "  • OpenJTalk Dictionary"
    echo "    Japanese text processing data"
fi
if [[ " ${MISSING_RESOURCES[@]} " =~ " models " ]]; then
    echo "  • Voice Models"
    echo "    Character voices"
fi

echo ""
echo -e "${BLUE}Target directory: $DATA_DIR${NC}"
echo ""

# Prompt for confirmation
read -p "Would you like to download these resources now? [Y/n]: " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]] && [[ ! -z "$REPLY" ]]; then
    echo ""
    echo "Setup cancelled."
    echo "You can run this script again later to complete the setup."
    exit 1
fi

# Create data directory
echo ""
echo -e "${BLUE}Creating directories...${NC}"
mkdir -p "$DATA_DIR"

# Find the voicevox-download binary
DOWNLOADER=""
if command -v voicevox-download &> /dev/null; then
    DOWNLOADER="voicevox-download"
elif [ -f "${0%/*}/voicevox-download" ]; then
    DOWNLOADER="${0%/*}/voicevox-download"
elif [ -f "./voicevox-download" ]; then
    DOWNLOADER="./voicevox-download"
else
    echo -e "${RED}Error: voicevox-download binary not found${NC}"
    echo "Please ensure voicevox-download is in your PATH or in the same directory as this script"
    exit 1
fi

# Build the --only argument
ONLY_ARG=""
for resource in "${MISSING_RESOURCES[@]}"; do
    if [ -z "$ONLY_ARG" ]; then
        ONLY_ARG="$resource"
    else
        ONLY_ARG="$ONLY_ARG,$resource"
    fi
done

# Download resources
echo -e "${BLUE}Downloading resources...${NC}"
echo "Running: $DOWNLOADER --only $ONLY_ARG --output $DATA_DIR"
echo ""

if $DOWNLOADER --only "$ONLY_ARG" --output "$DATA_DIR"; then
    echo ""
    echo -e "${GREEN}✅ All resources downloaded successfully!${NC}"
    echo ""
    
    # Set up environment variable hints
    echo "Installation complete!"
    echo ""
    echo "Resources installed to: $DATA_DIR"
    echo ""
    
    # Check if we need to set environment variables
    if [[ " ${MISSING_RESOURCES[@]} " =~ " onnxruntime " ]]; then
        if [ -f "$DATA_DIR/lib/libonnxruntime.dylib" ]; then
            echo "To use ONNX Runtime, you may need to set:"
            echo "  export ORT_DYLIB_PATH=\"$DATA_DIR/lib/libonnxruntime.dylib\""
        elif [ -f "$DATA_DIR/lib/libonnxruntime.so" ]; then
            echo "To use ONNX Runtime, you may need to set:"
            echo "  export ORT_DYLIB_PATH=\"$DATA_DIR/lib/libonnxruntime.so\""
        fi
        echo ""
    fi
    
    echo "You can now use:"
    echo "  voicevox-say \"こんにちは\"     # Text-to-speech"
    echo "  voicevox-daemon --start       # Start daemon"
    echo ""
    
    # Clean up any temporary files
    echo -e "${BLUE}Cleaning up temporary files...${NC}"
    find "$DATA_DIR" -name "*.tar.gz" -o -name "*.zip" -o -name "*.tgz" 2>/dev/null | while read -r file; do
        rm -f "$file" && echo "  Removed: $(basename "$file")"
    done
    
    echo ""
    echo -e "${GREEN}Setup complete!${NC}"
else
    echo ""
    echo -e "${RED}❌ Resource download failed${NC}"
    echo ""
    echo "You can try running the download manually:"
    echo "  $DOWNLOADER --only $ONLY_ARG --output $DATA_DIR"
    echo ""
    echo "Or download individual components:"
    echo "  $DOWNLOADER --only onnxruntime --output $DATA_DIR"
    echo "  $DOWNLOADER --only dict --output $DATA_DIR"
    echo "  $DOWNLOADER --only models --output $DATA_DIR"
    exit 1
fi