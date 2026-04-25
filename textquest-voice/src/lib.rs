pub mod backend;
pub mod config;
pub mod voices;

#[cfg(test)]
mod tests {
    use super::config::{BackendKind, VoiceProfile};

    #[test]
    fn parses_character_voice_toml() {
        let toml = r#"
[voice]
backend = "coqui"
voice_id = "~/.textquest/voices/motherly_cleric.wav"
speed = 1.0
pitch = 0.0
"#;

        let profile = VoiceProfile::from_toml_str(toml).unwrap();

        assert_eq!(profile.voice.backend, BackendKind::Coqui);
        assert_eq!(
            profile.voice.voice_id.as_str(),
            "~/.textquest/voices/motherly_cleric.wav"
        );
        assert_eq!(profile.voice.speed, 1.0);
        assert_eq!(profile.voice.pitch, 0.0);
    }
}
