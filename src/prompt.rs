use anyhow::{Context, Result};
use futures::Stream;
use futures::stream::{self, StreamExt};
use log::warn;
use rand;
use rand::seq::SliceRandom;
use rand_distr::{Distribution, Normal};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Once;

use crate::api;
use crate::config::AppConfig;
use crate::metrics::{self, Metrics};

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
    expect_output_tokens: u32,
    tokenizer: &tokenizers::Tokenizer,
    sonnet_lines: &[(tokenizers::Encoding, u32)],
) -> (String, u32) {
    let prompt_text = format!(
        "Repeat lines indefinitely from the following text with {expect_output_tokens} output tokens. Don't generate eos tokens:\n\n"
    );

    let prompt_encoding = tokenizer.encode_fast(prompt_text.as_str(), false).unwrap();
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

    let (prompt, actual_token_count) =
        create_prompt(prompt_ids, sonnet_lines, tokenizer, remaining_prompt_tokens);

    (prompt, actual_token_count)
}
pub fn create_prompt(
    mut prompt_ids: Vec<u32>,
    sonnet_lines: &[(tokenizers::Encoding, u32)],
    tokenizer: &tokenizers::Tokenizer,
    remaining_prompt_tokens: u32,
) -> (String, u32) {
    // Create a mutable copy to shuffle for each call to maintain randomness
    let mut shuffled_indices: Vec<usize> = (0..sonnet_lines.len()).collect();
    let mut rng = rand::rng();
    shuffled_indices.shuffle(&mut rng);

    let mut result_ids = Vec::new();
    let mut remaining = remaining_prompt_tokens;

    // Use cycle to replicate the while loop from the original python code
    for &line_idx in shuffled_indices.iter().cycle() {
        if remaining == 0 {
            break;
        }

        let (encoding, line_len) = &sonnet_lines[line_idx];
        let line_ids = encoding.get_ids();

        if line_len <= &remaining {
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
    prompt_ids.extend(&result_ids);

    // Decode the final result to get the prompt text
    let prompt = tokenizer.decode(&prompt_ids, false).unwrap();

    (prompt, prompt_ids.len() as u32)
}

pub fn read_sonnet_file(
    tokenizer: &tokenizers::Tokenizer,
    sonnet_path: &str,
) -> Result<Vec<(tokenizers::Encoding, u32)>> {
    let file = File::open(sonnet_path).context("Failed to open sonnet file")?;
    let reader = BufReader::new(file);

    let lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .map(|line| line + "\n")
        .collect();

    // We need to pass references to encode_batch so we don't consume the lines
    let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();

    let encodings = tokenizer
        .encode_batch_fast(line_refs, false)
        .map_err(|e| anyhow::anyhow!("Failed to encode batch: {}", e))?;

    let lines_with_encodings: Vec<(tokenizers::Encoding, u32)> = encodings
        .into_iter()
        .map(|e| {
            let len = e.len() as u32;
            (e, len)
        })
        .collect();

    Ok(lines_with_encodings)
}
#[allow(dead_code)]
fn get_token_length(tokenizer: &tokenizers::Tokenizer, text: &str) -> u32 {
    tokenizer
        .encode_fast(text, true)
        .expect("Failed to get token length")
        .len() as u32
}

pub fn create_inputs(
    sonnet_lines: &[(tokenizers::Encoding, u32)],
    app_config: &AppConfig,
) -> Vec<(String, u32, u32)> {
    (0..app_config.cli_config.max_num_completed_requests)
        .map(|_| {
            let output_tokens = sample_random_positive_int(
                app_config.cli_config.mean_output_tokens,
                app_config.cli_config.stddev_output_tokens,
                1,
            );
            let (prompt, prompt_tokens) = randomly_sample_sonnet_lines_prompt(
                app_config.cli_config.mean_input_tokens,
                app_config.cli_config.stddev_input_tokens,
                output_tokens,
                &app_config.tokenizer,
                sonnet_lines,
            );
            (prompt, output_tokens, prompt_tokens)
        })
        .collect()
}
// Builds the stream of tasks
pub fn create_tasks(
    inputs: Vec<(String, u32, u32)>,
    app_config: &AppConfig,
) -> impl Stream<Item = (Metrics, Result<(), anyhow::Error>)> + use<> {
    let api_base = app_config.api_base.clone();
    let api_key = app_config.api_key.clone();
    let model = app_config.model.clone();
    let api_timeout = app_config.api_timeout;
    let num_concurrent_requests = app_config.cli_config.num_concurrent_requests;

    stream::iter(inputs)
        .map(move |(prompt, output_tokens, prompt_tokens)| {
            let url = api_base.clone();
            let api_key = api_key.clone();
            let model = model.clone();
            async move {
                // Perform the HTTP GET request
                let completion = api::models::ChatCompletionRequest::from_prompt(
                    model,
                    prompt,
                    output_tokens,
                    true,
                );
                let post_request = api::models::Request::new(&url, api_key, completion);
                let mut metrics = metrics::Metrics::default();
                // Populate the variables we already know
                metrics.number_input_tokens = prompt_tokens;
                metrics.number_output_tokens = output_tokens;
                metrics.number_total_tokens =
                    metrics.number_input_tokens + metrics.number_output_tokens;
                let result = api::chat_completions(post_request, &mut metrics, &api_timeout)
                    .await
                    .map_err(|e| anyhow::anyhow!("API error: {}", e));

                (metrics, result)
            }
        })
        .buffered(num_concurrent_requests)
}
