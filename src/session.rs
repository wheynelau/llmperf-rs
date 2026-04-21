use crate::api::models::{ChatCompletionRequest, Message};
use crate::prompt::PromptConfig;
use std::sync::Arc;

/// Multi-turn conversation session.
///
/// Manages the state of a conversation across multiple turns. Each turn consists of
/// a user prompt and an assistant response. The session accumulates message history
/// as turns progress.
///
/// For single-turn mode, `num_turns = 1` and the session completes after one request.
pub struct MultiTurnSession {
    pub num_turns: usize,
    pub messages: Vec<Message>,
    pub turn_index: usize,
    pub model: String,
    pub max_tokens: u32,
    pub thinking: bool,
}

impl MultiTurnSession {
    /// Create a new multi-turn session.
    /// Initializes with the first user prompt already in the message history.
    pub fn new(
        num_turns: usize,
        initial_prompt: String,
        model: String,
        max_tokens: u32,
        thinking: bool,
    ) -> Self {
        let messages = vec![Message {
            role: "user".to_string(),
            content: Arc::from(initial_prompt),
        }];

        MultiTurnSession {
            num_turns,
            messages,
            turn_index: 0,
            model,
            max_tokens,
            thinking,
        }
    }

    /// Check if all turns have been completed.
    /// A session is complete when we've received `num_turns` assistant responses.
    /// Since each turn adds one user message and one assistant message, we check
    /// if the number of assistant messages equals `num_turns`.
    pub fn is_complete(&self) -> bool {
        self.turn_index >= self.num_turns
    }

    /// Store the assistant's response and optionally advance to the next turn.
    /// Appends the assistant's response to the message history. If there are more
    /// turns to complete, appends the String.
    pub fn store_response_and_advance(&mut self, content: Arc<str>, config: &PromptConfig) {
        // Append assistant response
        self.messages.push(Message {
            role: "assistant".to_string(),
            content,
        });

        // Advance turn index
        self.turn_index += 1;

        if !self.is_complete() {
            let (prompt, _) = config.generate_turn_prompt();
            self.messages.push(Message {
                role: "user".to_string(),
                content: Arc::from(prompt),
            });
        }
    }

    /// Build a ChatCompletionRequest for the current turn.
    ///
    /// Uses the accumulated message history as the conversation context.
    pub fn build_request(&self) -> ChatCompletionRequest {
        ChatCompletionRequest::from_messages(
            self.model.clone(),
            self.messages.clone(),
            self.max_tokens,
            true,
            self.thinking,
        )
    }

    /// Get the current turn number (1-based for readability).
    pub fn current_turn(&self) -> usize {
        self.turn_index + 1
    }
}
