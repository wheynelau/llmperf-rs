use eventsource_client::{Client, SSE};
use futures::StreamExt;
use log::{error, info};
use std::{error::Error, time::Duration};

use crate::metrics::{Metrics, calculate_stats};
use crate::models::{Request, StreamResponse};

pub async fn chat_completions(
    request: Request,
    metrics: &mut Metrics,
) -> Result<(), Box<dyn Error>> {
    let mut final_str = String::new();
    if request.chat_completion.stream {
        let mut client = eventsource_client::ClientBuilder::for_url(&request.url)?
            .method("POST".to_string())
            .header("Content-Type", "application/json")?;
        if let Some(api_key) = request.api_key {
            client = client.header("Authorization", &format!("Bearer {}", api_key))?;
        }
        let body_json = serde_json::to_string(&request.chat_completion)?;
        client = client.body(body_json);
        let built_client = client.build();

        // Init the variables

        let prefill_start = std::time::Instant::now();
        let mut decode_start = std::time::Instant::now();
        let mut final_time = std::time::Instant::now();
        let mut ttft: Option<Duration> = None;
        let mut prev_token: Option<std::time::Instant> = None;
        let mut itl: Vec<Duration> = Vec::new();
        let mut stream = built_client.stream();
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => match event {
                    SSE::Comment(_) => {}
                    SSE::Event(evt) => {
                        // Check if this is the [DONE] message
                        if evt.data.trim() == "[DONE]" {
                            final_time = std::time::Instant::now();
                            break;
                        }

                        if let Ok(response) = serde_json::from_str::<StreamResponse>(&evt.data) {
                            if let Some(choice) = response.choices.first() {
                                // Capture content if available
                                if let Some(content) = &choice.delta.content {
                                    final_str.push_str(content);
                                    if ttft.is_none() {
                                        ttft = Some(prefill_start.elapsed());
                                        decode_start = std::time::Instant::now();
                                    } else if let Some(prev_time) = prev_token {
                                        itl.push(prev_time.elapsed());
                                    }
                                    prev_token = Some(std::time::Instant::now());
                                }
                                // Capture finish reason if available (typically in final chunk)
                                if choice.finish_reason.is_some() {
                                    metrics.finish_reason = choice.finish_reason.clone();
                                }
                            }
                            // Check if usage is provided, some endpoints will send their usage.
                            if let Some(usage) = response.usage {
                                // Should we replace the input?
                                // TODO: We could include a REAL INPUT TOKEN COUNT
                                metrics.number_output_tokens = usage.completion_tokens;
                                metrics.number_total_tokens = usage.total_tokens;
                            }
                        }
                    }
                    SSE::Connected(_) => {}
                },
                Err(e) => {
                    error!("Stream error: {}", e);

                    // Extract status code if it's an HTTP response error
                    match &e {
                        eventsource_client::Error::UnexpectedResponse(response, _) => {
                            metrics.error_code = Some(response.status());
                        }
                        _ => {
                            metrics.error_code = None;
                        }
                    }

                    // Record error in metrics
                    metrics.error_msg = Some(e.to_string());
                    metrics.number_errors += 1;
                    break;
                }
            }
        }

        metrics.prefill_throughput_tps =
            calculate_prefill_tps(prefill_start, decode_start, metrics.number_input_tokens);
        metrics.decode_throughput_tps =
            calculate_decode_tps(decode_start, final_time, metrics.number_output_tokens);

        populate_metrics(metrics, prefill_start, ttft, itl, final_str);
    }
    Ok(())
}

fn calculate_prefill_tps(
    prefill_start: std::time::Instant,
    decode_start: std::time::Instant,
    input_tokens: u32,
) -> f64 {
    let time = decode_start.duration_since(prefill_start);
    input_tokens as f64 / time.as_secs_f64()
}

fn calculate_decode_tps(
    decode_start: std::time::Instant,
    final_time: std::time::Instant,
    output_tokens: u32,
) -> f64 {
    let time = final_time.duration_since(decode_start);
    output_tokens as f64 / time.as_secs_f64()
}
fn populate_metrics(
    metrics: &mut Metrics,
    prefill_start: std::time::Instant,
    ttft: Option<std::time::Duration>,
    itl: Vec<std::time::Duration>,
    response: String,
) {
    // Populate metrics at the end of streaming
    let total_time = prefill_start.elapsed();
    metrics.end_to_end_latency_s = total_time.as_secs_f64();

    // Set TTFT if we got a first token
    if let Some(ttft_duration) = ttft {
        metrics.ttft_s = ttft_duration.as_secs_f64();
    }

    // Calculate ITL statistics
    if !itl.is_empty() {
        let itl_f64: Vec<f64> = itl.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
        let (mean, stddev) = calculate_stats(&itl_f64);
        metrics.itl_ms_mean = mean;
        metrics.itl_ms_stddev = stddev;
        metrics.itl_ms_vec = itl_f64;
    }
    metrics.response = response;
}

/// Check API endpoint connectivity by making a GET request to /models endpoint
pub async fn check_endpoint(url: &str, api_key: Option<String>) -> Result<String, anyhow::Error> {
    // Construct the models endpoint URL
    let models_endpoint = format!("{}/models", url.trim_end_matches('/'));

    // Mask API key for logging
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

    let client = reqwest::Client::new();
    let mut request = client.get(&models_endpoint);

    // Add Authorization header if API key is provided
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request.send().await?;

    // Check if the request was successful
    match response.status() {
        reqwest::StatusCode::OK => {
            info!("Endpoint check successful - server responded with OK status");
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
