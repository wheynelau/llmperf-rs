use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FinishReason {
    #[serde(rename = "stop", alias = "max_tokens")]
    Stop,
    #[serde(rename = "length", alias = "end_turn")]
    Length,
}

#[derive(Deserialize, Debug)]
pub struct StreamResponse {
    pub choices: Vec<StreamChoice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Debug)]
pub struct Usage {
    pub completion_tokens: u32,
    pub prompt_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub logprobs: Option<serde_json::Value>,
    // stop reason is for anthropic
    #[serde(alias = "stop_reason")]
    pub finish_reason: Option<FinishReason>,
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
    /// Creates a ChatCompletionRequest with existing messages (for multi-turn conversations)
    pub fn from_messages(
        model: impl Into<String>,
        messages: Vec<Message>,
        max_tokens: u32,
        stream: bool,
        thinking: bool,
    ) -> Self {
        let chat_template_kwargs = if !thinking {
            Some(ChatTemplateKwargs::new(thinking))
        } else {
            None
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
