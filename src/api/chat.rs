use futures::StreamExt;
use log::{debug, info, warn};
use reqwest::Client;
use std::{error::Error, sync::Once};
use tokio::time::{Duration, Instant};

use super::models::{ModelList, Request, StreamResponse};
use super::sse::{Sse, sse_stream};
use crate::metrics::{
    models::Metrics,
    utils::{calculate_decode_tps, calculate_prefill_tps, populate_metrics},
};

/// Estimated average bytes per token for string capacity pre-allocation
const AVG_BYTES_PER_TOKEN: usize = 5;
/// Length to truncate when printing and showing to user
const TRUNCATED_LIMIT: usize = 200;

/// Extract error message from response body, handling OpenAI-style error format
fn extract_error_message(body_str: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str)
        && let Some(msg) = json["error"]["message"].as_str()
    {
        return msg.to_string();
    }
    if body_str.len() > TRUNCATED_LIMIT {
        format!("{}...", &body_str[..TRUNCATED_LIMIT])
    } else {
        body_str.to_string()
    }
}

/// Process a token delta (content or reasoning) and update timing metrics
fn process_token_delta(
    content: Option<&str>,
    ttft: &mut Option<Duration>,
    prev_token: &mut Option<Instant>,
    itl: &mut Vec<Duration>,
    prefill_start: Instant,
) {
    if content.is_some() {
        if ttft.is_none() {
            *ttft = Some(prefill_start.elapsed());
        } else if let Some(prev_time) = *prev_token {
            itl.push(prev_time.elapsed());
        }
        *prev_token = Some(Instant::now());
    }
}

pub async fn chat_completions(
    request: Request,
    metrics: &mut Metrics,
    api_timeout: &Duration,
) -> Result<(), Box<dyn Error>> {
    let mut final_str =
        String::with_capacity(request.chat_completion.max_tokens as usize * AVG_BYTES_PER_TOKEN);
    let mut reasoning_str = String::new();

    if request.chat_completion.stream {
        let body_json = serde_json::to_string(&request.chat_completion)?;
        debug!("Sending request to {}: {}", request.url, body_json);

        // add user-agent just for telemetry purposes
        let client = Client::builder()
            .timeout(*api_timeout)
            .user_agent(concat!("llmperf-rs/", env!("CARGO_PKG_VERSION")))
            .build()?;

        let mut req = client
            .post(&request.url)
            .header("Content-Type", "application/json")
            .body(body_json);

        if let Some(key) = &request.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            metrics.error_code = Some(status.as_u16());
            let body_str = response.text().await.unwrap_or_default();
            let msg = extract_error_message(&body_str);
            let error_msg = format!("HTTP {} - {}", status, msg);
            info!("API error: {}", error_msg);
            metrics.error_msg = Some(error_msg.clone());
            return Err(error_msg.into());
        }

        static WARN_ONCE: Once = Once::new();
        let mut should_warn_usage_missing = true;

        let prefill_start = Instant::now();
        let mut ttft: Option<Duration> = None;
        let mut prev_token: Option<Instant> = None;
        let mut itl: Vec<Duration> =
            Vec::with_capacity(request.chat_completion.max_tokens as usize);

        let stream = sse_stream(response);
        tokio::pin!(stream);

        while let Some(event_result) = stream.next().await {
            match event_result? {
                Sse::Comment(_) => {}
                Sse::Done => break,
                Sse::Event(data) => {
                    if let Ok(response) = serde_json::from_str::<StreamResponse>(&data) {
                        if let Some(choice) = response.choices.first() {
                            if let Some(reasoning) = choice
                                .delta
                                .reasoning
                                .as_deref()
                                .or(choice.delta.reasoning_content.as_deref())
                            {
                                reasoning_str.push_str(reasoning);
                                process_token_delta(
                                    Some(reasoning),
                                    &mut ttft,
                                    &mut prev_token,
                                    &mut itl,
                                    prefill_start,
                                );
                            }
                            if let Some(content) = &choice.delta.content {
                                final_str.push_str(content);
                                process_token_delta(
                                    Some(content.as_str()),
                                    &mut ttft,
                                    &mut prev_token,
                                    &mut itl,
                                    prefill_start,
                                );
                            }
                            if choice.finish_reason.is_some() {
                                metrics.finish_reason = choice.finish_reason.clone();
                            }
                        }
                        if let Some(usage) = response.usage {
                            metrics.number_input_tokens = usage.prompt_tokens;
                            metrics.number_output_tokens = usage.completion_tokens;
                            metrics.number_total_tokens = usage.total_tokens;
                            should_warn_usage_missing = false;
                        }
                    }
                }
            }
        }

        if should_warn_usage_missing {
            WARN_ONCE.call_once(|| {
                warn!("Usage stats not provided by endpoint, using input stats. Results may vary");
            });
        }
        let e2e_time = prefill_start.elapsed();

        metrics.prefill_throughput_tps =
            calculate_prefill_tps(ttft.as_ref(), metrics.number_input_tokens);
        metrics.decode_throughput_tps = calculate_decode_tps(&itl);
        metrics.end_to_end_latency_s = e2e_time.as_secs_f64();

        populate_metrics(metrics, ttft, itl, final_str, reasoning_str);
    }
    Ok(())
}

/// Check API endpoint connectivity by making a GET request to /models endpoint
/// Checks if the model is in the list of available models
pub async fn check_endpoint(
    url: &str,
    model: &str,
    api_key: Option<&str>,
) -> Result<String, anyhow::Error> {
    let models_endpoint = format!("{}/models", url.trim_end_matches('/'));

    let masked_key = match &api_key {
        Some(key) if key.len() > 8 => {
            format!("{}...{}", &key[..4], &key[key.len() - 4..])
        }
        Some(key) => "*".repeat(key.len()),
        None => "None".to_string(),
    };

    info!(
        "Checking endpoint connectivity: {} (API key: {})",
        models_endpoint, masked_key
    );

    let client = Client::builder()
        .user_agent(concat!("llmperf/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut request = client.get(&models_endpoint);

    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request.send().await?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let models_list: ModelList = response.json().await?;
            info!("Endpoint check successful - server responded with OK status");
            if !models_list.data.iter().any(|m| m.id == model) {
                return Err(anyhow::anyhow!(
                    "Model '{}' not found in available models: {:?}",
                    model,
                    models_list
                        .data
                        .iter()
                        .map(|m| &m.id)
                        .collect::<Vec<&String>>()
                ));
            }
            Ok("Endpoint check passed".to_string())
        }
        status => {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error response".to_string());
            Err(anyhow::anyhow!(
                "Endpoint check failed with status {}: {}",
                status,
                error_text
            ))
        }
    }
}
