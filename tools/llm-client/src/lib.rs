use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};
use thiserror::Error;

pub const MAX_NARRATIVE_WORDS: usize = 200;
pub const MAX_LESSONS: usize = 3;
pub const MAX_MEMORY_CUES: usize = 5;
pub const DEFAULT_ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-haiku-4-5";
pub const DEFAULT_LLAMA_ENDPOINT: &str = "http://127.0.0.1:8080/v1/chat/completions";
pub const DEFAULT_LLAMA_MODEL: &str = "llama-3.1-8b";
pub const DEFAULT_HTTP_TIMEOUT_SECONDS: u64 = 10;
pub const DEBRIEF_TOOL_NAME: &str = "emit_debrief";
pub const DEBRIEF_GBNF: &str = r#"
root ::= object
object ::= "{" ws "\"narrative\"" ws ":" ws string ws "," ws "\"highlight_event_id\"" ws ":" ws nullable_string ws "," ws "\"lessons\"" ws ":" ws string_array ws "," ws "\"mood_delta\"" ws ":" ws mood ws "," ws "\"callouts\"" ws ":" ws callout_array ws "}"
nullable_string ::= string | "null"
string_array ::= "[" ws string_list? ws "]"
string_list ::= string (ws "," ws string)*
callout_array ::= "[" ws callout_list? ws "]"
callout_list ::= callout (ws "," ws callout)*
callout ::= "{" ws "\"to_character\"" ws ":" ws string ws "," ws "\"line\"" ws ":" ws string ws "}"
mood ::= "\"improved\"" | "\"same\"" | "\"worsened\""
string ::= "\"" chars "\""
chars ::= char*
char ::= [^"\\] | "\\" ["\\/bfnrt] | "\\" "u" hex hex hex hex
hex ::= [0-9a-fA-F]
ws ::= [ \t\n\r]*
"#;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("missing field `{0}`")]
    MissingField(&'static str),
    #[error("unsupported backend `{0}`")]
    UnsupportedBackend(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Character {
    pub name: String,
    pub level: u32,
    pub class: String,
    pub voice_profile: String,
    #[serde(default)]
    pub speech_style: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionDebrief {
    #[serde(default)]
    pub recent_memories: Vec<String>,
    #[serde(default)]
    pub party: Vec<String>,
    #[serde(default)]
    pub notable_event_ids: Vec<String>,
    #[serde(default)]
    pub details: JsonValue,
}

impl SessionDebrief {
    pub fn to_pretty_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Callout {
    pub to_character: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoodDelta {
    Improved,
    Same,
    Worsened,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Debrief {
    pub narrative: String,
    pub highlight_event_id: Option<String>,
    #[serde(default)]
    pub lessons: Vec<String>,
    pub mood_delta: MoodDelta,
    #[serde(default)]
    pub callouts: Vec<Callout>,
}

pub type DebriefRaw = Debrief;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendSelection {
    Anthropic,
    LlamaCpp,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub backend: BackendSelection,
    #[serde(default)]
    pub anthropic: AnthropicConfig,
    #[serde(default)]
    pub llama_cpp: LlamaCppConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicConfig {
    #[serde(default = "default_anthropic_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_anthropic_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_anthropic_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_true")]
    pub prompt_cache: bool,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: Option<u64>,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            endpoint: default_anthropic_endpoint(),
            model: default_anthropic_model(),
            api_key: None,
            max_tokens: default_anthropic_max_tokens(),
            prompt_cache: true,
            timeout_seconds: default_timeout_seconds(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlamaCppConfig {
    #[serde(default = "default_llama_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_llama_model")]
    pub model: String,
    #[serde(default = "default_llama_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub parallel_slots: Option<u32>,
}

impl Default for LlamaCppConfig {
    fn default() -> Self {
        Self {
            endpoint: default_llama_endpoint(),
            model: default_llama_model(),
            max_tokens: default_llama_max_tokens(),
            temperature: 0.0,
            timeout_seconds: default_timeout_seconds(),
            parallel_slots: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicBackend {
    client: Client,
    endpoint: String,
    model: String,
    api_key: String,
    max_tokens: u32,
    prompt_cache: bool,
}

#[derive(Debug, Clone)]
pub struct LlamaCppBackend {
    client: Client,
    endpoint: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Clone)]
pub enum ConfiguredBackend {
    Anthropic(AnthropicBackend),
    LlamaCpp(LlamaCppBackend),
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    messages: Vec<AnthropicMessage>,
    tools: Vec<AnthropicTool>,
    tool_choice: AnthropicToolChoice,
}

#[derive(Debug, Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    kind: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Serialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: JsonValue,
}

#[derive(Debug, Serialize)]
struct AnthropicToolChoice {
    #[serde(rename = "type")]
    kind: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicResponseBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponseBlock {
    #[serde(rename = "type")]
    kind: String,
    name: Option<String>,
    input: Option<JsonValue>,
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct LlamaRequest {
    model: String,
    messages: Vec<LlamaMessage>,
    temperature: f32,
    top_p: f32,
    max_tokens: u32,
    stream: bool,
    grammar: String,
}

#[derive(Debug, Serialize)]
struct LlamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct LlamaResponse {
    choices: Vec<LlamaChoice>,
}

#[derive(Debug, Deserialize)]
struct LlamaChoice {
    message: LlamaMessageResponse,
}

#[derive(Debug, Deserialize)]
struct LlamaMessageResponse {
    content: Option<String>,
    tool_calls: Option<Vec<LlamaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct LlamaToolCall {
    function: LlamaFunctionCall,
}

#[derive(Debug, Deserialize)]
struct LlamaFunctionCall {
    arguments: String,
}

impl AnthropicBackend {
    pub fn new(config: AnthropicConfig) -> Result<Self, Error> {
        let api_key = config
            .api_key
            .or_else(resolve_anthropic_api_key)
            .ok_or(Error::MissingField("anthropic.api_key"))?;
        let client = build_client(config.timeout_seconds)?;
        Ok(Self {
            client,
            endpoint: config.endpoint,
            model: config.model,
            api_key,
            max_tokens: config.max_tokens,
            prompt_cache: config.prompt_cache,
        })
    }

    fn request_payload(
        &self,
        facts: &SessionDebrief,
        ch: &Character,
        strict: bool,
    ) -> Result<AnthropicRequest, Error> {
        let prompt = build_prompt(facts, ch, strict)?;
        let system = vec![SystemBlock {
            kind: "text".to_string(),
            text: prompt.system,
            cache_control: if self.prompt_cache {
                Some(CacheControl {
                    kind: "ephemeral".to_string(),
                })
            } else {
                None
            },
        }];
        let messages = vec![AnthropicMessage {
            role: "user".to_string(),
            content: vec![AnthropicContentBlock {
                kind: "text".to_string(),
                text: prompt.user,
            }],
        }];
        Ok(AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system,
            messages,
            tools: vec![AnthropicTool {
                name: prompt.tool_name.to_string(),
                description: "Emit a structured debrief for a finished session.".to_string(),
                input_schema: prompt.output_schema,
            }],
            tool_choice: AnthropicToolChoice {
                kind: "tool".to_string(),
                name: prompt.tool_name.to_string(),
            },
        })
    }
}

impl LlamaCppBackend {
    pub fn new(config: LlamaCppConfig) -> Result<Self, Error> {
        let client = build_client(config.timeout_seconds)?;
        Ok(Self {
            client,
            endpoint: config.endpoint,
            model: config.model,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }

    fn request_payload(
        &self,
        facts: &SessionDebrief,
        ch: &Character,
        strict: bool,
    ) -> Result<LlamaRequest, Error> {
        let prompt = build_prompt(facts, ch, strict)?;
        Ok(LlamaRequest {
            model: self.model.clone(),
            messages: vec![
                LlamaMessage {
                    role: "system".to_string(),
                    content: prompt.system,
                },
                LlamaMessage {
                    role: "user".to_string(),
                    content: prompt.user,
                },
            ],
            temperature: self.temperature,
            top_p: 1.0,
            max_tokens: self.max_tokens,
            stream: false,
            grammar: DEBRIEF_GBNF.to_string(),
        })
    }
}

impl ConfiguredBackend {
    pub fn from_config(config: LlmConfig) -> Result<Self, Error> {
        match config.backend {
            BackendSelection::Anthropic => {
                AnthropicBackend::new(config.anthropic).map(Self::Anthropic)
            }
            BackendSelection::LlamaCpp => {
                LlamaCppBackend::new(config.llama_cpp).map(Self::LlamaCpp)
            }
        }
    }
}

pub async fn run(facts: &SessionDebrief, ch: &Character) -> Result<Debrief, Error> {
    let backend = load_default_backend()?;
    run_with_backend(&backend, facts, ch).await
}

pub async fn run_with_backend<B: LlmBackend + Sync>(
    backend: &B,
    facts: &SessionDebrief,
    ch: &Character,
) -> Result<Debrief, Error> {
    let attempts = [false, true];
    let mut last_error: Option<Error> = None;

    for strict in attempts {
        let tool_name = if strict {
            "emit_debrief::strict"
        } else {
            DEBRIEF_TOOL_NAME
        };

        match backend.call_tool(tool_name, facts, ch).await {
            Ok(raw) => match validate_debrief(&raw, facts) {
                Ok(()) => return Ok(raw),
                Err(err) => {
                    tracing::warn!(attempt = strict, error = %err, "debrief validation failed");
                    last_error = Some(err);
                }
            },
            Err(err) => {
                tracing::warn!(attempt = strict, error = %err, "debrief backend call failed");
                last_error = Some(err);
            }
        }
    }

    tracing::warn!(error = ?last_error, "using deterministic debrief fallback");
    Ok(deterministic_fallback(facts, ch))
}

#[async_trait]
pub trait LlmBackend {
    async fn call_tool(
        &self,
        tool: &str,
        facts: &SessionDebrief,
        ch: &Character,
    ) -> Result<DebriefRaw, Error>;
}

#[async_trait]
impl LlmBackend for AnthropicBackend {
    async fn call_tool(
        &self,
        tool: &str,
        facts: &SessionDebrief,
        ch: &Character,
    ) -> Result<DebriefRaw, Error> {
        let tool_name = normalize_tool_name(tool)?;
        let payload = self.request_payload(facts, ch, tool_name.strict)?;
        let response = self
            .client
            .post(&self.endpoint)
            .header("anthropic-version", "2023-06-01")
            .header("x-api-key", &self.api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        let parsed: AnthropicResponse = response.json().await?;
        parse_anthropic_response(parsed, tool_name.base)
    }
}

#[async_trait]
impl LlmBackend for LlamaCppBackend {
    async fn call_tool(
        &self,
        tool: &str,
        facts: &SessionDebrief,
        ch: &Character,
    ) -> Result<DebriefRaw, Error> {
        let tool_name = normalize_tool_name(tool)?;
        let payload = self.request_payload(facts, ch, tool_name.strict)?;
        let response = self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        let parsed: LlamaResponse = response.json().await?;
        parse_llama_response(parsed)
    }
}

#[async_trait]
impl LlmBackend for ConfiguredBackend {
    async fn call_tool(
        &self,
        tool: &str,
        facts: &SessionDebrief,
        ch: &Character,
    ) -> Result<DebriefRaw, Error> {
        match self {
            Self::Anthropic(backend) => backend.call_tool(tool, facts, ch).await,
            Self::LlamaCpp(backend) => backend.call_tool(tool, facts, ch).await,
        }
    }
}

fn build_prompt(facts: &SessionDebrief, ch: &Character, strict: bool) -> Result<PromptSpec, Error> {
    let recent_memories = facts
        .recent_memories
        .iter()
        .take(MAX_MEMORY_CUES)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    let speech_style = if ch.speech_style.is_empty() {
        String::new()
    } else {
        ch.speech_style.join(", ")
    };
    let system = build_system_prompt(ch, &recent_memories, &speech_style, strict);
    let user = build_user_prompt(facts, ch)?;
    Ok(PromptSpec {
        system,
        user,
        tool_name: DEBRIEF_TOOL_NAME,
        output_schema: debrief_input_schema(ch),
    })
}

fn build_system_prompt(
    ch: &Character,
    recent_memories: &str,
    speech_style: &str,
    strict: bool,
) -> String {
    let mut prompt = format!(
        "You are the post-session narrator for {name}, a level {level} {class} in EverQuest.\nVoice profile: {voice_profile}\nSpeech style tokens: {speech_style}\nMemory cues (most-recent-first, 5 max): {recent_memories}\n\nHARD RULES:\n1. You MUST use only the numeric values present in <facts>. Do not invent numbers, percentages, dps, mob counts, gold amounts, or zone names.\n2. Output exactly the schema in <output_schema>. No prose outside the schema.\n3. Stay in voice. Refer to party members by the names in <facts.party>.\n4. 200 words max in `narrative`. Three bullets max in `lessons`.\n5. If a field in <facts> is null, do not reference that topic.",
        name = ch.name,
        level = ch.level,
        class = ch.class,
        voice_profile = ch.voice_profile,
        speech_style = speech_style,
        recent_memories = recent_memories,
    );
    if strict {
        prompt.push_str(
            "\n\nSTRICT RETRY:\n- Do not infer any number that is not explicitly grounded in <facts>.\n- Prefer omission over approximation.\n- Keep the result terse and literal.\n",
        );
    }
    prompt
}

fn build_user_prompt(facts: &SessionDebrief, ch: &Character) -> Result<String, Error> {
    let facts_json = facts.to_pretty_json()?;
    Ok(format!(
        "<facts>\n{facts_json}\n</facts>\n\n<output_schema>\n{schema}\n</output_schema>",
        schema = output_schema_text(ch)
    ))
}

fn output_schema_text(ch: &Character) -> String {
    format!(
        "{{\n  \"narrative\": \"string, <=200 words, first-person from {name}\",\n  \"highlight_event_id\": \"string | null   // must equal one of facts.notable_event_ids\",\n  \"lessons\": [\"string\", \"...\"],\n  \"mood_delta\": \"improved|same|worsened\",\n  \"callouts\": [{{\"to_character\": \"string\", \"line\": \"string\"}}]\n}}",
        name = ch.name
    )
}

fn debrief_input_schema(ch: &Character) -> JsonValue {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["narrative", "highlight_event_id", "lessons", "mood_delta", "callouts"],
        "properties": {
            "narrative": {
                "type": "string",
                "description": format!("<=200 words, first-person from {}", ch.name),
                "maxLength": 2000
            },
            "highlight_event_id": {
                "anyOf": [
                    {"type": "string"},
                    {"type": "null"}
                ]
            },
            "lessons": {
                "type": "array",
                "maxItems": 3,
                "items": {"type": "string"}
            },
            "mood_delta": {
                "type": "string",
                "enum": ["improved", "same", "worsened"]
            },
            "callouts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["to_character", "line"],
                    "properties": {
                        "to_character": {"type": "string"},
                        "line": {"type": "string"}
                    }
                }
            }
        }
    })
}

fn parse_anthropic_response(
    response: AnthropicResponse,
    tool_name: &str,
) -> Result<DebriefRaw, Error> {
    for block in response.content {
        if block.kind == "tool_use" && block.name.as_deref().is_none_or(|name| name == tool_name)
            && let Some(input) = block.input {
                return Ok(serde_json::from_value(input)?);
            }

        if block.kind == "text"
            && let Some(text) = block.text
                && let Ok(raw) = serde_json::from_str::<DebriefRaw>(&text) {
                    return Ok(raw);
                }
    }

    Err(Error::MissingField("anthropic tool_use payload"))
}

fn parse_llama_response(response: LlamaResponse) -> Result<DebriefRaw, Error> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or(Error::MissingField("choices[0]"))?;

    if let Some(content) = choice.message.content {
        return Ok(serde_json::from_str(&content)?);
    }

    if let Some(tool_calls) = choice.message.tool_calls
        && let Some(tool_call) = tool_calls.into_iter().next() {
            return Ok(serde_json::from_str(&tool_call.function.arguments)?);
        }

    Err(Error::MissingField("llama response content"))
}

fn validate_debrief(raw: &DebriefRaw, facts: &SessionDebrief) -> Result<(), Error> {
    validate_word_limit(&raw.narrative)?;
    validate_lessons_count(raw)?;
    validate_party_names(raw, facts)?;
    validate_numbers(raw, facts)?;
    validate_event_ids(raw, facts)?;
    Ok(())
}

fn validate_word_limit(narrative: &str) -> Result<(), Error> {
    let words = narrative.split_whitespace().count();
    if words > MAX_NARRATIVE_WORDS {
        return Err(Error::Validation(format!(
            "narrative exceeds {} words: {}",
            MAX_NARRATIVE_WORDS, words
        )));
    }
    Ok(())
}

fn validate_lessons_count(raw: &DebriefRaw) -> Result<(), Error> {
    if raw.lessons.len() > MAX_LESSONS {
        return Err(Error::Validation(format!(
            "lessons exceed {} entries: {}",
            MAX_LESSONS,
            raw.lessons.len()
        )));
    }
    Ok(())
}

fn validate_party_names(raw: &DebriefRaw, facts: &SessionDebrief) -> Result<(), Error> {
    if facts.party.is_empty() {
        if raw.callouts.is_empty() {
            return Ok(());
        }
        return Err(Error::Validation(
            "callouts present but facts.party is empty".to_string(),
        ));
    }

    let allowed: HashSet<&str> = facts.party.iter().map(String::as_str).collect();
    for callout in &raw.callouts {
        if !allowed.contains(callout.to_character.as_str()) {
            return Err(Error::Validation(format!(
                "callout references unknown party member `{}`",
                callout.to_character
            )));
        }
    }
    Ok(())
}

fn validate_numbers(raw: &DebriefRaw, facts: &SessionDebrief) -> Result<(), Error> {
    let facts_json = facts.to_pretty_json()?;
    let allowed = collect_allowed_number_tokens(&facts_json);
    let visible_text = collect_visible_text(raw);

    for token in number_tokens(&visible_text) {
        if is_known_ordinal(token) {
            continue;
        }
        let canonical = canonical_number_token(token);
        if !allowed.contains(&canonical) {
            return Err(Error::Validation(format!(
                "fabricated numeric token `{}` not grounded in facts",
                token
            )));
        }
    }

    Ok(())
}

fn validate_event_ids(raw: &DebriefRaw, facts: &SessionDebrief) -> Result<(), Error> {
    match &raw.highlight_event_id {
        None => Ok(()),
        Some(event_id)
            if facts
                .notable_event_ids
                .iter()
                .any(|candidate| candidate == event_id) =>
        {
            Ok(())
        }
        Some(event_id) => Err(Error::Validation(format!(
            "highlight_event_id `{}` is not one of facts.notable_event_ids",
            event_id
        ))),
    }
}

fn collect_visible_text(raw: &DebriefRaw) -> String {
    let mut parts = vec![raw.narrative.clone()];
    parts.extend(raw.lessons.iter().cloned());
    parts.extend(raw.callouts.iter().map(|callout| callout.line.clone()));
    parts.join("\n")
}

fn collect_allowed_number_tokens(facts_json: &str) -> HashSet<String> {
    let mut allowed = HashSet::new();
    for token in number_tokens(facts_json) {
        allowed.insert(canonical_number_token(token));
    }
    allowed
}

fn number_tokens(text: &str) -> Vec<&str> {
    number_regex()
        .captures_iter(text)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str()))
        .collect()
}

fn number_regex() -> &'static Regex {
    static NUMBER_RE: OnceLock<Regex> = OnceLock::new();
    NUMBER_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:^|[^A-Za-z0-9])(-?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?(?:st|nd|rd|th)?)(?:$|[^A-Za-z0-9])")
            .expect("valid numeric regex")
    })
}

fn canonical_number_token(token: &str) -> String {
    let mut value = token.replace(',', "");
    let lower = value.to_ascii_lowercase();
    let suffix_len = if lower.ends_with("st")
        || lower.ends_with("nd")
        || lower.ends_with("rd")
        || lower.ends_with("th")
    {
        2
    } else {
        0
    };
    if suffix_len > 0 {
        value.truncate(value.len() - suffix_len);
    }

    let mut negative = false;
    if let Some(rest) = value.strip_prefix('+') {
        value = rest.to_string();
    }
    if let Some(rest) = value.strip_prefix('-') {
        negative = true;
        value = rest.to_string();
    }

    let (int_part, frac_part) = match value.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (value.as_str(), None),
    };

    let int_part = int_part.trim_start_matches('0');
    let int_part = if int_part.is_empty() { "0" } else { int_part };

    let mut canonical = String::new();
    if negative && !(int_part == "0" && frac_part.is_none()) {
        canonical.push('-');
    }
    canonical.push_str(int_part);

    if let Some(frac_part) = frac_part {
        let frac_part = frac_part.trim_end_matches('0');
        if !frac_part.is_empty() {
            canonical.push('.');
            canonical.push_str(frac_part);
        }
    }

    if canonical == "-0" {
        "0".to_string()
    } else {
        canonical
    }
}

fn is_known_ordinal(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.ends_with("st") || lower.ends_with("nd") || lower.ends_with("rd") || lower.ends_with("th")
}

fn normalize_tool_name(tool: &str) -> Result<NormalizedToolName<'_>, Error> {
    if let Some(base) = tool.strip_suffix("::strict") {
        if base != DEBRIEF_TOOL_NAME {
            return Err(Error::UnsupportedBackend(tool.to_string()));
        }
        return Ok(NormalizedToolName { base, strict: true });
    }

    if tool != DEBRIEF_TOOL_NAME {
        return Err(Error::UnsupportedBackend(tool.to_string()));
    }

    Ok(NormalizedToolName {
        base: DEBRIEF_TOOL_NAME,
        strict: false,
    })
}

fn default_anthropic_endpoint() -> String {
    DEFAULT_ANTHROPIC_ENDPOINT.to_string()
}

fn default_anthropic_model() -> String {
    DEFAULT_ANTHROPIC_MODEL.to_string()
}

fn default_anthropic_max_tokens() -> u32 {
    1024
}

fn default_llama_endpoint() -> String {
    DEFAULT_LLAMA_ENDPOINT.to_string()
}

fn default_llama_model() -> String {
    DEFAULT_LLAMA_MODEL.to_string()
}

fn default_llama_max_tokens() -> u32 {
    1024
}

fn default_true() -> bool {
    true
}

fn default_timeout_seconds() -> Option<u64> {
    Some(DEFAULT_HTTP_TIMEOUT_SECONDS)
}

fn resolve_anthropic_api_key() -> Option<String> {
    env::var("ANTHROPIC_API_KEY")
        .ok()
        .or_else(|| env::var("TEXTQUEST_ANTHROPIC_API_KEY").ok())
}

fn build_client(timeout_seconds: Option<u64>) -> Result<Client, Error> {
    let timeout_seconds = timeout_seconds.unwrap_or(DEFAULT_HTTP_TIMEOUT_SECONDS);
    let builder = Client::builder().timeout(Duration::from_secs(timeout_seconds));
    Ok(builder.build()?)
}

fn default_config_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".textquest/llm.toml")
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

pub fn load_config(path: impl AsRef<Path>) -> Result<LlmConfig, Error> {
    let contents = fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

pub fn load_default_backend() -> Result<ConfiguredBackend, Error> {
    let path = default_config_path();
    let config = load_config(path)?;
    ConfiguredBackend::from_config(config)
}

#[derive(Debug)]
struct PromptSpec {
    system: String,
    user: String,
    tool_name: &'static str,
    output_schema: JsonValue,
}

#[derive(Debug)]
struct NormalizedToolName<'a> {
    base: &'a str,
    strict: bool,
}

fn deterministic_fallback(facts: &SessionDebrief, ch: &Character) -> Debrief {
    let narrative = if facts.party.is_empty() {
        format!(
            "I kept my footing and finished the session with {} composure.",
            ch.name
        )
    } else {
        format!(
            "I stayed with the group and kept the session steady for {}.",
            facts.party.join(", ")
        )
    };

    let lessons = if facts.recent_memories.is_empty() {
        vec![
            "Keep the plan simple and grounded in the pull at hand.".to_string(),
            "Let the group settle before taking extra risks.".to_string(),
        ]
    } else {
        vec![
            "Stay anchored to the most recent fight rhythm.".to_string(),
            "Call out the next step before the pull gets busy.".to_string(),
        ]
    };

    let callouts = facts
        .party
        .first()
        .map(|party_member| Callout {
            to_character: party_member.clone(),
            line: "Thanks for keeping the pace steady.".to_string(),
        })
        .into_iter()
        .collect();

    Debrief {
        narrative,
        highlight_event_id: facts.notable_event_ids.first().cloned(),
        lessons,
        mood_delta: MoodDelta::Same,
        callouts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    fn fixture_character() -> Character {
        Character {
            name: "Liora".to_string(),
            level: 60,
            class: "cleric".to_string(),
            voice_profile: "measured, devotional, dry wit".to_string(),
            speech_style: vec![
                "short sentences".to_string(),
                "battlefield calm".to_string(),
            ],
        }
    }

    fn fixture_facts() -> SessionDebrief {
        SessionDebrief {
            recent_memories: vec![
                "Held the north hallway".to_string(),
                "Kept the pull at 3 mobs".to_string(),
            ],
            party: vec!["Borin".to_string(), "Mira".to_string()],
            notable_event_ids: vec!["evt-1".to_string(), "evt-2".to_string()],
            details: json!({
                "zone": "Sebilis",
                "kills": 3,
                "loot": 12,
                "mood_before": "same"
            }),
        }
    }

    fn fixture_debrief() -> DebriefRaw {
        DebriefRaw {
            narrative: "I kept the line steady and helped Borin and Mira finish the pull without losing tempo.".to_string(),
            highlight_event_id: Some("evt-1".to_string()),
            lessons: vec![
                "Stay patient when the hallway gets crowded.".to_string(),
                "Keep the rhythm simple when the group is already stable.".to_string(),
            ],
            mood_delta: MoodDelta::Improved,
            callouts: vec![Callout {
                to_character: "Borin".to_string(),
                line: "Good footing on that hallway hold.".to_string(),
            }],
        }
    }

    #[test]
    fn backend_config_defaults_set_request_timeout() {
        assert_eq!(
            AnthropicConfig::default().timeout_seconds,
            Some(DEFAULT_HTTP_TIMEOUT_SECONDS)
        );
        assert_eq!(
            LlamaCppConfig::default().timeout_seconds,
            Some(DEFAULT_HTTP_TIMEOUT_SECONDS)
        );
    }

    #[tokio::test]
    async fn anthropic_backend_parses_tool_use_response() {
        let server = MockServer::start().await;
        let body = json!({
            "content": [
                {
                    "type": "tool_use",
                    "name": DEBRIEF_TOOL_NAME,
                    "input": fixture_debrief()
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let backend = AnthropicBackend::new(AnthropicConfig {
            endpoint: format!("{}/v1/messages", server.uri()),
            model: DEFAULT_ANTHROPIC_MODEL.to_string(),
            api_key: Some("test-key".to_string()),
            max_tokens: 256,
            prompt_cache: false,
            timeout_seconds: None,
        })
        .expect("backend");

        let raw = backend
            .call_tool(DEBRIEF_TOOL_NAME, &fixture_facts(), &fixture_character())
            .await
            .expect("raw debrief");

        assert_eq!(raw, fixture_debrief());
    }

    #[tokio::test]
    async fn llama_backend_parses_chat_completion_response() {
        let server = MockServer::start().await;
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": serde_json::to_string(&fixture_debrief()).expect("json")
                    }
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let backend = LlamaCppBackend::new(LlamaCppConfig {
            endpoint: format!("{}/v1/chat/completions", server.uri()),
            model: DEFAULT_LLAMA_MODEL.to_string(),
            max_tokens: 256,
            temperature: 0.0,
            timeout_seconds: None,
            parallel_slots: Some(4),
        })
        .expect("backend");

        let raw = backend
            .call_tool(DEBRIEF_TOOL_NAME, &fixture_facts(), &fixture_character())
            .await
            .expect("raw debrief");

        assert_eq!(raw, fixture_debrief());
    }

    #[tokio::test]
    async fn numeric_allowlist_rejects_fabricated_numbers_and_falls_back() {
        let facts = fixture_facts();
        let ch = fixture_character();
        let backend = ScriptedBackend::new(vec![
            Ok(DebriefRaw {
                narrative:
                    "I kept the pull to 4 mobs and 99 gold while Borin and Mira held the line."
                        .to_string(),
                highlight_event_id: Some("evt-1".to_string()),
                lessons: vec!["Hold the corner".to_string()],
                mood_delta: MoodDelta::Improved,
                callouts: vec![Callout {
                    to_character: "Borin".to_string(),
                    line: "Nice 4-way control.".to_string(),
                }],
            }),
            Ok(DebriefRaw {
                narrative:
                    "I kept the pull to 4 mobs and 99 gold while Borin and Mira held the line."
                        .to_string(),
                highlight_event_id: Some("evt-1".to_string()),
                lessons: vec!["Hold the corner".to_string()],
                mood_delta: MoodDelta::Improved,
                callouts: vec![Callout {
                    to_character: "Borin".to_string(),
                    line: "Nice 4-way control.".to_string(),
                }],
            }),
        ]);

        let result = run_with_backend(&backend, &facts, &ch)
            .await
            .expect("fallback should succeed");

        assert_eq!(result.mood_delta, MoodDelta::Same);
        assert_eq!(result.highlight_event_id.as_deref(), Some("evt-1"));
        assert!(result.narrative.starts_with("I stayed with the group"));
        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn event_id_grounding_rejects_fabricated_ids_and_falls_back() {
        let facts = fixture_facts();
        let ch = fixture_character();
        let backend = ScriptedBackend::new(vec![
            Ok(DebriefRaw {
                narrative: "I kept the line steady and helped Borin and Mira finish the pull without losing tempo.".to_string(),
                highlight_event_id: Some("evt-999".to_string()),
                lessons: vec!["Stay patient".to_string()],
                mood_delta: MoodDelta::Improved,
                callouts: vec![Callout {
                    to_character: "Borin".to_string(),
                    line: "Good footing on that hallway hold.".to_string(),
                }],
            }),
            Ok(DebriefRaw {
                narrative: "I kept the line steady and helped Borin and Mira finish the pull without losing tempo.".to_string(),
                highlight_event_id: Some("evt-999".to_string()),
                lessons: vec!["Stay patient".to_string()],
                mood_delta: MoodDelta::Improved,
                callouts: vec![Callout {
                    to_character: "Borin".to_string(),
                    line: "Good footing on that hallway hold.".to_string(),
                }],
            }),
        ]);

        let result = run_with_backend(&backend, &facts, &ch)
            .await
            .expect("fallback should succeed");

        assert_eq!(result.mood_delta, MoodDelta::Same);
        assert_eq!(result.highlight_event_id.as_deref(), Some("evt-1"));
        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn known_ordinals_are_allowed_without_fact_numbers() {
        let facts = SessionDebrief {
            recent_memories: vec!["First pull felt easy".to_string()],
            party: vec!["Borin".to_string()],
            notable_event_ids: vec!["evt-1".to_string()],
            details: JsonValue::Null,
        };
        let ch = fixture_character();
        let backend = ScriptedBackend::new(vec![Ok(DebriefRaw {
            narrative: "I called the 1st anchor and kept Borin close.".to_string(),
            highlight_event_id: Some("evt-1".to_string()),
            lessons: vec!["Stay with the 1st anchor".to_string()],
            mood_delta: MoodDelta::Improved,
            callouts: vec![Callout {
                to_character: "Borin".to_string(),
                line: "Nice 2nd-step follow through.".to_string(),
            }],
        })]);

        let result = run_with_backend(&backend, &facts, &ch)
            .await
            .expect("ordinal narrative should pass");

        assert_eq!(result.highlight_event_id.as_deref(), Some("evt-1"));
        assert_eq!(result.mood_delta, MoodDelta::Improved);
    }

    #[tokio::test]
    async fn strict_retry_falls_back_when_backend_errors_twice() {
        let facts = fixture_facts();
        let ch = fixture_character();
        let backend = ScriptedBackend::new(vec![
            Err(Error::Validation("temporary schema miss".to_string())),
            Err(Error::Validation("temporary schema miss".to_string())),
        ]);

        let result = run_with_backend(&backend, &facts, &ch)
            .await
            .expect("fallback should succeed");

        assert_eq!(result.highlight_event_id.as_deref(), Some("evt-1"));
        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
    }

    struct ScriptedBackend {
        responses: Mutex<VecDeque<Result<DebriefRaw, Error>>>,
        calls: AtomicUsize,
    }

    impl ScriptedBackend {
        fn new(responses: Vec<Result<DebriefRaw, Error>>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl LlmBackend for ScriptedBackend {
        async fn call_tool(
            &self,
            _tool: &str,
            _facts: &SessionDebrief,
            _ch: &Character,
        ) -> Result<DebriefRaw, Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.responses
                .lock()
                .expect("lock")
                .pop_front()
                .unwrap_or_else(|| Err(Error::Validation("script exhausted".to_string())))
        }
    }
}
