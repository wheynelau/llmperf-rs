use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    #[serde(alias = "max_tokens")]
    Stop,
    #[serde(alias = "end_turn")]
    Length,
    ContentFilter,
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug)]
pub struct StreamResponse {
    pub choices: Vec<StreamChoice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct PromptTokensDetails {
    pub cached_tokens: Option<u32>,
    pub cached_read_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct Usage {
    pub completion_tokens: u32,
    pub prompt_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: Option<FinishReason>,
    /// Anthropic sends `stop_reason` instead of `finish_reason`
    pub stop_reason: Option<FinishReason>,
    pub token_ids: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning: Option<String>,
    pub reasoning_content: Option<String>,
}

#[derive(Default)]
pub struct Request {
    pub url: String,
    pub api_key: Option<String>,
    pub chat_completion: ChatCompletionRequest,
    pub headers: HashMap<String, String>,
}

#[derive(Default, Serialize)]
pub struct ChatTemplateKwargs {
    pub enable_thinking: bool,
    pub thinking: bool,
}

impl ChatTemplateKwargs {
    pub fn new(thinking: bool) -> Self {
        ChatTemplateKwargs {
            enable_thinking: thinking,
            thinking,
        }
    }
}

impl Request {
    /// Creates a new Request instance.
    pub fn new(
        url: impl Into<String>,
        api_key: Option<String>,
        chat_completion: ChatCompletionRequest,
    ) -> Self {
        let base_url = url.into();

        let base_url = base_url.trim_end_matches('/');

        Request {
            url: format!("{base_url}/chat/completions"),
            api_key,
            chat_completion,
            headers: HashMap::new(),
        }
    }
}
/// This is the body of the request for openai completions
#[derive(Serialize, Default)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub max_tokens: u32,
    pub stream_options: StreamOptions,
    pub chat_template_kwargs: Option<ChatTemplateKwargs>,
}
#[derive(Serialize, Default)]
pub struct StreamOptions {
    pub include_usage: bool,
}

impl ChatCompletionRequest {
    /// Creates a `ChatCompletionRequest` with existing messages (for multi-turn conversations)
    pub fn from_messages(
        model: impl Into<String>,
        messages: Vec<Message>,
        max_tokens: u32,
        stream: bool,
        thinking: bool,
    ) -> Self {
        let chat_template_kwargs = if thinking {
            None
        } else {
            Some(ChatTemplateKwargs::new(thinking))
        };
        ChatCompletionRequest {
            model: model.into(),
            messages,
            max_tokens,
            stream,
            stream_options: StreamOptions {
                include_usage: true,
            },
            chat_template_kwargs,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: Arc<str>,
    #[serde(rename = "reasoning_content", skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Arc<str>>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "status_code")]
    pub status_code: u16,
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct ModelList {
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub id: String,
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    #[test]
    fn usage_parses_prompt_tokens_details() {
        let json = r#"{
            "completion_tokens": 100,
            "prompt_tokens": 53,
            "total_tokens": 153,
            "prompt_tokens_details": {"cached_tokens": 32}
        }"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        let details = usage.prompt_tokens_details.as_ref().unwrap();
        assert_eq!(details.cached_tokens, Some(32));
        assert_eq!(details.cached_read_tokens, None);
    }

    #[test]
    fn usage_without_prompt_tokens_details_deserializes() {
        let json = r#"{
            "completion_tokens": 100,
            "prompt_tokens": 53,
            "total_tokens": 153
        }"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        assert!(usage.prompt_tokens_details.is_none());
    }

    #[test]
    fn prompt_tokens_details_partial_fields_default_to_zero() {
        let json = r#"{
            "completion_tokens": 100,
            "prompt_tokens": 53,
            "total_tokens": 153,
            "prompt_tokens_details": {"cached_tokens": 32}
        }"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        let details = usage.prompt_tokens_details.as_ref().unwrap();
        assert_eq!(details.cached_tokens, Some(32));
        assert_eq!(details.cached_read_tokens, None);
    }

    #[test]
    fn prompt_tokens_details_only_cached_read_tokens() {
        let json = r#"{
            "completion_tokens": 100,
            "prompt_tokens": 53,
            "total_tokens": 153,
            "prompt_tokens_details": {"cached_read_tokens": 64}
        }"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        let details = usage.prompt_tokens_details.as_ref().unwrap();
        assert_eq!(details.cached_tokens, None);
        assert_eq!(details.cached_read_tokens, Some(64));
    }

    #[test]
    fn prompt_tokens_details_accepts_dual_keys_without_duplicate_field_error() {
        let json = r#"{
            "completion_tokens": 100,
            "prompt_tokens": 53,
            "total_tokens": 153,
            "prompt_tokens_details": {"cached_tokens": 384, "cached_read_tokens": 512}
        }"#;
        let usage: Usage = serde_json::from_str(json)
            .expect("dual keys must not trigger serde_json duplicate-field error");
        let details = usage
            .prompt_tokens_details
            .as_ref()
            .expect("details present");
        assert_eq!(details.cached_tokens, Some(384));
        assert_eq!(details.cached_read_tokens, Some(512));
    }
}
