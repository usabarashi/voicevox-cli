use voicevox_core::blocking::{OpenJtalk, Synthesizer};

use crate::voice::{Speaker, Style};

pub(crate) fn collect_speakers_from_synthesizer(
    synthesizer: &Synthesizer<OpenJtalk>,
) -> Vec<Speaker> {
    synthesizer
        .metas()
        .iter()
        .map(|meta| Speaker {
            name: meta.name.clone().into(),
            speaker_uuid: meta.speaker_uuid.clone().into(),
            styles: meta
                .styles
                .iter()
                .map(|style| Style {
                    name: style.name.clone().into(),
                    id: style.id.0,
                    style_type: Some(format!("{:?}", style.r#type).into()),
                })
                .collect(),
            version: meta.version.to_string().into(),
        })
        .collect()
}
