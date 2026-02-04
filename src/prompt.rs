use anyhow::Result;
use futures::Stream;
use futures::stream::{self, StreamExt};
use log::warn;
use rand;
use rand::seq::SliceRandom;
use rand_distr::{Distribution, Normal};
use std::sync::Once;
use tokio::time::Duration;

use crate::api;
use crate::api::models::Request;
use crate::config::AppConfig;
use crate::metrics::{self, Metrics};

pub const SONNET_TEXT: &str = include_str!("../sonnet.txt");
pub const PROMPT_TEXT: &str =
    "Repeat lines indefinitely from the above text. Don't generate eos tokens:\n";

pub fn sample_random_positive_int(mean: u32, stddev: u32, prompt_token_length: u32) -> u32 {
    if stddev == 0 {
        return mean;
    }
    let mut rng = rand::rng();

    let normal = Normal::new(mean as f64, stddev as f64).unwrap();

    loop {
        let sample_f64 = normal.sample(&mut rng);
        let sample_u32 = sample_f64.round() as u32;

        if sample_u32 >= prompt_token_length {
            return sample_u32;
        }
    }
}

pub fn randomly_sample_sonnet_lines_prompt(
    prompt_tokens_mean: u32,
    prompt_tokens_stddev: u32,
    prompt_encoding: &tokenizers::Encoding,
    tokenizer: &tokenizers::Tokenizer,
    sonnet_lines: &[tokenizers::Encoding],
) -> (String, u32) {
    let prompt_ids = prompt_encoding.get_ids().to_vec();

    let prompt_token_len = prompt_ids.len() as u32;

    // Set a safe mean in the event a low mean was set with stddev, potentially creating an infinite loop
    // With the recent change to args.rs this may not be needed
    let safe_mean = std::cmp::max(prompt_tokens_mean, prompt_token_len);

    if prompt_tokens_mean < prompt_token_len {
        static WARN_ONCE: Once = Once::new();
        WARN_ONCE.call_once(|| {
            warn!(
                "prompt_tokens_mean ({}) is less than base prompt length ({}). \
                Adjusting mean to ({})\n\
                This warning will only show once.",
                prompt_tokens_mean, prompt_token_len, prompt_token_len
            );
        });
    }

    let num_prompt_tokens =
        sample_random_positive_int(safe_mean, prompt_tokens_stddev, prompt_token_len);

    let remaining_prompt_tokens = num_prompt_tokens - prompt_token_len;

    let result_ids = create_prompt(&prompt_ids, sonnet_lines, remaining_prompt_tokens);
    let prompt = tokenizer.decode(&result_ids, false).unwrap();
    let actual_token_count = result_ids.len() as u32;

    (prompt, actual_token_count)
}
pub fn create_prompt(
    prompt_ids: &[u32],
    sonnet_lines: &[tokenizers::Encoding],
    remaining_prompt_tokens: u32,
) -> Vec<u32> {
    // Create a mutable copy to shuffle for each call to maintain randomness
    let mut shuffled_indices: Vec<usize> = (0..sonnet_lines.len()).collect();
    let mut rng = rand::rng();
    shuffled_indices.shuffle(&mut rng);

    let mut result_ids: Vec<u32> = Vec::new();
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
            // Take the whole line
            result_ids.extend(line_ids);
            remaining -= line_len;
        } else {
            // Take partial line
            result_ids.extend(&line_ids[..remaining as usize]);
            break;
        }
    }

    // Combine prompt_ids and result_ids
    result_ids.extend(prompt_ids);

    result_ids
}

fn tokenize_sonnet_lines(
    tokenizer: &tokenizers::Tokenizer,
    sonnet_lines: &[String],
) -> Result<Vec<tokenizers::Encoding>> {
    // We need to pass references to encode_batch so we don't consume the lines
    let line_refs: Vec<&str> = sonnet_lines.iter().map(|s| s.as_str()).collect();

    let encodings = tokenizer
        .encode_batch_fast(line_refs, false)
        .map_err(|e| anyhow::anyhow!("Failed to encode batch: {}", e))?;

    Ok(encodings)
}

pub fn parse_sonnet_text(
    tokenizer: &tokenizers::Tokenizer,
    sonnet_text: &str,
) -> Result<Vec<tokenizers::Encoding>> {
    let lines: Vec<String> = sonnet_text
        .lines()
        .map(|line| line.to_string() + "\n")
        .collect();

    let lines_with_encodings = tokenize_sonnet_lines(tokenizer, &lines)?;

    Ok(lines_with_encodings)
}

fn build_metrics(prompt_tokens: u32, output_tokens: u32) -> Metrics {
    let mut metrics = metrics::Metrics::default();
    // Populate the variables we already know
    metrics.number_input_tokens = prompt_tokens;
    metrics.number_output_tokens = output_tokens;
    metrics.number_total_tokens = metrics.number_input_tokens + metrics.number_output_tokens;

    metrics
}

fn build_request(api_config: &AppConfig, prompt: String, output_tokens: u32) -> Request {
    let completion = api::models::ChatCompletionRequest::from_prompt(
        api_config.model.clone(),
        prompt,
        output_tokens,
        true,
        api_config.cli_config.thinking,
    );
    Request::new(
        api_config.api_base.clone(),
        api_config.api_key.clone(),
        completion,
    )
}

fn create_inputs(
    sonnet_lines: Vec<tokenizers::Encoding>,
    app_config: &AppConfig,
) -> impl Iterator<Item = (Metrics, Request)> {
    let prompt_encoding = app_config
        .tokenizer
        .encode_fast(PROMPT_TEXT, false)
        .unwrap();
    (0..app_config.cli_config.max_num_completed_requests).map(move |_| {
        let output_tokens = sample_random_positive_int(
            app_config.cli_config.mean_output_tokens,
            app_config.cli_config.stddev_output_tokens,
            1,
        );
        let (prompt, prompt_tokens) = randomly_sample_sonnet_lines_prompt(
            app_config.cli_config.mean_input_tokens,
            app_config.cli_config.stddev_input_tokens,
            &prompt_encoding,
            &app_config.tokenizer,
            &sonnet_lines,
        );
        let metrics = build_metrics(prompt_tokens, output_tokens);
        let request = build_request(app_config, prompt, output_tokens);
        (metrics, request)
    })
}
// Builds the stream of tasks
fn create_tasks(
    inputs: impl Iterator<Item = (Metrics, Request)>,
    api_timeout: Duration,
    num_concurrent_requests: usize,
) -> impl Stream<Item = (Metrics, Result<(), anyhow::Error>)> {
    stream::iter(inputs)
        .map(move |(mut metrics, post_request)| {
            async move {
                // Perform the HTTP GET request
                let result = api::chat_completions(post_request, &mut metrics, &api_timeout)
                    .await
                    .map_err(|e| anyhow::anyhow!("API error: {}", e));

                (metrics, result)
            }
        })
        .buffer_unordered(num_concurrent_requests)
}

pub fn create_task_stream(
    config: &AppConfig,
) -> Result<impl Stream<Item = (Metrics, Result<(), anyhow::Error>)>> {
    let sonnet_lines = parse_sonnet_text(&config.tokenizer, SONNET_TEXT)?;
    let inputs = create_inputs(sonnet_lines, config);

    Ok(create_tasks(
        inputs,
        config.api_timeout,
        config.cli_config.num_concurrent_requests,
    ))
}
