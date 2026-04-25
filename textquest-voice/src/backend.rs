use std::future::Future;
use std::path::PathBuf;

use crate::{config::VoiceId, voices::VoiceLibrary};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioBuffer {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl AudioBuffer {
    pub fn silence(sample_rate_hz: u32, channels: u16, duration_ms: u32) -> Self {
        let frame_count = ((sample_rate_hz as u64 * duration_ms as u64) / 1000) as usize;
        let sample_count = frame_count.saturating_mul(channels as usize);
        Self {
            sample_rate_hz,
            channels,
            samples: vec![0; sample_count],
        }
    }

    pub fn duration_ms(&self) -> u32 {
        if self.sample_rate_hz == 0 || self.channels == 0 {
            return 0;
        }

        let frame_count = self.samples.len() as u64 / self.channels as u64;
        ((frame_count * 1000) / self.sample_rate_hz as u64) as u32
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TtsError {
    #[error("unknown voice clip: {0}")]
    UnknownVoice(String),
    #[error("missing ElevenLabs API key")]
    MissingApiKey,
}

pub trait TtsBackend: Send + Sync {
    fn synthesize(
        &self,
        text: &str,
        voice_id: &VoiceId,
    ) -> impl Future<Output = Result<AudioBuffer, TtsError>> + Send;
    fn supports_cloning(&self) -> bool;
    fn estimated_latency_ms(&self, text: &str) -> u32;
}

#[derive(Debug, Clone)]
pub struct CoquiBackend {
    voice_library: VoiceLibrary,
    sample_rate_hz: u32,
}

impl CoquiBackend {
    pub fn new(voice_library: VoiceLibrary) -> Self {
        Self {
            voice_library,
            sample_rate_hz: 22_050,
        }
    }
}

impl Default for CoquiBackend {
    fn default() -> Self {
        Self::new(VoiceLibrary::new(
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from),
        ))
    }
}

impl TtsBackend for CoquiBackend {
    fn synthesize(
        &self,
        text: &str,
        voice_id: &VoiceId,
    ) -> impl Future<Output = Result<AudioBuffer, TtsError>> + Send {
        let resolved = self
            .voice_library
            .resolve_voice_clip(voice_id)
            .ok_or_else(|| TtsError::UnknownVoice(voice_id.as_str().to_owned()));
        let sample_rate_hz = self.sample_rate_hz;
        let latency_ms = self.estimated_latency_ms(text).max(25);

        async move {
            let _ = resolved?;
            Ok(AudioBuffer::silence(sample_rate_hz, 1, latency_ms))
        }
    }

    fn supports_cloning(&self) -> bool {
        true
    }

    fn estimated_latency_ms(&self, text: &str) -> u32 {
        220 + (text.chars().count() as u32 / 8)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PiperBackend {
    sample_rate_hz: u32,
}

impl PiperBackend {
    pub fn new() -> Self {
        Self {
            sample_rate_hz: 22_050,
        }
    }
}

impl TtsBackend for PiperBackend {
    fn synthesize(
        &self,
        text: &str,
        _voice_id: &VoiceId,
    ) -> impl Future<Output = Result<AudioBuffer, TtsError>> + Send {
        let sample_rate_hz = self.sample_rate_hz;
        let latency_ms = self.estimated_latency_ms(text).max(10);

        async move { Ok(AudioBuffer::silence(sample_rate_hz, 1, latency_ms)) }
    }

    fn supports_cloning(&self) -> bool {
        false
    }

    fn estimated_latency_ms(&self, text: &str) -> u32 {
        35 + (text.chars().count() as u32 / 20)
    }
}

#[derive(Debug, Clone)]
pub struct SapiBackend {
    sample_rate_hz: u32,
}

impl Default for SapiBackend {
    fn default() -> Self {
        Self {
            sample_rate_hz: 22_050,
        }
    }
}

impl TtsBackend for SapiBackend {
    fn synthesize(
        &self,
        text: &str,
        _voice_id: &VoiceId,
    ) -> impl Future<Output = Result<AudioBuffer, TtsError>> + Send {
        let sample_rate_hz = self.sample_rate_hz;
        let latency_ms = self.estimated_latency_ms(text).max(10);

        async move { Ok(AudioBuffer::silence(sample_rate_hz, 1, latency_ms)) }
    }

    fn supports_cloning(&self) -> bool {
        false
    }

    fn estimated_latency_ms(&self, _text: &str) -> u32 {
        75
    }
}

#[derive(Debug, Clone)]
pub struct ElevenLabsBackend {
    sample_rate_hz: u32,
    api_key: Option<String>,
    voice_id: String,
}

impl ElevenLabsBackend {
    pub fn new(api_key: Option<String>, voice_id: impl Into<String>) -> Self {
        Self {
            sample_rate_hz: 24_000,
            api_key,
            voice_id: voice_id.into(),
        }
    }
}

impl TtsBackend for ElevenLabsBackend {
    fn synthesize(
        &self,
        text: &str,
        _voice_id: &VoiceId,
    ) -> impl Future<Output = Result<AudioBuffer, TtsError>> + Send {
        let api_key = self.api_key.clone();
        let sample_rate_hz = self.sample_rate_hz;
        let latency_ms = self.estimated_latency_ms(text).max(25);
        let selected_voice_id = self.voice_id.clone();

        async move {
            if api_key.is_none() {
                return Err(TtsError::MissingApiKey);
            }

            let _ = selected_voice_id;
            Ok(AudioBuffer::silence(sample_rate_hz, 1, latency_ms))
        }
    }

    fn supports_cloning(&self) -> bool {
        true
    }

    fn estimated_latency_ms(&self, text: &str) -> u32 {
        300 + (text.chars().count() as u32 / 2)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn backends_report_expected_capabilities_and_latency_budgets() {
        let temp_home = TempDir::new().expect("temp home");
        let voices_dir = temp_home.path().join(".textquest").join("voices");
        fs::create_dir_all(&voices_dir).unwrap();
        fs::write(voices_dir.join("motherly_cleric.wav"), b"override").unwrap();

        let coqui = CoquiBackend::new(VoiceLibrary::new(temp_home.path()));
        let piper = PiperBackend::new();
        let sapi = SapiBackend::default();
        let eleven = ElevenLabsBackend::new(Some("demo".to_owned()), "default");

        assert!(coqui.supports_cloning());
        assert!(coqui.estimated_latency_ms("hello world") <= 350);

        assert!(!piper.supports_cloning());
        assert!(piper.estimated_latency_ms("hello world") <= 50);

        assert!(!sapi.supports_cloning());
        assert!(sapi.estimated_latency_ms("hello world") <= 100);

        assert!(eleven.supports_cloning());
        assert!(eleven.estimated_latency_ms("hello world") <= 800);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn sapi_backend_is_a_no_op_stub_on_non_windows() {
        let sapi = SapiBackend::default();
        let buffer = sapi
            .synthesize("hello world", &VoiceId::new("system-default"))
            .await
            .expect("stub output");

        assert!(buffer.samples.len() > 0);
        assert!(buffer.duration_ms() >= 10);
    }
}
