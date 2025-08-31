# Serena MCP server configuration for VOICEVOX CLI
{
  pkgs,
  rustToolchain,
}:
let
  # Common Serena environment setup script
  serenaEnvSetup = ''
    # Get the directory where this script is invoked from
    PROJECT_DIR="$(pwd)"

    # Create fake home directory structure in project
    export HOME="$PROJECT_DIR/.project-home"
    export XDG_DATA_HOME="$HOME/.local/share"
    export XDG_CACHE_HOME="$HOME/.cache"
    export UV_CACHE_DIR="$HOME/.cache/uv"
    export UV_TOOL_DIR="$HOME/.local/uv/tools"
    export CARGO_HOME="$PROJECT_DIR/.project-home/.cargo"

    # Create necessary directories
    mkdir -p "$HOME/.serena/logs"
    mkdir -p "$XDG_DATA_HOME/uv"
    mkdir -p "$XDG_CACHE_HOME"
  '';

  # Helper function to run uvx with Serena
  runSerenaCommand = ''
    exec ${pkgs.uv}/bin/uvx \
        --cache-dir "$UV_CACHE_DIR" \
        --from git+https://github.com/oraios/serena \
        serena "$@"
  '';

  # Serena index creation wrapper
  serenaIndexWrapper = pkgs.writeShellScriptBin "serena-index" ''
    ${serenaEnvSetup}
    
    # Add rust-analyzer to PATH for Serena
    export PATH="${rustToolchain.rust-analyzer}/bin:$PATH"

    echo "Creating Serena index for project..."
    echo "HOME: $HOME"
    echo "Project: $PROJECT_DIR"
    echo "rust-analyzer: $(which rust-analyzer || echo 'not found')"

    # Run serena index command with all paths pointing to project directory
    exec ${pkgs.uv}/bin/uvx \
        --cache-dir "$UV_CACHE_DIR" \
        --from git+https://github.com/oraios/serena \
        serena project index
  '';

  # Serena MCP server wrapper with project-local paths
  serenaMcpWrapper = pkgs.writeShellScriptBin "serena-mcp-wrapper" ''
    ${serenaEnvSetup}
    
    # Add rust-analyzer to PATH for Serena
    export PATH="${rustToolchain.rust-analyzer}/bin:$PATH"

    echo "Starting Serena MCP server with project-local paths..."
    echo "HOME: $HOME"
    echo "Project: $PROJECT_DIR"
    echo "rust-analyzer: $(which rust-analyzer || echo 'not found')"

    # Run serena with all paths pointing to project directory
    exec ${pkgs.uv}/bin/uvx \
        --cache-dir "$UV_CACHE_DIR" \
        --from git+https://github.com/oraios/serena \
        serena start-mcp-server \
        --context ide-assistant \
        --enable-web-dashboard false \
        --project "$PROJECT_DIR"
  '';

  # Serena memory management wrapper
  serenaMemoryWrapper = pkgs.writeShellScriptBin "serena-memory" ''
    set -euo pipefail

    ${serenaEnvSetup}

    # Handle memory commands
    case "''${1:-}" in
      write)
        if [ "$#" -lt 3 ]; then
          echo "Error: write command requires at least 2 arguments" >&2
          echo "Usage: serena-memory write <memory-name> <content>" >&2
          exit 1
        fi
        MEMORY_NAME="$2"
        echo "Writing memory: $MEMORY_NAME"
        # Shift twice to get all remaining args as content
        shift 2
        ${runSerenaCommand} memory write "$MEMORY_NAME" "$*"
        ;;
      read)
        if [ "$#" -lt 2 ]; then
          echo "Error: read command requires 1 argument" >&2
          echo "Usage: serena-memory read <memory-name>" >&2
          exit 1
        fi
        ${runSerenaCommand} memory read "$2"
        ;;
      list)
        ${runSerenaCommand} memory list
        ;;
      delete)
        if [ "$#" -lt 2 ]; then
          echo "Error: delete command requires 1 argument" >&2
          echo "Usage: serena-memory delete <memory-name>" >&2
          exit 1
        fi
        echo "Deleting memory: $2"
        ${runSerenaCommand} memory delete "$2"
        ;;
      *)
        echo "Serena Memory Management"
        echo ""
        echo "Usage:"
        echo "  serena-memory write <name> <content>  - Save a memory"
        echo "  serena-memory read <name>             - Read a memory"
        echo "  serena-memory list                    - List all memories"
        echo "  serena-memory delete <name>           - Delete a memory"
        echo ""
        echo "Example:"
        echo "  serena-memory write architecture 'This project uses daemon-client model'"
        exit 1
        ;;
    esac
  '';
in
{
  wrappers = {
    serenaIndexWrapper = serenaIndexWrapper;
    serenaMcpWrapper = serenaMcpWrapper;
    serenaMemoryWrapper = serenaMemoryWrapper;
  };
  
  packages = [
    pkgs.uv
    serenaIndexWrapper
    serenaMcpWrapper
    serenaMemoryWrapper
  ];
}