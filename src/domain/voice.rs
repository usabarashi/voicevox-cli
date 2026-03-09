#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListVoiceStylesFilter {
    pub speaker_name: Option<String>,
    pub style_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceStyle {
    pub name: String,
    pub id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerStyles {
    pub speaker_name: String,
    pub styles: Vec<VoiceStyle>,
}

#[must_use]
pub fn normalized_filters(filter: &ListVoiceStylesFilter) -> (Option<String>, Option<String>) {
    (
        filter.speaker_name.as_ref().map(|s| s.to_lowercase()),
        filter.style_name.as_ref().map(|s| s.to_lowercase()),
    )
}

#[must_use]
pub fn filter_speakers(
    speakers: Vec<SpeakerStyles>,
    speaker_name_filter: Option<&str>,
    style_name_filter: Option<&str>,
) -> Vec<SpeakerStyles> {
    speakers
        .into_iter()
        .filter_map(|speaker| {
            let speaker_name_lower =
                speaker_name_filter.map(|_| speaker.speaker_name.to_lowercase());

            if let Some(name_filter) = speaker_name_filter
                && !speaker_name_lower
                    .as_deref()
                    .is_some_and(|lower| lower.contains(name_filter))
            {
                return None;
            }

            let filtered_styles = speaker
                .styles
                .into_iter()
                .filter(|style| {
                    style_name_filter
                        .is_none_or(|style_filter| style.name.to_lowercase().contains(style_filter))
                })
                .collect::<Vec<_>>();

            (!filtered_styles.is_empty()).then_some(SpeakerStyles {
                speaker_name: speaker.speaker_name,
                styles: filtered_styles,
            })
        })
        .collect()
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    fn sample_speakers() -> Vec<SpeakerStyles> {
        vec![
            SpeakerStyles {
                speaker_name: "Alice".to_string(),
                styles: vec![
                    VoiceStyle {
                        name: "Normal".to_string(),
                        id: 1,
                    },
                    VoiceStyle {
                        name: "Happy".to_string(),
                        id: 2,
                    },
                ],
            },
            SpeakerStyles {
                speaker_name: "Bob".to_string(),
                styles: vec![VoiceStyle {
                    name: "Whisper".to_string(),
                    id: 3,
                }],
            },
        ]
    }

    #[kani::proof]
    fn no_filter_keeps_all_speakers_and_styles() {
        let speakers = sample_speakers();
        let filtered = filter_speakers(speakers, None, None);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].styles.len(), 2);
        assert_eq!(filtered[1].styles.len(), 1);
    }

    #[kani::proof]
    fn style_filter_keeps_only_matching_styles() {
        let speakers = sample_speakers();
        let filtered = filter_speakers(speakers, None, Some("whisp"));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].speaker_name, "Bob");
        assert_eq!(filtered[0].styles.len(), 1);
        assert_eq!(filtered[0].styles[0].name, "Whisper");
    }

    #[kani::proof]
    fn speaker_filter_excludes_non_matching_speakers() {
        let speakers = sample_speakers();
        let filtered = filter_speakers(speakers, Some("ali"), None);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].speaker_name, "Alice");
    }
}
