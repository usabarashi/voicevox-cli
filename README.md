# VOICEVOX CLI

VOICEVOX Core 0.16.0 を使用した macOS 向け CLI ツール（CPU 専用処理）

## 🫛 特徴

- ✅ **macOS CPU 専用**: CUDA/DirectML を無効化、CPU のみで動作
- ✅ **say コマンド互換**: macOS の say コマンドと同様の使い方
- ✅ **全キャラクター対応**: ずんだもん、四国めたん、春日部つむぎなど全スタイル
- ✅ **ストリーミング再生**: ファイル出力なしでリアルタイム音声再生
- ✅ **名前・ID 両対応**: 音声名での指定と数値 ID での指定の両方に対応
- ✅ **Nix パッケージ**: 再現可能なビルドとデプロイ

## 📦 インストール

### Nix を使用（推奨）

```bash
# ビルド
nix build

# 実行
./result/bin/voicevox-say --help

# 直接実行
nix run . -- --help
```

### 他の Nix Flake から使用

このパッケージは他の Nix Flake から input として使用できるのだ！

#### Input として追加

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    voicevox-cli.url = "github:your-username/voicevox-cli";
  };

  outputs = { self, nixpkgs, voicevox-cli }:
    # 使用例は以下を参照
}
```

#### パッケージとして使用

```nix
# パッケージを直接使用
environment.systemPackages = [
  voicevox-cli.packages.${system}.default
];

# または名前付きで
environment.systemPackages = [
  voicevox-cli.packages.${system}.voicevox-cli
];
```

#### Overlay として使用

```nix
# overlay を適用
nixpkgs.overlays = [ voicevox-cli.overlays.default ];

# その後、通常のパッケージとして利用可能
environment.systemPackages = with pkgs; [
  voicevox-cli
  voicevox-say  # alias
];
```

#### App として実行

```nix
# 他の flake から直接実行
nix run github:your-username/voicevox-cli -- "こんにちはなのだ！"
```

#### ライブラリ関数として使用

```nix
let
  # カスタム nixpkgs でビルド
  voicevox-package = voicevox-cli.lib.${system}.mkVoicevoxCli pkgs;
in
{
  environment.systemPackages = [ voicevox-package ];
}
```

### Cargo を使用

```bash
# 開発環境
nix develop

# ビルド
cargo build --release

# 実行
./target/release/voicevox-cli --help
```

## 🎯 使い方

### 基本的な使い方（say コマンドスタイル）

```bash
# テキスト指定
voicevox-cli "こんにちは、ずんだもんなのだ！"

# 音声指定
voicevox-cli -v zundamon-amama "あまあまモードなのだ♪"
voicevox-cli -v zundamon-tsuyo "強気モードなのだ！"

# ファイルから読み込み
voicevox-cli -f input.txt -o output.wav

# 標準入力から
echo "テキスト" | voicevox-cli
```

### 音声一覧表示

```bash
# 利用可能な音声を表示
voicevox-cli -v "?"

# スピーカー一覧（詳細）
voicevox-cli --list-speakers
```

### ストリーミング再生

```bash
# リアルタイム再生（ファイル出力なし）
voicevox-cli --streaming "長いテキストもリアルタイムで再生するのだ！"
```

## 🔧 技術仕様

- **VOICEVOX Core**: 0.16.0
- **プラットフォーム**: macOS (aarch64/x86_64)
- **処理モード**: CPU専用（GPU無効化）
- **音声形式**: WAV (16bit, 24kHz)
- **言語**: Rust
- **ビルドシステム**: Nix + Cargo

## 🔬 CPU 専用処理について

このツールは macOS 環境において以下の理由で CPU 専用処理をハードコーディングしています：

- macOS では CUDA サポートがない
- DirectML は Windows 専用
- Apple Silicon の高性能 CPU により十分な性能を実現

```rust
// macOS では強制的に CPU モードで初期化
#[cfg(target_os = "macos")]
{
    let init_options = voicevox_initialize_options_new(
        VoicevoxAccelerationMode::Cpu,
        cpu_threads
    );
}
```

## 🚀 開発

### 開発環境

```bash
# Nix 開発環境
nix develop

# Rust ツールチェーンが利用可能
cargo --version
rustc --version
```

### ビルド

```bash
# デバッグビルド
cargo build

# リリースビルド
cargo build --release

# Nix ビルド
nix build
```

### テスト

```bash
# ユニットテスト
cargo test

# 統合テスト（実際の音声合成）
cargo run -- --list-speakers
cargo run -- "テストメッセージ"
```

## 📄 ライセンス

MIT License

## 🤝 貢献

Issues や Pull Requests は歓迎です！

## 🔗 関連リンク

- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [VOICEVOX Core](https://github.com/VOICEVOX/voicevox_core)
- [Nix](https://nixos.org/)

---

🫛 ずんだもんと一緒に楽しい TTS ライフを送るのだ！
