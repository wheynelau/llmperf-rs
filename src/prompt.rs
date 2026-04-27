use anyhow::Result;
use futures::Stream;
use futures::stream::{self, StreamExt};
use indicatif::ProgressBar;
use log::warn;
use rand::Rng;
use rand::seq::SliceRandom;
use statrs::distribution::{ContinuousCDF, Normal};
use std::sync::{Arc, Once};
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::api;
use crate::api::models::Request;
use crate::config::AppConfig;
use crate::metrics::{self, Metrics};
use crate::session::MultiTurnSession;

pub const SONNET_TEXT: &str = include_str!("../sonnet.txt");
pub const PROMPT_TEXT: &str =
    "Repeat lines indefinitely from the above text. Don't generate eos tokens:\n";

/// Sample from a truncated normal distribution using the PPF (Percent Point Function) method.
/// This is slower than rejection sampling, but can be a little more stable
pub fn sample_random_positive_int(mean: u32, stddev: u32, min_value: u32) -> u32 {
    if stddev == 0 {
        return mean;
    }

    // unwrap is safe here, we reject stddev == 0 above, and stddev should be positive
    let dist = Normal::new(mean as f64, stddev as f64).unwrap();

    // 1. Get the CDF at the minimum bound
    // This is repeatedly declared, but not necessary to optimize for now
    let p_min = dist.cdf(min_value as f64);

    // 2. Generate a random value between p_min and 1.0
    let mut rng = rand::rng();
    let u: f64 = rng.random_range(p_min..1.0);

    // 3. Use the Percent Point Function (Inverse CDF) to get the sample
    dist.inverse_cdf(u) as u32
}

pub fn randomly_sample_sonnet_lines_prompt(
    prompt_tokens_mean: u32,
    prompt_tokens_stddev: u32,
    prompt_encoding: &tokenizers::Encoding,
    tokenizer: &tokenizers::Tokenizer,
    sonnet_lines: &[tokenizers::Encoding],
) -> (String, u32) {
    let prompt_ids = prompt_encoding.get_ids();

    let prompt_token_len = prompt_ids.len() as u32;

    // Set a safe mean in the event a low mean was set with stddev, potentially creating an infinite loop
    // With the recent change to args.rs this may not be needed
    let safe_mean = std::cmp::max(prompt_tokens_mean, prompt_token_len);

    if prompt_tokens_mean < prompt_token_len {
        static WARN_ONCE: Once = Once::new();
        WARN_ONCE.call_once(|| {
            warn!(
                "prompt_tokens_mean ({prompt_tokens_mean}) is less than base prompt length ({prompt_token_len}). \
                Adjusting mean to ({prompt_token_len})\n\
                This warning will only show once."
            );
        });
    }

    let num_prompt_tokens =
        sample_random_positive_int(safe_mean, prompt_tokens_stddev, prompt_token_len);

    let remaining_prompt_tokens = num_prompt_tokens - prompt_token_len;

    let result_ids = create_prompt(prompt_ids, sonnet_lines, remaining_prompt_tokens);
    let prompt = tokenizer.decode(&result_ids, false).unwrap();
    let actual_token_count = result_ids.len() as u32;

    (prompt, actual_token_count)
}
pub fn create_prompt(
    prompt_ids: &[u32],
    sonnet_lines: &[tokenizers::Encoding],
    remaining_prompt_tokens: u32,
) -> Vec<u32> {
    let mut shuffled_indices: Vec<usize> = (0..sonnet_lines.len()).collect();
    let mut rng = rand::rng();
    shuffled_indices.shuffle(&mut rng);

    let mut result_ids: Vec<u32> =
        Vec::with_capacity(remaining_prompt_tokens as usize + prompt_ids.len());
    let mut remaining = remaining_prompt_tokens;

    // Use cycle to replicate the while loop from the original python code
    for &line_idx in shuffled_indices.iter().cycle() {
        if remaining == 0 {
            break;
        }

        let encoding = &sonnet_lines[line_idx];
        let line_len = encoding.len() as u32;
        let line_ids = encoding.get_ids();

        if line_len <= remaining {
            result_ids.extend(line_ids);
            remaining -= line_len;
        } else {
            result_ids.extend(&line_ids[..remaining as usize]);
            break;
        }
    }

    result_ids.extend(prompt_ids);

    result_ids
}

fn tokenize_sonnet_lines(
    tokenizer: &tokenizers::Tokenizer,
    sonnet_lines: &[String],
) -> Result<Vec<tokenizers::Encoding>> {
    // We need to pass references to encode_batch so we don't consume the lines
    let line_refs: Vec<&str> = sonnet_lines.iter().map(String::as_str).collect();

    let encodings = tokenizer
        .encode_batch_fast(line_refs, false)
        .map_err(|e| anyhow::anyhow!("Failed to encode batch: {e}"))?;

    Ok(encodings)
}

pub fn parse_sonnet_text(
    tokenizer: &tokenizers::Tokenizer,
    sonnet_text: &str,
) -> Result<Arc<[tokenizers::Encoding]>> {
    let lines: Vec<String> = sonnet_text
        .lines()
        .map(|line| line.to_string() + "\n")
        .collect();

    let lines_with_encodings = tokenize_sonnet_lines(tokenizer, &lines)?;

    Ok(Arc::from(lines_with_encodings))
}

fn build_metrics(prompt_tokens: u32, output_tokens: u32, turn_index: usize) -> Metrics {
    let mut metrics = metrics::Metrics::default();
    // Populate the variables we already know
    metrics.number_input_tokens = prompt_tokens;
    metrics.number_output_tokens = output_tokens;
    metrics.number_total_tokens = metrics.number_input_tokens + metrics.number_output_tokens;
    metrics.turn_index = turn_index;

    metrics
}

/// Shared configuration for prompt generation across turns.
#[derive(Clone)]
pub struct PromptConfig {
    prompt_encoding: tokenizers::Encoding,
    sonnet_lines: Arc<[tokenizers::Encoding]>,
    mean_input_tokens: u32,
    stddev_input_tokens: u32,
    tokenizer: tokenizers::Tokenizer,
}

impl PromptConfig {
    pub fn generate_turn_prompt(&self) -> (String, u32) {
        randomly_sample_sonnet_lines_prompt(
            self.mean_input_tokens,
            self.stddev_input_tokens,
            &self.prompt_encoding,
            &self.tokenizer,
            &self.sonnet_lines,
        )
    }
}

struct SessionInput {
    session: MultiTurnSession,
    output_tokens: u32,
    input_tokens: u32,
    config: PromptConfig,
}

fn create_session_inputs(
    sonnet_lines: Arc<[tokenizers::Encoding]>,
    app_config: &AppConfig,
) -> impl Iterator<Item = SessionInput> {
    let prompt_encoding = app_config
        .tokenizer
        .encode_fast(PROMPT_TEXT, false)
        .unwrap();
    let num_turns = app_config.cli_config.multi_turn as usize;
    let model = app_config.cli_config.model.clone();
    let max_tokens = app_config.cli_config.mean_output_tokens;
    let thinking = app_config.cli_config.thinking;
    let mean_input_tokens = app_config.cli_config.mean_input_tokens;
    let stddev_input_tokens = app_config.cli_config.stddev_input_tokens;
    let tokenizer = app_config.tokenizer.clone();

    let config = PromptConfig {
        prompt_encoding,
        sonnet_lines,
        mean_input_tokens,
        stddev_input_tokens,
        tokenizer,
    };

    (0..app_config.cli_config.max_num_completed_requests).map(move |_| {
        // Generate initial prompt for this session
        let (initial_prompt, input_tokens) = config.generate_turn_prompt();
        let output_tokens = sample_random_positive_int(
            app_config.cli_config.mean_output_tokens,
            app_config.cli_config.stddev_output_tokens,
            1,
        );

        let session = MultiTurnSession::new(
            num_turns,
            initial_prompt,
            model.clone(),
            max_tokens,
            thinking,
        );

        SessionInput {
            session,
            output_tokens,
            input_tokens,
            config: config.clone(),
        }
    })
}

/// Run a complete session: loop through all turns, collecting metrics.
async fn run_session(
    input: SessionInput,
    api_timeout: Duration,
    api_base: String,
    api_key: Option<String>,
    pb: &ProgressBar,
    sender: Option<mpsc::UnboundedSender<Metrics>>,
) -> Vec<Metrics> {
    let mut session = input.session;
    let output_tokens = input.output_tokens;
    let input_tokens = input.input_tokens;
    let config = input.config;
    let mut metrics_list = Vec::new();

    while !session.is_complete() {
        let turn_index = session.turn_index;
        let chat_req = session.build_request();
        let request = Request::new(api_base.clone(), api_key.clone(), chat_req);

        // Build metrics template for this turn
        let mut metrics = build_metrics(input_tokens, output_tokens, turn_index);

        // Send request and collect response
        if let Err(e) = api::chat_completions(request, &mut metrics, &api_timeout).await {
            metrics.error_msg = Some(e.to_string());
            metrics_list.push(metrics.clone());
            // Send metrics immediately on error
            if let Some(ref tx) = sender {
                tx.send(metrics).ok();
            }
            break;
        }

        // Update metrics with actual token counts
        metrics.number_total_tokens = metrics.number_input_tokens + metrics.number_output_tokens;

        // Extract response content for the next turn, even if reasoning is present, don't clone it
        // Clone here cause we need the content for the metrics writing
        let response_content = metrics.content.clone().unwrap_or_default();

        // Send metrics immediately on successful turn
        if let Some(ref tx) = sender {
            tx.send(metrics.clone()).ok();
        }

        metrics_list.push(metrics);

        session.store_response_and_advance(response_content, &config);
        pb.inc(1);
    }

    metrics_list
}

/// Build the session stream for multi-turn benchmarking.
fn create_session_tasks(
    inputs: impl Iterator<Item = SessionInput>,
    api_timeout: Duration,
    num_concurrent_requests: usize,
    api_base: String,
    api_key: Option<String>,
    progress_bar: &ProgressBar,
    sender: Option<mpsc::UnboundedSender<Metrics>>,
) -> impl Stream<Item = Vec<Metrics>> {
    stream::iter(inputs)
        .map(move |input| {
            let api_base = api_base.clone();
            let api_key = api_key.clone();
            let sender = sender.clone();

            async move {
                run_session(input, api_timeout, api_base, api_key, progress_bar, sender).await
            }
        })
        .buffer_unordered(num_concurrent_requests)
}

/// Container for the task stream and its associated channel endpoints.
///
/// The sender is used by `run_session()` to send metrics incrementally.
/// The receiver is passed to the file saver for persisting metrics.
pub struct TaskStream<S> {
    /// The stream of metrics (one Vec per session, containing all turns).
    pub stream: S,
    /// Sender for metrics - metrics are sent immediately after each turn completes.
    pub sender: mpsc::UnboundedSender<Metrics>,
    /// Receiver for metrics - passed to file saver for persistence.
    pub receiver: mpsc::UnboundedReceiver<Metrics>,
}

pub fn create_task_stream(
    config: &AppConfig,
) -> Result<TaskStream<impl Stream<Item = Vec<Metrics>>>> {
    let sonnet_lines = parse_sonnet_text(&config.tokenizer, SONNET_TEXT)?;
    let inputs = create_session_inputs(sonnet_lines, config);

    // Create channel for incremental metric sending
    let (sender, receiver) = mpsc::unbounded_channel();

    let stream = create_session_tasks(
        inputs,
        config.api_timeout,
        config.cli_config.num_concurrent_requests,
        config.api_base.clone(),
        config.api_key.clone(),
        &config.progress_bar,
        Some(sender.clone()),
    );

    Ok(TaskStream {
        stream,
        sender,
        receiver,
    })
}
