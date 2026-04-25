//! Alert delivery channels — MQ2Sound audio playback and MQTextToSpeech TTS.
//!
//! Provides the audio and speech-synthesis delivery channels that integrate
//! with the existing alert pipeline (`alerts.rs`, #1567).

pub mod sound;
pub mod tts;
