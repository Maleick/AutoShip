//! HTTP-based LLM provider — calls local ollama-compatible APIs.
//!
//! When no local provider is configured (or provider = "none"), the caller
//! should fall back to `TraitDrivenResponder`.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::{LlmProvider, LlmRequest, LlmResponse, Situation};
use crate::config::{BotPersonalityConfig, LlmConfig, LlmProviderKind};

/// LLM API client that routes to the configured provider.
pub struct ApiLlmClient {
    config: LlmConfig,
    personality: BotPersonalityConfig,
    http: reqwest::blocking::Client,
}

impl ApiLlmClient {
    /// Create a new API client from operator config.
    #[must_use]
    pub fn new(config: LlmConfig, personality: BotPersonalityConfig) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self {
            config,
            personality,
            http,
        }
    }

    /// Build the system prompt from preset + any custom override.
    fn system_prompt(&self) -> String {
        let preset_prompt = self.personality.preset.system_prompt();
        if self.personality.system_prompt.is_empty() {
            preset_prompt.to_string()
        } else {
            format!("{}\n\n{}", preset_prompt, self.personality.system_prompt)
        }
    }

    /// Convert a Situation into a user-facing prompt string.
    fn situation_to_prompt(request: &LlmRequest) -> String {
        match &request.situation {
            Situation::GameEvent { description } => {
                format!("[Fleet Event] {description}\nComment on this event in character.")
            }
            Situation::CombatReaction { description } => {
                format!("[Combat] {description}\nReact to this combat event in character.")
            }
            Situation::PlayerChat {
                player_name,
                message,
                ..
            } => {
                format!("[Chat from {player_name}] {message}\nRespond in character.")
            }
            Situation::BotChat {
                character_name,
                message,
            } => {
                format!("[{character_name} says] {message}\nRespond in character.")
            }
            Situation::IdleChatter => {
                "Generate a short idle comment or observation in character.".into()
            }
            Situation::FleetCommentary { event_summary } => {
                format!(
                    "[Fleet Update] {event_summary}\nProvide snarky commentary on this fleet event."
                )
            }
        }
    }

    fn call_ollama(&self, system: &str, user_msg: &str) -> Result<(String, u32)> {
        let base = if self.config.base_url.is_empty() {
            "http://localhost:11434"
        } else {
            &self.config.base_url
        };

        let body = serde_json::json!({
            "model": self.config.model,
            "stream": false,
            "options": {
                "temperature": self.config.temperature,
                "num_predict": self.config.max_tokens,
            },
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user_msg}
            ]
        });

        let resp = self
            .http
            .post(format!("{base}/api/chat"))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .context("Ollama API request failed")?;

        let status = resp.status();
        let text = resp.text().context("Failed to read ollama response")?;
        if !status.is_success() {
            bail!("Ollama API error {status}: {text}");
        }

        let parsed: OllamaResponse =
            serde_json::from_str(&text).context("Failed to parse ollama response")?;

        let tokens = parsed.eval_count.unwrap_or(0);
        Ok((parsed.message.content, tokens))
    }
}

impl LlmProvider for ApiLlmClient {
    fn generate(&mut self, request: &LlmRequest) -> Result<LlmResponse> {
        let system = self.system_prompt();
        let user_msg = Self::situation_to_prompt(request);

        let (text, tokens_used) = match self.config.provider {
            LlmProviderKind::Ollama => self.call_ollama(&system, &user_msg)?,
            LlmProviderKind::None => {
                bail!("No LLM provider configured");
            }
        };

        Ok(LlmResponse {
            text,
            from_llm: true,
            tokens_used,
        })
    }

    fn name(&self) -> &str {
        match self.config.provider {
            LlmProviderKind::Ollama => "ollama",
            LlmProviderKind::None => "none",
        }
    }

    fn is_available(&self) -> bool {
        match self.config.provider {
            LlmProviderKind::Ollama => true,
            LlmProviderKind::None => false,
        }
    }
}

// ─── API Response Types ───

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BotPersonalityPreset, LlmProviderKind};
    use textquest_common::soul::{MoodState, PersonalityTraits, SpeechStyle};

    fn test_config(provider: LlmProviderKind) -> LlmConfig {
        LlmConfig {
            provider,
            api_key: "test-key".into(),
            ..LlmConfig::default()
        }
    }

    fn test_personality() -> BotPersonalityConfig {
        BotPersonalityConfig::default()
    }

    fn test_request() -> LlmRequest {
        LlmRequest {
            character_name: "Fippy".into(),
            traits: PersonalityTraits::default(),
            mood: MoodState::Neutral,
            speech_style: SpeechStyle::default(),
            situation: Situation::FleetCommentary {
                event_summary: "Group 1 wiped to a train in Befallen".into(),
            },
            priority: super::super::LlmPriority::Medium,
            memory_context: vec![],
            backstory: String::new(),
        }
    }

    #[test]
    fn api_client_construction() {
        let client = ApiLlmClient::new(test_config(LlmProviderKind::Ollama), test_personality());
        assert_eq!(client.name(), "ollama");
        assert!(client.is_available());
    }

    #[test]
    fn none_provider_not_available() {
        let client = ApiLlmClient::new(test_config(LlmProviderKind::None), test_personality());
        assert!(!client.is_available());
        assert_eq!(client.name(), "none");
    }

    #[test]
    fn ollama_available_without_key() {
        let config = LlmConfig {
            provider: LlmProviderKind::Ollama,
            api_key: String::new(),
            ..LlmConfig::default()
        };
        let client = ApiLlmClient::new(config, test_personality());
        assert!(client.is_available());
    }

    #[test]
    fn system_prompt_uses_preset() {
        let client = ApiLlmClient::new(test_config(LlmProviderKind::Ollama), test_personality());
        let prompt = client.system_prompt();
        assert!(prompt.contains("Fippy Darkpaw"));
        assert!(prompt.contains("gnoll"));
    }

    #[test]
    fn system_prompt_appends_custom() {
        let personality = BotPersonalityConfig {
            preset: BotPersonalityPreset::FippyDarkpaw,
            system_prompt: "Also mention loot drops.".into(),
            ..BotPersonalityConfig::default()
        };
        let client = ApiLlmClient::new(test_config(LlmProviderKind::Ollama), personality);
        let prompt = client.system_prompt();
        assert!(prompt.contains("Fippy Darkpaw"));
        assert!(prompt.contains("Also mention loot drops."));
    }

    #[test]
    fn situation_to_prompt_fleet_commentary() {
        let req = test_request();
        let prompt = ApiLlmClient::situation_to_prompt(&req);
        assert!(prompt.contains("Fleet Update"));
        assert!(prompt.contains("wiped"));
    }

    #[test]
    fn situation_to_prompt_all_variants() {
        let situations = vec![
            Situation::GameEvent {
                description: "Ding 50".into(),
            },
            Situation::CombatReaction {
                description: "Killed Nagafen".into(),
            },
            Situation::PlayerChat {
                player_name: "Dave".into(),
                message: "Hello".into(),
                channel: "say".into(),
            },
            Situation::BotChat {
                character_name: "Grimjaw".into(),
                message: "Hey".into(),
            },
            Situation::IdleChatter,
            Situation::FleetCommentary {
                event_summary: "Camp started".into(),
            },
        ];
        for sit in situations {
            let req = LlmRequest {
                situation: sit,
                ..test_request()
            };
            let prompt = ApiLlmClient::situation_to_prompt(&req);
            assert!(!prompt.is_empty());
        }
    }

    #[test]
    fn none_provider_generate_fails() {
        let mut client = ApiLlmClient::new(test_config(LlmProviderKind::None), test_personality());
        let result = client.generate(&test_request());
        assert!(result.is_err());
    }

    #[test]
    fn preset_system_prompts_are_distinct() {
        let presets = [
            BotPersonalityPreset::FippyDarkpaw,
            BotPersonalityPreset::DruzzilRo,
            BotPersonalityPreset::Bristlebane,
            BotPersonalityPreset::Custom,
        ];
        let prompts: Vec<&str> = presets.iter().map(|p| p.system_prompt()).collect();
        for i in 0..prompts.len() {
            for j in (i + 1)..prompts.len() {
                assert_ne!(prompts[i], prompts[j]);
            }
        }
    }

    #[test]
    fn provider_names() {
        for (kind, expected) in [
            (LlmProviderKind::Ollama, "ollama"),
            (LlmProviderKind::None, "none"),
        ] {
            let client = ApiLlmClient::new(test_config(kind), test_personality());
            assert_eq!(client.name(), expected);
        }
    }
}
