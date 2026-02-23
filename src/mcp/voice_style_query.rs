use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListVoiceStylesParams {
    pub speaker_name: Option<String>,
    pub style_name: Option<String>,
}

pub type FilteredSpeakerStyles = (String, Vec<crate::voice::Style>);

#[must_use]
pub fn normalized_filters(params: &ListVoiceStylesParams) -> (Option<String>, Option<String>) {
    (
        params.speaker_name.as_ref().map(|s| s.to_lowercase()),
        params.style_name.as_ref().map(|s| s.to_lowercase()),
    )
}

#[must_use]
pub fn filter_speakers(
    speakers: Vec<crate::voice::Speaker>,
    speaker_name_filter: Option<&str>,
    style_name_filter: Option<&str>,
) -> Vec<FilteredSpeakerStyles> {
    speakers
        .into_iter()
        .filter_map(|speaker| {
            let crate::voice::Speaker { name, styles, .. } = speaker;
            let speaker_name_lower = speaker_name_filter.map(|_| name.to_lowercase());

            if let Some(name_filter) = speaker_name_filter {
                if !speaker_name_lower
                    .as_deref()
                    .is_some_and(|lower| lower.contains(name_filter))
                {
                    return None;
                }
            }

            let filtered_styles = styles
                .into_iter()
                .filter(|style| {
                    style_name_filter
                        .is_none_or(|style_filter| style.name.to_lowercase().contains(style_filter))
                })
                .collect::<Vec<_>>();

            (!filtered_styles.is_empty()).then_some((name.into(), filtered_styles))
        })
        .collect()
}

#[must_use]
pub fn render_voice_styles_result(filtered_results: &[FilteredSpeakerStyles]) -> String {
    if filtered_results.is_empty() {
        return "No speakers found matching the criteria.".to_string();
    }

    let blocks = filtered_results
        .iter()
        .map(render_voice_styles_block)
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("{blocks}\nTotal speakers found: {}", filtered_results.len())
}

fn render_voice_styles_block((speaker_name, styles): &FilteredSpeakerStyles) -> String {
    let style_lines = styles
        .iter()
        .map(|style| format!("  - {} (ID: {})", style.name, style.id))
        .collect::<Vec<_>>()
        .join("\n");

    format!("Speaker: {speaker_name}\nStyles:\n{style_lines}")
}
