use futures::StreamExt;
use log::{debug, info, warn};
use reqwest::Client;
use std::sync::Once;
use tokio::time::{Duration, Instant};

static WARN_ONCE: Once = Once::new();

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
/// User-agent sent with every outbound request. Telemetry only.
const USER_AGENT: &str = concat!("llmperf-rs/", env!("CARGO_PKG_VERSION"));

/// Build the shared HTTP client used for all requests.
///
/// `reqwest::Client` owns the connection pool; constructing one and cloning it
/// into each task (it's a cheap `Arc` clone) lets concurrent requests reuse
/// keep-alive connections instead of each spinning up its own pool.
/// Per-request timeouts are set on the individual `RequestBuilder`.
pub fn build_shared_client() -> anyhow::Result<reqwest::Client> {
    Ok(Client::builder().user_agent(USER_AGENT).build()?)
}

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

/// Accumulated state during SSE streaming. Cleared when streaming ends.
struct StreamState {
    final_str: String,
    reasoning_str: String,
    prefill_start: Instant,
    ttft: Option<Duration>,
    prev_token: Option<Instant>,
    itl: Vec<Duration>,
    usage_seen: bool,
}

impl StreamState {
    fn new(max_tokens: u32) -> Self {
        Self {
            final_str: String::with_capacity(max_tokens as usize * AVG_BYTES_PER_TOKEN),
            reasoning_str: String::new(),
            prefill_start: Instant::now(),
            ttft: None,
            prev_token: None,
            itl: Vec::with_capacity(max_tokens as usize),
            usage_seen: false,
        }
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

/// Parse one SSE event payload into state and metrics.
fn handle_response(data: &str, state: &mut StreamState, metrics: &mut Metrics) {
    let response = match serde_json::from_str::<StreamResponse>(data) {
        Ok(r) => r,
        Err(e) => {
            debug!("Failed to deserialize StreamResponse: {e} (data: {data})");
            return;
        }
    };
    if let Some(choice) = response.choices.first() {
        if let Some(reasoning) = choice
            .delta
            .reasoning
            .as_deref()
            .or(choice.delta.reasoning_content.as_deref())
        {
            state.reasoning_str.push_str(reasoning);
            process_token_delta(
                Some(reasoning),
                &mut state.ttft,
                &mut state.prev_token,
                &mut state.itl,
                state.prefill_start,
            );
        }
        if let Some(content) = &choice.delta.content {
            state.final_str.push_str(content);
            process_token_delta(
                Some(content.as_str()),
                &mut state.ttft,
                &mut state.prev_token,
                &mut state.itl,
                state.prefill_start,
            );
        }
        let reason = choice.finish_reason.clone().or(choice.stop_reason.clone());
        if reason.is_some() {
            debug!("Got finish_reason={reason:?}, data={data}");
            metrics.finish_reason = reason;
        }
    }
    if let Some(usage) = response.usage {
        metrics.number_input_tokens = usage.prompt_tokens;
        metrics.number_output_tokens = usage.completion_tokens;
        metrics.number_total_tokens = usage.total_tokens;
        metrics.cached_tokens = usage
            .prompt_tokens_details
            .as_ref()
            .and_then(|d| d.cached_tokens.or(d.cached_read_tokens));
        state.usage_seen = true;
    }
}

async fn handle_error_response(
    response: reqwest::Response,
    metrics: &mut Metrics,
) -> anyhow::Error {
    let status = response.status();
    metrics.error_code = Some(status.as_u16());
    let body_str = response.text().await.unwrap_or_default();
    let msg = extract_error_message(&body_str);
    let error_msg = format!("HTTP {status} - {msg}");
    info!("API error: {error_msg}");
    metrics.error_msg = Some(error_msg.clone());
    anyhow::anyhow!(error_msg)
}

async fn send_streaming_request(
    client: &reqwest::Client,
    request: &Request,
    api_timeout: &Duration,
) -> Result<reqwest::Response, anyhow::Error> {
    let body_json = serde_json::to_string(&request.chat_completion)?;
    debug!(
        "Sending request to {}: model={} messages={} max_tokens={} stream={}",
        request.url,
        request.chat_completion.model,
        request.chat_completion.messages.len(),
        request.chat_completion.max_tokens,
        request.chat_completion.stream,
    );

    let mut req = client
        .post(&request.url)
        .timeout(*api_timeout)
        .header("Content-Type", "application/json")
        .body(body_json);

    for (name, value) in &request.headers {
        req = req.header(name, value);
    }

    if let Some(key) = &request.api_key {
        req = req.header("Authorization", format!("Bearer {key}"));
    }

    Ok(req.send().await?)
}

pub async fn chat_completions(
    client: &reqwest::Client,
    request: Request,
    metrics: &mut Metrics,
    api_timeout: &Duration,
) -> Result<(), anyhow::Error> {
    if request.chat_completion.stream {
        let response = send_streaming_request(client, &request, api_timeout).await?;

        if !response.status().is_success() {
            return Err(handle_error_response(response, metrics).await);
        }

        let mut state = StreamState::new(request.chat_completion.max_tokens);

        let stream = sse_stream(response);
        tokio::pin!(stream);

        while let Some(event_result) = stream.next().await {
            match event_result? {
                Sse::Comment(_) => {}
                Sse::Done => break,
                Sse::Event(data) => {
                    debug!("SSE data: {data}");
                    handle_response(&data, &mut state, metrics);
                }
            }
        }

        if !state.usage_seen {
            WARN_ONCE.call_once(|| {
                warn!("Usage stats not provided by endpoint, using input stats. Results may vary");
            });
        }
        let e2e_time = state.prefill_start.elapsed();

        metrics.prefill_throughput_tps =
            calculate_prefill_tps(state.ttft.as_ref(), metrics.number_input_tokens);
        // use the total tokens instead, due to those endpoints with chunked text
        metrics.decode_throughput_tps = calculate_decode_tps(metrics, &e2e_time);
        metrics.end_to_end_latency_s = e2e_time.as_secs_f64();

        populate_metrics(
            metrics,
            state.ttft,
            &state.itl,
            state.final_str,
            state.reasoning_str,
        );
    }
    Ok(())
}

/// Check API endpoint connectivity by making a GET request to /models endpoint
/// Checks if the model is in the list of available models
pub async fn check_endpoint(
    client: &reqwest::Client,
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

    info!("Checking endpoint connectivity: {models_endpoint} (API key: {masked_key})");

    let mut request = client.get(&models_endpoint);

    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {key}"));
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
                "Endpoint check failed with status {status}: {error_text}"
            ))
        }
    }
}
