use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FinishReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
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
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: Option<FinishReason>,
    pub stop_reason: Option<serde_json::Value>,
    pub token_ids: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct Request {
    pub url: String,
    pub api_key: Option<String>,
    pub chat_completion: ChatCompletionRequest,
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

#[derive(Serialize, Default)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub max_tokens: u32,
    pub stream_options: StreamOptions,
}
#[derive(Serialize, Default)]
pub struct StreamOptions {
    pub include_usage: bool,
}

impl ChatCompletionRequest {
    pub fn from_prompt(
        model: impl Into<String>,
        prompt: impl Into<String>,
        max_tokens: u32,
        stream: bool,
    ) -> Self {
        let prompt = prompt.into();
        // Original llmperf code had the system message, but it doesn't seem to be necessary
        // Might be a legacy thing
        // Could add as a config option
        let messages = vec![
            // Message {
            //     role: "system".to_string(),
            //     content: "".to_string(),
            // },
            Message {
                role: "user".to_string(),
                content: prompt,
            },
        ];
        ChatCompletionRequest {
            model: model.into(),
            messages,
            max_tokens,
            stream,
            stream_options: StreamOptions {
                include_usage: true,
            },
        }
    }
}

#[derive(Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "status_code")]
    pub status_code: u16,
    pub headers: std::collections::HashMap<String, String>,
}
