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

            if let Some(name_filter) = speaker_name_filter {
                if !speaker_name_lower
                    .as_deref()
                    .is_some_and(|lower| lower.contains(name_filter))
                {
                    return None;
                }
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
