pub struct Segment {
    pub text: String,
    pub speaker: Option<String>,
}

pub fn format(segments: &[Segment], filename: &str, language: &str) -> String {
    let date = chrono::Local::now().format("%Y-%m-%d %H:%M");

    let mut lines = vec![
        format!("# Транскрибация: {filename}"),
        String::new(),
        format!("**Язык:** {language}"),
        format!("**Дата:** {date}"),
        String::new(),
        "---".to_string(),
        String::new(),
    ];

    let has_speakers = segments.iter().any(|s| s.speaker.is_some());

    if has_speakers {
        let mut current_speaker = String::new();
        for seg in segments {
            if let Some(ref speaker) = seg.speaker {
                if speaker != &current_speaker {
                    current_speaker = speaker.clone();
                    lines.push(format!("## {speaker}"));
                    lines.push(String::new());
                }
            }
            lines.push(seg.text.clone());
        }
    } else {
        for seg in segments {
            lines.push(seg.text.clone());
        }
    }

    lines.push(String::new());
    lines.push("---".to_string());
    lines.push("*Сгенерировано с помощью speech2text-whisper*".to_string());

    lines.join("\n")
}
