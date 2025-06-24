use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    pub name: String,
    #[serde(default)]
    pub speaker_uuid: String,
    pub styles: Vec<Style>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    pub name: String,
    pub id: u32,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub style_type: Option<String>,
}

// 音声IDから必要なVVMモデル番号を取得
pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    match voice_id {
        // ずんだもん (3.vvm)
        1 | 3 | 7 | 5 | 22 | 38 => Some(3),
        // 四国めたん (2.vvm)
        2 | 0 | 6 | 4 | 36 | 37 => Some(2),
        // 春日部つむぎ (8.vvm)
        8 | 83 | 84 => Some(8),
        // 雨晴はう (10.vvm)
        10 | 85 => Some(10),
        // 波音リツ (9.vvm)
        9 | 65 => Some(9),
        // 玄野武宏 (11.vvm)
        11 | 39 | 40 | 41 => Some(11),
        // 白上虎太郎 (12.vvm)
        12 | 32 | 33 => Some(12),
        // 青山龍星 (13.vvm)
        13 | 86 | 87 | 88 | 89 | 90 => Some(13),
        // 冥鳴ひまり (14.vvm)
        14 => Some(14),
        // 九州そら (16.vvm)
        15 | 16 | 17 | 18 | 19 => Some(16),
        // もち子さん (17.vvm)
        20 => Some(17),
        // 剣崎雌雄 (18.vvm)
        21 => Some(18),
        // デフォルトは不明
        _ => None,
    }
}

// 音声名からスタイルIDへのマッピング
pub fn get_voice_mapping() -> HashMap<&'static str, (u32, &'static str)> {
    let mut voices = HashMap::new();

    // ずんだもん（全モード）
    voices.insert("zundamon", (3, "ずんだもん (ノーマル)"));
    voices.insert("zundamon-normal", (3, "ずんだもん (ノーマル)"));
    voices.insert("zundamon-amama", (1, "ずんだもん (あまあま)"));
    voices.insert("zundamon-tsundere", (7, "ずんだもん (ツンツン)"));
    voices.insert("zundamon-sexy", (5, "ずんだもん (セクシー)"));
    voices.insert("zundamon-whisper", (22, "ずんだもん (ささやき)"));
    voices.insert("zundamon-excited", (38, "ずんだもん (ヘロヘロ)"));

    // 四国めたん（全モード）
    voices.insert("metan", (2, "四国めたん (ノーマル)"));
    voices.insert("metan-normal", (2, "四国めたん (ノーマル)"));
    voices.insert("metan-amama", (0, "四国めたん (あまあま)"));
    voices.insert("metan-tsundere", (6, "四国めたん (ツンツン)"));
    voices.insert("metan-sexy", (4, "四国めたん (セクシー)"));
    voices.insert("metan-whisper", (36, "四国めたん (ささやき)"));
    voices.insert("metan-excited", (37, "四国めたん (ヘロヘロ)"));

    // 春日部つむぎ
    voices.insert("tsumugi", (8, "春日部つむぎ (ノーマル)"));
    voices.insert("tsumugi-normal", (8, "春日部つむぎ (ノーマル)"));

    // 雨晴はう
    voices.insert("hau", (10, "雨晴はう (ノーマル)"));
    voices.insert("hau-normal", (10, "雨晴はう (ノーマル)"));

    // 波音リツ
    voices.insert("ritsu", (9, "波音リツ (ノーマル)"));
    voices.insert("ritsu-normal", (9, "波音リツ (ノーマル)"));

    // 玄野武宏
    voices.insert("takehiro", (11, "玄野武宏 (ノーマル)"));
    voices.insert("takehiro-normal", (11, "玄野武宏 (ノーマル)"));
    voices.insert("takehiro-excited", (39, "玄野武宏 (喜び)"));
    voices.insert("takehiro-tsundere", (40, "玄野武宏 (ツンギレ)"));
    voices.insert("takehiro-sad", (41, "玄野武宏 (悲しみ)"));

    // 白上虎太郎
    voices.insert("kohtaro", (12, "白上虎太郎 (ふつう)"));
    voices.insert("kohtaro-normal", (12, "白上虎太郎 (ふつう)"));
    voices.insert("kohtaro-excited", (32, "白上虎太郎 (わーい)"));
    voices.insert("kohtaro-angry", (33, "白上虎太郎 (びくびく)"));

    // 青山龍星
    voices.insert("ryusei", (13, "青山龍星 (ノーマル)"));
    voices.insert("ryusei-normal", (13, "青山龍星 (ノーマル)"));
    voices.insert("ryusei-excited", (86, "青山龍星 (熱血)"));
    voices.insert("ryusei-cool", (87, "青山龍星 (不機嫌)"));
    voices.insert("ryusei-sad", (88, "青山龍星 (喜び)"));
    voices.insert("ryusei-surprised", (89, "青山龍星 (しっとり)"));
    voices.insert("ryusei-whisper", (90, "青山龍星 (かなしみ)"));

    // 冥鳴ひまり
    voices.insert("himari", (14, "冥鳴ひまり (ノーマル)"));
    voices.insert("himari-normal", (14, "冥鳴ひまり (ノーマル)"));

    // 九州そら
    voices.insert("sora", (16, "九州そら (ノーマル)"));
    voices.insert("sora-normal", (16, "九州そら (ノーマル)"));
    voices.insert("sora-amama", (15, "九州そら (あまあま)"));
    voices.insert("sora-tsundere", (18, "九州そら (ツンツン)"));
    voices.insert("sora-sexy", (17, "九州そら (セクシー)"));
    voices.insert("sora-whisper", (19, "九州そら (ささやき)"));

    // もち子さん
    voices.insert("mochiko", (20, "もち子さん (ノーマル)"));
    voices.insert("mochiko-normal", (20, "もち子さん (ノーマル)"));

    // 剣崎雌雄
    voices.insert("menou", (21, "剣崎雌雄 (ノーマル)"));
    voices.insert("menou-normal", (21, "剣崎雌雄 (ノーマル)"));

    // デフォルトエイリアス
    voices.insert("default", (3, "ずんだもん (ノーマル)"));

    voices
}

pub fn resolve_voice_name(voice_name: &str) -> Result<(u32, String)> {
    let voices = get_voice_mapping();

    // 音声一覧表示の特別なケース
    if voice_name == "?" {
        println!("Available VOICEVOX voices:");
        println!();

        // キャラクター別にグループ化して表示
        println!("  ずんだもん:");
        println!("    zundamon, zundamon-normal    (ID: 3)  - ずんだもん (ノーマル)");
        println!("    zundamon-amama              (ID: 1)  - ずんだもん (あまあま)");
        println!("    zundamon-tsundere           (ID: 7)  - ずんだもん (ツンツン)");
        println!("    zundamon-sexy               (ID: 5)  - ずんだもん (セクシー)");
        println!("    zundamon-whisper            (ID: 22) - ずんだもん (ささやき)");
        println!("    zundamon-excited            (ID: 38) - ずんだもん (ヘロヘロ)");
        println!();

        println!("  四国めたん:");
        println!("    metan, metan-normal         (ID: 2)  - 四国めたん (ノーマル)");
        println!("    metan-amama                 (ID: 0)  - 四国めたん (あまあま)");
        println!("    metan-tsundere              (ID: 6)  - 四国めたん (ツンツン)");
        println!("    metan-sexy                  (ID: 4)  - 四国めたん (セクシー)");
        println!("    metan-whisper               (ID: 36) - 四国めたん (ささやき)");
        println!("    metan-excited               (ID: 37) - 四国めたん (ヘロヘロ)");
        println!();

        println!("  その他のキャラクター:");
        println!("    tsumugi                     (ID: 8)  - 春日部つむぎ (ノーマル)");
        println!("    hau                         (ID: 10) - 雨晴はう (ノーマル)");
        println!("    ritsu                       (ID: 9)  - 波音リツ (ノーマル)");
        println!("    takehiro                    (ID: 11) - 玄野武宏 (ノーマル)");
        println!("    kohtaro                     (ID: 12) - 白上虎太郎 (ふつう)");
        println!("    ryusei                      (ID: 13) - 青山龍星 (ノーマル)");
        println!("    sora                        (ID: 16) - 九州そら (ノーマル)");
        println!();

        println!("  Tips:");
        println!("    - 数値IDを直接指定することも可能です: -v 3");
        println!("    - キャラクター名のみでデフォルトモードを使用: -v zundamon");
        println!("    - 特定のモードを指定: -v zundamon-amama");
        println!();

        std::process::exit(0);
    }

    // 直接的な数値指定をサポート
    if let Ok(style_id) = voice_name.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }

    // 音声名から検索
    if let Some((style_id, description)) = voices.get(voice_name) {
        Ok((*style_id, description.to_string()))
    } else {
        Err(anyhow!(
            "Unknown voice: '{}'. Use -v ? to list available voices.",
            voice_name
        ))
    }
}