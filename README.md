# VOICEVOX TTS

VOICEVOX Core 0.16.0 を使用した production-ready daemon-client 型 TTS ツール

## 特徴

- **Daemon-Client アーキテクチャ**: 高速な音声合成のための daemon プロセス
- **macOS say 互換**: macOS の say コマンドと同様の silent 動作
- **99 音声スタイル**: ずんだもん、四国めたん、春日部つむぎなど 26 キャラクター
- **即座の音声合成**: 事前読み込み済みモデルによる瞬時の音声生成
- **CPU 専用処理**: macOS 向け最適化（CUDA/DirectML 無効）
- **XDG 準拠**: 標準的な Unix ファイル配置規則
- **環境独立**: 自動パス発見による設定不要

## アーキテクチャ

### Production システム

1. **`voicevox-daemon`**: 全 VVM モデル事前読み込み済み background プロセス
2. **`voicevox-say`**: 軽量 CLI client（primary interface）
3. **`voicevox-tts`**: Legacy standalone binary（互換性維持）

### IPC 通信

- **Unix Sockets**: XDG 準拠ファイル配置
- **Tokio Async**: 非同期 I/O による高性能通信
- **Bincode**: 効率的なバイナリ protocol

### Socket パス優先順位

1. `$VOICEVOX_SOCKET_PATH` (環境変数)
2. `$XDG_RUNTIME_DIR/voicevox/daemon.sock` (runtime)
3. `$XDG_STATE_HOME/voicevox/daemon.sock` (state)  
4. `~/.local/state/voicevox/daemon.sock` (fallback)
5. `$TMPDIR/voicevox-daemon-{pid}.sock` (temporary)

## インストール

### Nix（推奨）

```bash
# ビルドとインストール
nix build

# 直接実行
nix run . -- "こんにちは、ずんだもんなのだ"

# 開発環境
nix develop
```

### Nix Flake として使用

#### Input として追加

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    voicevox-tts.url = "github:usabarashi/voicevox-tts";
  };

  outputs = { nixpkgs, voicevox-tts, ... }: {
    packages.aarch64-darwin.default = nixpkgs.legacyPackages.aarch64-darwin.mkShell {
      buildInputs = [ voicevox-tts.packages.aarch64-darwin.default ];
    };
  };
}
```

#### Overlay として使用

```nix
nixpkgs.overlays = [ voicevox-tts.overlays.default ];

environment.systemPackages = with pkgs; [
  voicevox-tts   # or voicevox-say
];
```

### Cargo（Development）

```bash
# 必要な環境変数設定
export DYLD_LIBRARY_PATH=./voicevox_core/c_api/lib:./voicevox_core/onnxruntime/lib

# Production build
cargo build --release --bin voicevox-daemon --bin voicevox-say

# Development build
cargo build --bin voicevox-daemon --bin voicevox-say
```

## 使い方

### Daemon-Client モード（推奨）

```bash
# Daemon 自動起動による音声合成
voicevox-say "こんにちは、ずんだもんなのだ"

# 音声指定
voicevox-say -v zundamon-amama "あまあまモードなのだ♪"
voicevox-say -v metan-tsundere "ツンツンめたんです"

# ファイル保存
voicevox-say -o output.wav "保存するテキスト"

# 標準入力から
echo "パイプからの入力" | voicevox-say

# Daemon 状況確認
voicevox-say --daemon-status
```

### Daemon 直接操作

```bash
# Daemon 手動起動（foreground）
voicevox-daemon --foreground

# Daemon 手動起動（background）
voicevox-daemon

# Daemon 停止
pkill -f voicevox-daemon
```

### 音声発見

```bash
# 音声一覧表示
voicevox-say -v "?"

# 詳細スピーカー情報
voicevox-say --list-speakers

# Speaker ID 直接指定
voicevox-say --speaker-id 3 "ずんだもん（ノーマル）"
```

### Standalone モード

```bash
# Daemon 使用しない強制 standalone
voicevox-say --standalone "独立実行モード"

# Minimal models（高速起動）
voicevox-say --standalone --minimal-models "軽量モード"
```

## 音声キャラクター

### 主要キャラクター

**ずんだもん（8種類）**
- `zundamon` / `--speaker-id 3` - ノーマル
- `zundamon-amama` / `--speaker-id 1` - あまあま
- `zundamon-tsundere` / `--speaker-id 7` - ツンツン
- `zundamon-sexy` / `--speaker-id 5` - セクシー
- `zundamon-whisper` / `--speaker-id 22` - ささやき
- その他3種類の感情表現

**四国めたん（6種類）**
- `metan` / `--speaker-id 2` - ノーマル
- `metan-amama` / `--speaker-id 0` - あまあま
- `metan-tsundere` / `--speaker-id 6` - ツンツン
- その他3種類の感情表現

**その他 16キャラクター**
- 春日部つむぎ、雨晴はう、波音リツ、玄野武宏、白上虎太郎等

## 技術仕様

### Core 技術

- **VOICEVOX Core**: 0.16.0 (MIT License)
- **Runtime**: CPU-only processing on macOS
- **Audio Format**: WAV (16bit, 24kHz)
- **Language**: Rust with async/await
- **Communication**: Unix sockets + tokio
- **Platform**: macOS (aarch64/x86_64)

### パフォーマンス

- **Daemon 起動時間**: ~3秒（全モデル読み込み）
- **音声合成時間**: ~100ms（daemon モード）
- **メモリ使用量**: ~500MB（全モデル読み込み時）
- **ファイルサイズ**: ~20MB（最小構成）

## 開発

### 開発環境

```bash
# Nix 開発環境
nix develop

# 依存関係確認
cargo build --bin voicevox-daemon --bin voicevox-say

# テスト実行
cargo test

# 実動作確認
./target/debug/voicevox-daemon --foreground &
./target/debug/voicevox-say "動作テスト"
```

### アーキテクチャ詳細

**重要ファイル**:
- `src/lib.rs` - 共有ライブラリ、VoicevoxCore、IPC プロトコル
- `src/bin/daemon.rs` - バックグラウンド daemon、モデル管理  
- `src/bin/client.rs` - 軽量 CLI client、primary interface
- `voicevox_core/` - VOICEVOX Core runtime ライブラリ
- `models/*.vvm` - 音声モデルファイル（19 models）
- `dict/` - OpenJTalk 辞書

## ライセンス

### CLI ツール

MIT License OR Apache License 2.0

### VOICEVOX Core

MIT License  
Copyright (c) 2021 Hiroshiba Kazuyuki

### ONNX Runtime

Custom License Terms  
Commercial use allowed with attribution required  
See: `voicevox_core/onnxruntime/TERMS.txt`

### 使用時の注意

**音声生成時のクレジット表記が必要です**:
- 「VOICEVOX を使用して生成」
- キャラクター別の利用規約に従ってください
- 商用利用時は個別ライセンスを確認してください

詳細: [VOICEVOX 利用規約](https://voicevox.hiroshiba.jp/term)

## 貢献

Issues や Pull Requests を歓迎します！

### 開発ガイドライン

- 実装前に Issue で相談推奨
- Rust 標準スタイル（rustfmt）準拠
- 全てのテストが通ることを確認
- Commit message は英語で簡潔に

## 関連リンク

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix](https://nixos.org/)
- [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)

---

ずんだもんと一緒に楽しい TTS ライフを送るのだ！