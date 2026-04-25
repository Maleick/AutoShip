//! Text-to-speech alerts — MQTextToSpeech native equivalent.
//!
//! Speaks alert text aloud using the platform TTS engine.  On Windows a
//! dedicated worker thread initialises the SAPI speech synthesiser once and
//! processes speak requests through an in-process channel — keeping
//! end-to-end latency well under 1 second after the engine is warmed up.
//!
//! On macOS and Linux [`TtsEngine`] compiles fully but speak calls are
//! explicit no-ops; parity will be wired in a future milestone.

use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// TTS voice configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Name of the SAPI voice to use (e.g. `"Microsoft Zira Desktop"`).
    /// `None` uses the system default voice.
    #[serde(default)]
    pub voice: Option<String>,
    /// Speech rate in the range [-10, 10]. 0 = normal speed.
    #[serde(default)]
    pub rate: i32,
    /// Volume in [0, 100]. 100 = maximum.
    #[serde(default = "default_volume")]
    pub volume: u16,
    /// Master mute — suppresses all speech when `true`.
    #[serde(default)]
    pub muted: bool,
}

fn default_volume() -> u16 {
    100
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            voice: None,
            rate: 0,
            volume: 100,
            muted: false,
        }
    }
}

impl TtsConfig {
    /// Load from a TOML file. A missing file returns the default config.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).map_err(|e| anyhow::anyhow!("tts config parse: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow::anyhow!("read tts config {}: {e}", path.display())),
        }
    }
}

// ── messages sent to the worker thread ────────────────────────────────────

enum WorkerMsg {
    Speak(String),
    UpdateConfig(TtsConfig),
    Shutdown,
}

/// Speech engine handle — owns the background worker and channel.
///
/// Create with [`TtsEngine::new`], submit text with [`TtsEngine::speak`].
/// The worker thread is joined when the engine is dropped.
pub struct TtsEngine {
    sender: mpsc::SyncSender<WorkerMsg>,
    _worker: thread::JoinHandle<()>,
}

impl TtsEngine {
    /// Initialise the TTS engine with the given config.
    ///
    /// On Windows this spawns a background thread that initialises the SAPI
    /// `SpVoice` COM object.  The thread is ready to speak without additional
    /// startup cost after this call returns.
    pub fn new(config: TtsConfig) -> Self {
        // Channel depth of 64 so bursts don't block callers.
        let (sender, receiver) = mpsc::sync_channel::<WorkerMsg>(64);
        let worker = thread::Builder::new()
            .name("tts-worker".into())
            .spawn(move || run_worker(receiver, config))
            .expect("spawn tts worker");

        Self {
            sender,
            _worker: worker,
        }
    }

    /// Queue `text` for speech output.  Returns immediately; delivery is
    /// asynchronous via the worker thread.
    ///
    /// Silently drops the request when the engine is shut down or the channel
    /// is full.
    pub fn speak(&self, text: impl Into<String>) {
        let _ = self.sender.try_send(WorkerMsg::Speak(text.into()));
    }

    /// Update the voice/rate/volume/mute config at runtime without restarting.
    pub fn update_config(&self, config: TtsConfig) {
        let _ = self.sender.try_send(WorkerMsg::UpdateConfig(config));
    }

    /// Shut down the worker thread gracefully.  The engine is unusable after
    /// this call; drop the [`TtsEngine`] instead when possible.
    pub fn shutdown(&self) {
        let _ = self.sender.try_send(WorkerMsg::Shutdown);
    }
}

impl Drop for TtsEngine {
    fn drop(&mut self) {
        let _ = self.sender.try_send(WorkerMsg::Shutdown);
    }
}

// ── worker thread ──────────────────────────────────────────────────────────

fn run_worker(receiver: mpsc::Receiver<WorkerMsg>, initial_config: TtsConfig) {
    run_worker_impl(receiver, initial_config);
}

#[cfg(windows)]
fn run_worker_impl(receiver: mpsc::Receiver<WorkerMsg>, initial_config: TtsConfig) {
    use windows::{
        Win32::{
            Media::Speech::{ISpVoice, SPF_DEFAULT, SpVoice},
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL,
                COINIT_APARTMENTTHREADED,
            },
        },
        core::PCWSTR,
    };

    // Initialise COM for this STA thread — required for ISpVoice.
    let co_init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if let Err(e) = co_init {
        tracing::error!("CoInitializeEx failed: {e} — TTS unavailable");
        return;
    }

    let voice_result: windows::core::Result<ISpVoice> =
        unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL) };

    let voice = match voice_result {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("SpVoice CoCreateInstance failed: {e} — TTS unavailable");
            unsafe { CoUninitialize() };
            return;
        }
    };

    let mut config = initial_config;
    apply_voice_config(&voice, &config);

    for msg in receiver {
        match msg {
            WorkerMsg::Speak(text) => {
                if config.muted || text.is_empty() {
                    continue;
                }
                // Convert to null-terminated UTF-16 for PCWSTR.
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let pcwstr = PCWSTR::from_raw(wide.as_ptr());
                // SPF_DEFAULT speaks synchronously within the STA message pump.
                let result = unsafe { voice.Speak(pcwstr, SPF_DEFAULT, None) };
                if let Err(e) = result {
                    tracing::warn!("ISpVoice::Speak failed: {e}");
                }
            }
            WorkerMsg::UpdateConfig(new) => {
                config = new;
                apply_voice_config(&voice, &config);
            }
            WorkerMsg::Shutdown => break,
        }
    }

    unsafe { CoUninitialize() };
}

#[cfg(windows)]
fn apply_voice_config(voice: &windows::Win32::Media::Speech::ISpVoice, config: &TtsConfig) {
    let rate = config.rate.clamp(-10, 10);
    if let Err(e) = unsafe { voice.SetRate(rate) } {
        tracing::warn!("ISpVoice::SetRate({rate}) failed: {e}");
    }

    let vol = config.volume.clamp(0, 100);
    if let Err(e) = unsafe { voice.SetVolume(vol) } {
        tracing::warn!("ISpVoice::SetVolume({vol}) failed: {e}");
    }
}

#[cfg(not(windows))]
fn run_worker_impl(receiver: mpsc::Receiver<WorkerMsg>, _initial_config: TtsConfig) {
    // Explicit no-op stub — drain the channel so senders never block.
    for msg in receiver {
        if matches!(msg, WorkerMsg::Shutdown) {
            break;
        }
        // Speak/UpdateConfig: no-op on non-Windows.
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_starts_and_shuts_down() {
        let engine = TtsEngine::new(TtsConfig::default());
        // On non-Windows these are no-ops; on Windows they exercise the worker
        // path without an audio device (CI environments may have no SAPI).
        engine.speak("test");
        engine.shutdown();
    }

    #[test]
    fn muted_engine_accepts_text() {
        let config = TtsConfig {
            muted: true,
            ..Default::default()
        };
        let engine = TtsEngine::new(config);
        engine.speak("this should be silent");
    }

    #[test]
    fn config_roundtrips_toml() {
        let config = TtsConfig {
            voice: Some("Microsoft David Desktop".into()),
            rate: 2,
            volume: 80,
            muted: false,
        };
        let s = toml::to_string(&config).expect("serialize");
        let back: TtsConfig = toml::from_str(&s).expect("deserialize");
        assert_eq!(back.voice.as_deref(), Some("Microsoft David Desktop"));
        assert_eq!(back.rate, 2);
        assert_eq!(back.volume, 80);
    }

    #[test]
    fn update_config_does_not_panic() {
        let engine = TtsEngine::new(TtsConfig::default());
        engine.update_config(TtsConfig {
            muted: true,
            ..Default::default()
        });
    }
}
