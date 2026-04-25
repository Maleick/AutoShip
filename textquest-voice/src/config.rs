use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Coqui,
    Piper,
    Sapi,
    ElevenLabs,
}

impl Default for BackendKind {
    fn default() -> Self {
        Self::Coqui
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct VoiceId(String);

impl VoiceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VoiceConfig {
    #[serde(default)]
    pub backend: BackendKind,
    pub voice_id: VoiceId,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default)]
    pub pitch: f32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            backend: BackendKind::Coqui,
            voice_id: VoiceId::new(""),
            speed: default_speed(),
            pitch: 0.0,
        }
    }
}

fn default_speed() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VoiceProfile {
    pub voice: VoiceConfig,
}

impl VoiceProfile {
    pub fn from_toml_str(input: &str) -> Result<Self, ConfigError> {
        toml::from_str(input).map_err(ConfigError::Toml)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("voice config parse error: {0}")]
    Toml(#[from] toml::de::Error),
}
