#!@@BASH_PATH@@
set -euo pipefail

MODELS_DIR="$1"

echo "🎭 VOICEVOX CLI - Voice Models Setup"
echo "Setting up voice models for current user..."
echo "Note: VOICEVOX Core libraries and ONNX Runtime are statically linked"
echo ""
echo "By using this Nix package, you agree to:"
echo "- Individual voice library terms for 26+ characters (credit required: 'VOICEVOX:[Character]')"
echo "- See: https://voicevox.hiroshiba.jp/ for details"
echo ""
echo "Target: $MODELS_DIR (user-specific)"
echo "Download size: ~200MB (voice models only)"
echo ""

mkdir -p "$(dirname "$MODELS_DIR")"
mkdir -p "$MODELS_DIR"

@@EXPECT_PATH@@ -c "
  set timeout 300
  spawn @@DOWNLOADER_PATH@@ --only models --output $MODELS_DIR
  expect {
    \"*同意しますか*\" { send \"y\r\"; exp_continue }
    \"*[y,n,r]*\" { send \"y\r\"; exp_continue }
    \"*Press*\" { send \"q\r\"; exp_continue }
    \"*を押して*\" { send \"q\r\"; exp_continue }
    eof
  }
" || {
  echo "⚠️  Automatic download failed. You can manually run:"
  echo "  voicevox-download --only models --output $MODELS_DIR"
  exit 1
}

echo "✅ Voice models setup completed!"
echo "   26+ voice characters ready for text-to-speech synthesis"
echo "   Static libraries (Core + ONNX Runtime) already available"