use eventsource_client::{Client, SSE};
use futures::StreamExt;
use log::{info, warn};
use std::{error::Error, sync::Once};
use tokio::time::{Duration, Instant};

use super::models::{Request, StreamResponse};
use crate::metrics::{
    models::Metrics,
    utils::{calculate_decode_tps, calculate_prefill_tps, populate_metrics},
};

pub async fn chat_completions(
    request: Request,
    metrics: &mut Metrics,
    api_timeout: &Duration,
) -> Result<(), Box<dyn Error>> {
    // Tiny performance optimization
    // 5 is just an estimate
    let mut final_str = String::with_capacity((request.chat_completion.max_tokens * 5) as usize);

    if request.chat_completion.stream {
        let mut client = eventsource_client::ClientBuilder::for_url(&request.url)?
            .method("POST".to_string())
            .header("Content-Type", "application/json")?
            .connect_timeout(*api_timeout)
            .read_timeout(*api_timeout)
            .write_timeout(*api_timeout);
        if let Some(api_key) = request.api_key {
            client = client.header("Authorization", &format!("Bearer {}", api_key))?;
        }
        let body_json = serde_json::to_string(&request.chat_completion)?;
        client = client.body(body_json);
        let built_client = client.build();

        // Init the variables
        static WARN_ONCE: Once = Once::new();
        let mut should_warn_usage_missing = true;

        let mut stream = built_client.stream();

        let request_start = Instant::now();
        let mut prefill_start = Instant::now();
        let mut ttft: Option<Duration> = None;
        let mut prev_token: Option<Instant> = None;
        let mut itl: Vec<Duration> = Vec::new();

        // TODO: Tidy up or break into smaller functions

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => match event {
                    SSE::Comment(_) => {}
                    SSE::Event(evt) => {
                        // Check if this is the [DONE] message
                        if evt.data.trim() == "[DONE]" {
                            break;
                        }

                        if let Ok(response) = serde_json::from_str::<StreamResponse>(&evt.data) {
                            if let Some(choice) = response.choices.first() {
                                // Capture content if available
                                if let Some(content) = &choice.delta.content {
                                    final_str.push_str(content);
                                    // First token, set the TTFT
                                    if ttft.is_none() {
                                        ttft = Some(prefill_start.elapsed());
                                        info!("Time to first token (TTFT): {:?}", ttft.unwrap());
                                    } else if let Some(prev_time) = prev_token {
                                        // This branch handles the ITL
                                        itl.push(prev_time.elapsed());
                                    }
                                    // Set the previous token time
                                    prev_token = Some(Instant::now());
                                }
                                // Capture finish reason if available (typically in final chunk)
                                if choice.finish_reason.is_some() {
                                    metrics.finish_reason = choice.finish_reason.clone();
                                }
                            }
                            // Check if usage is provided, some endpoints will send their usage.
                            if let Some(usage) = response.usage {
                                metrics.number_input_tokens = usage.prompt_tokens;
                                metrics.number_output_tokens = usage.completion_tokens;
                                metrics.number_total_tokens = usage.total_tokens;
                                should_warn_usage_missing = false;
                            }
                        }
                    }
                    SSE::Connected(evt) => {
                        prefill_start = Instant::now();
                        info!("Connected: {:?}", evt);
                        info!("Time taken to connect: {:?}", request_start.elapsed());
                    }
                },
                Err(e) => {
                    // Lower the log level as the error will be reported in the metrics
                    info!("Stream error: {}", e);

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
                    // Return Ok to continue the loop
                    return Err(e.into());
                }
            }
        }
        if should_warn_usage_missing {
            WARN_ONCE.call_once(|| {
                warn!("Usage stats not provided by endpoint, using input stats. Results may vary");
            });
        };
        let e2e_time = request_start.elapsed();

        if let Some(t) = ttft {
            metrics.ttft_s = t.as_secs_f64();
        }

        metrics.prefill_throughput_tps =
            calculate_prefill_tps(&metrics.ttft_s, metrics.number_input_tokens);
        metrics.decode_throughput_tps = calculate_decode_tps(&itl);
        metrics.end_to_end_latency_s = e2e_time.as_secs_f64();

        populate_metrics(metrics, itl, final_str);
    }
    Ok(())
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
