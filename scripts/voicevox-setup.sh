#!/usr/bin/env bash
set -euo pipefail

# VOICEVOX Setup Script - Downloads all required resources
# This script downloads:
# - ONNX Runtime library
# - OpenJTalk dictionary
# - Voice models

# Version
VERSION="0.1.0"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Handle --help and --version arguments
case "${1:-}" in
    -h|--help)
        cat <<EOF
VOICEVOX CLI Setup - Downloads required resources

Usage: voicevox-setup [OPTIONS]

Options:
  -h, --help     Show this help message and exit
  -V, --version  Show version information and exit

Environment variables:
  VOICEVOX_DIR           Override data directory location
  XDG_DATA_HOME          XDG base directory (default: ~/.local/share)

This script downloads:
  - ONNX Runtime library
  - OpenJTalk dictionary
  - Voice models
EOF
        exit 0
        ;;
    -V|--version)
        echo "voicevox-setup $VERSION"
        exit 0
        ;;
esac

# Function to create ONNX Runtime compatibility symlinks
create_onnxruntime_symlinks() {
    local lib_dir="$1"
    local symlinks_created=false
    
    if [ ! -d "$lib_dir" ]; then
        return 0
    fi
    
    # Check for macOS .dylib files
    for lib in "$lib_dir"/libvoicevox_onnxruntime.*.dylib; do
        if [ -f "$lib" ]; then
            filename=$(basename "$lib")
            version=${filename#libvoicevox_onnxruntime.}
            version=${version%.dylib}
            symlink_path="$lib_dir/libonnxruntime.$version.dylib"
            
            if [ ! -e "$symlink_path" ]; then
                if [ "$symlinks_created" = false ]; then
                    echo -e "${BLUE}Creating compatibility symlinks...${NC}"
                    symlinks_created=true
                fi
                ln -sf "$filename" "$symlink_path"
                echo "  Created: libonnxruntime.$version.dylib -> $filename"
                
                # Also create version-less symlink
                versionless_path="$lib_dir/libonnxruntime.dylib"
                if [ ! -e "$versionless_path" ]; then
                    ln -sf "$filename" "$versionless_path"
                    echo "  Created: libonnxruntime.dylib -> $filename"
                fi
            fi
        fi
    done
    
    # Check for Linux .so files
    for lib in "$lib_dir"/libvoicevox_onnxruntime.*.so; do
        if [ -f "$lib" ]; then
            filename=$(basename "$lib")
            version=${filename#libvoicevox_onnxruntime.}
            version=${version%.so}
            symlink_path="$lib_dir/libonnxruntime.$version.so"
            
            if [ ! -e "$symlink_path" ]; then
                if [ "$symlinks_created" = false ]; then
                    echo -e "${BLUE}Creating compatibility symlinks...${NC}"
                    symlinks_created=true
                fi
                ln -sf "$filename" "$symlink_path"
                echo "  Created: libonnxruntime.$version.so -> $filename"
                
                # Also create version-less symlink
                versionless_path="$lib_dir/libonnxruntime.so"
                if [ ! -e "$versionless_path" ]; then
                    ln -sf "$filename" "$versionless_path"
                    echo "  Created: libonnxruntime.so -> $filename"
                fi
            fi
        fi
    done
    
    if [ "$symlinks_created" = true ]; then
        echo ""
    fi
}

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

echo -e "${BLUE}VOICEVOX CLI Setup${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "Data directory: $DATA_DIR"
echo "(Following XDG Base Directory specification)"
echo ""

# Check what's already installed
MISSING_RESOURCES=()

# Check ONNX Runtime
ONNX_FOUND=false
if [ -n "${ORT_DYLIB_PATH:-}" ] && [ -f "$ORT_DYLIB_PATH" ]; then
    echo -e "${GREEN}[OK]${NC} ONNX Runtime: Found at $ORT_DYLIB_PATH"
    ONNX_FOUND=true
elif [ -f "$DATA_DIR/lib/libonnxruntime.dylib" ] || [ -f "$DATA_DIR/lib/libonnxruntime.so" ]; then
    echo -e "${GREEN}[OK]${NC} ONNX Runtime: Already installed"
    ONNX_FOUND=true
elif [ -d "$DATA_DIR/onnxruntime/lib" ]; then
    # Check for VOICEVOX ONNX Runtime files
    if find "$DATA_DIR/onnxruntime/lib" -name "libvoicevox_onnxruntime.*.dylib" -o -name "libvoicevox_onnxruntime.*.so" -o -name "libonnxruntime.*" 2>/dev/null | grep -q .; then
        echo -e "${GREEN}[OK]${NC} ONNX Runtime: Already installed"
        ONNX_FOUND=true
    fi
fi

if [ "$ONNX_FOUND" = false ]; then
    echo -e "${YELLOW}[ ]${NC} ONNX Runtime: Not installed"
    MISSING_RESOURCES+=("onnxruntime")
fi

# Check OpenJTalk dictionary
DICT_FOUND=false
if [ -n "${VOICEVOX_OPENJTALK_DICT:-}" ] && [ -d "$VOICEVOX_OPENJTALK_DICT" ]; then
    echo -e "${GREEN}[OK]${NC} OpenJTalk Dictionary: Found at $VOICEVOX_OPENJTALK_DICT"
    DICT_FOUND=true
elif [ -d "$DATA_DIR/openjtalk_dict" ]; then
    echo -e "${GREEN}[OK]${NC} OpenJTalk Dictionary: Already installed"
    DICT_FOUND=true
elif [ -d "$DATA_DIR/dict" ]; then
    # Check for VOICEVOX dictionary files
    if find "$DATA_DIR/dict" -name "open_jtalk_dic_*" -type d 2>/dev/null | grep -q .; then
        echo -e "${GREEN}[OK]${NC} OpenJTalk Dictionary: Already installed"
        DICT_FOUND=true
    fi
fi

if [ "$DICT_FOUND" = false ]; then
    echo -e "${YELLOW}[ ]${NC} OpenJTalk Dictionary: Not installed"
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
    echo -e "${GREEN}[OK]${NC} Voice Models: $MODEL_COUNT models installed"
else
    echo -e "${YELLOW}[ ]${NC} Voice Models: Not installed"
    MISSING_RESOURCES+=("models")
fi

echo ""

# Check if everything is already installed
if [ ${#MISSING_RESOURCES[@]} -eq 0 ]; then
    echo -e "${GREEN}All resources are already installed!${NC}"
    echo ""
    
    # Even if everything is installed, check and create compatibility symlinks if needed
    create_onnxruntime_symlinks "$DATA_DIR/onnxruntime/lib"
    
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

# Build the --only arguments array
ONLY_ARGS=()
for resource in "${MISSING_RESOURCES[@]}"; do
    ONLY_ARGS+=(--only "$resource")
done

# Download resources
echo -e "${BLUE}Downloading resources...${NC}"
echo "Running: $DOWNLOADER ${ONLY_ARGS[*]} --output $DATA_DIR"
echo ""

if $DOWNLOADER "${ONLY_ARGS[@]}" --output "$DATA_DIR"; then
    echo ""
    echo -e "${GREEN}All resources downloaded successfully!${NC}"
    echo ""
    
    # Create symlink for ONNX Runtime compatibility if needed
    if [[ " ${MISSING_RESOURCES[@]} " =~ " onnxruntime " ]]; then
        create_onnxruntime_symlinks "$DATA_DIR/onnxruntime/lib"
    fi
    
    # Set up environment variable hints
    echo "Installation complete!"
    echo ""
    echo "Resources installed to: $DATA_DIR"
    echo ""
    
    # No environment variables needed - compatibility symlinks handle everything
    
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
    echo -e "${RED}Resource download failed${NC}"
    echo ""
    echo "You can try running the download manually:"
    echo "  $DOWNLOADER ${ONLY_ARGS[*]} --output $DATA_DIR"
    echo ""
    echo "Or download individual components:"
    echo "  $DOWNLOADER --only onnxruntime --output $DATA_DIR"
    echo "  $DOWNLOADER --only dict --output $DATA_DIR"
    echo "  $DOWNLOADER --only models --output $DATA_DIR"
    exit 1
fi