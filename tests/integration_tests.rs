use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use token_benchmark::api::chat;
use token_benchmark::api::models::{ChatCompletionRequest, FinishReason, Message, Request};
use token_benchmark::metrics::models::Metrics;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_chat_completions_1_token_no_usage() {
    let mock_server = MockServer::start().await;

    // Return a response that does not contain the usage
    // Validates the input and output tokens are not changed
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"stop","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    // Create test request
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {e:?}");
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {result:?}"
    );

    // Verify metrics were populated
    assert_eq!(
        metrics.number_output_tokens, 999,
        "Output tokens should not change"
    );
    assert_eq!(
        metrics.number_input_tokens, 999,
        "Input tokens should not change"
    );
    // No usage → output keeps the pre-seeded budget; decode = budget / e2e > 0.
    assert!(
        metrics.decode_throughput_tps > 0.0,
        "Decode throughput should be budget-based when usage is absent"
    );
    assert!(
        metrics.prefill_throughput_tps > 0.0,
        "Should have prefill throughput"
    );
    assert!(metrics.ttft_s > 0.0, "Should have TTFT");
    assert!(
        metrics.itl_ms_vec.is_empty(),
        "Should not have ITL data because its only 1 token"
    );
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Stop,
        "Finish reason should be Stop"
    );
}

#[tokio::test]
async fn test_chat_completions_n_tokens_no_usage() {
    let mock_server = MockServer::start().await;

    // Return a response that does not contain the usage
    // Validates the input and output tokens are not changed
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"stop","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    // Create test request
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {e:?}");
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {result:?}"
    );

    // Verify metrics were populated
    assert_eq!(
        metrics.number_output_tokens, 999,
        "Output tokens should not change"
    );
    assert_eq!(
        metrics.number_input_tokens, 999,
        "Input tokens should not change"
    );
    // 2 tokens should have tps
    assert!(
        metrics.decode_throughput_tps > 0.0,
        "Should have decode throughput"
    );
    assert!(
        metrics.prefill_throughput_tps > 0.0,
        "Should have prefill throughput"
    );
    assert!(metrics.ttft_s > 0.0, "Should have TTFT");
    assert_eq!(metrics.itl_ms_vec.len(), 1, "ITL len should be n-1");
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Stop,
        "Finish reason should be Stop"
    );
}

#[tokio::test]
async fn test_chat_completions_n_tokens_with_usage() {
    let mock_server = MockServer::start().await;

    // Return a response that does not contain the usage
    // Validates the input and output tokens are not changed
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"stop","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"completion_tokens":10,"prompt_tokens":10,"total_tokens":20,"completion_tokens_details":{"reasoning_tokens":0}}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    // Create test request
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {e:?}");
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {result:?}"
    );

    // Verify metrics were populated
    assert_eq!(
        metrics.number_output_tokens, 10,
        "Output tokens should change"
    );
    assert_eq!(
        metrics.number_input_tokens, 10,
        "Input tokens should change"
    );
    // This is an issue, should single tokens have decode_throughput_tps?
    assert!(
        metrics.decode_throughput_tps > 0.0,
        "Should have decode throughput"
    );
    assert!(
        metrics.prefill_throughput_tps > 0.0,
        "Should have prefill throughput"
    );
    assert!(metrics.ttft_s > 0.0, "Should have TTFT");
    assert_eq!(metrics.itl_ms_vec.len(), 1, "ITL len should be n-1");
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Stop,
        "Finish reason should be Stop"
    );
}

#[tokio::test]
async fn test_chat_completions_http_error() {
    let mock_server = MockServer::start().await;

    // Mock error response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(json!({"error": "Rate limit exceeded"})),
        )
        .mount(&mock_server)
        .await;

    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Test"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics::default();
    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    assert!(
        result.is_err(),
        "chat_completions should fail with HTTP error"
    );

    // Verify error metrics
    assert!(metrics.error_code.is_some(), "Should have error code");
    assert_eq!(metrics.error_code.unwrap(), 429);
    assert!(metrics.error_msg.is_some(), "Should have error message");
}

#[tokio::test]
async fn test_check_endpoint_success() {
    let mock_server = MockServer::start().await;

    // Mock successful /models endpoint
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {"id": "gpt-3.5-turbo"},
                {"id": "gpt-4"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let result = chat::check_endpoint(
        &client,
        &format!("{}/", mock_server.uri()),
        "gpt-3.5-turbo",
        Some("test-key"),
    )
    .await;

    assert!(result.is_ok(), "Endpoint check should succeed");
}

#[tokio::test]
async fn test_check_endpoint_wrong_model() {
    let mock_server = MockServer::start().await;

    // Mock successful /models endpoint
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {"id": "gpt-3.5-turbo"},
                {"id": "gpt-4"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let result = chat::check_endpoint(
        &client,
        &format!("{}/", mock_server.uri()),
        "non-existent-model",
        Some("test-key"),
    )
    .await;

    assert!(
        result.is_err(),
        "Endpoint check should fail for wrong model"
    );
}

#[tokio::test]
async fn test_check_usage() {
    let mock_server = MockServer::start().await;

    // Mock failed /models endpoint
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error": "Unauthorized"})))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let result = chat::check_endpoint(
        &client,
        &format!("{}/", mock_server.uri()),
        "gpt-3.5-turbo",
        Some("invalid-key"),
    )
    .await;

    assert!(result.is_err(), "Endpoint check should fail");
}

#[tokio::test]
async fn test_chat_completions_n_tokens_with_usage_stop_reason_length() {
    let mock_server = MockServer::start().await;

    // Return a response that does not contain the usage
    // Validates the input and output tokens are not changed
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"length","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"completion_tokens":10,"prompt_tokens":10,"total_tokens":20,"completion_tokens_details":{"reasoning_tokens":0}}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    // Create test request
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };
    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {e:?}");
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {result:?}",
    );

    // Verify metrics were populated
    assert_eq!(
        metrics.number_output_tokens, 10,
        "Output tokens should change"
    );
    assert_eq!(
        metrics.number_input_tokens, 10,
        "Input tokens should change"
    );
    // This is an issue, should single tokens have decode_throughput_tps?
    assert!(
        metrics.decode_throughput_tps > 0.0,
        "Should have decode throughput"
    );
    assert!(
        metrics.prefill_throughput_tps > 0.0,
        "Should have prefill throughput"
    );
    assert!(metrics.ttft_s > 0.0, "Should have TTFT");
    assert_eq!(metrics.itl_ms_vec.len(), 1, "ITL len should be n-1");
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Length,
        "Finish reason should be Length"
    );
}
#[tokio::test]
async fn test_chat_completions_with_reasoning_tokens() {
    // some endpoints send their content as reasoning rather than normal content
    let mock_server = MockServer::start().await;

    // This is the vllm endpoint behavior
    // Alternate reasoning and reasoning_content, as mentioned in their docs for v0.11.0
    // vllm: reasoning used to be called reasoning_content. For now, reasoning_content will continue to work.
    // However, we encourage you to migrate to reasoning in case reasoning_content is removed in future.
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"reasoning":"Okay", "reasoning_content":"Okay"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"reasoning_content":",", "reasoning":","}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"reasoning_content":" the"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"reasoning":" point"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" of"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" physics"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" are"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" speed"},"finish_reason":"length"}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[],"usage":{"completion_tokens":10,"prompt_tokens":10,"total_tokens":31}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    // Create test request
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Explain the theory of relativity"),
            reasoning: None,
        }],
        1500,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {e:?}");
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {result:?}",
    );

    // Verify metrics were populated
    assert_eq!(
        metrics.number_output_tokens, 10,
        "Output tokens should change"
    );
    assert_eq!(
        metrics.number_input_tokens, 10,
        "Input tokens should change"
    );
    // This is an issue, should single tokens have decode_throughput_tps?
    assert!(
        metrics.decode_throughput_tps > 0.0,
        "Should have decode throughput"
    );
    assert!(
        metrics.prefill_throughput_tps > 0.0,
        "Should have prefill throughput"
    );
    assert!(metrics.ttft_s > 0.0, "Should have TTFT");
    assert_eq!(metrics.itl_ms_vec.len(), 7, "ITL len should be n-1");
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Length,
        "Finish reason should be Length"
    );
    assert_eq!(
        metrics.content,
        Some(Arc::from(" of physics are speed")),
        "Content mismatch"
    );
    assert_eq!(
        metrics.reasoning,
        Some(Arc::from("Okay, the point")),
        "Reasoning mismatch"
    );
}

#[tokio::test]
async fn test_chat_completions_both_finish_reason_and_stop_reason() {
    let mock_server = MockServer::start().await;

    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"length","stop_reason":null,"index":0,"delta":{"content":"!"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"completion_tokens":10,"prompt_tokens":10,"total_tokens":20,"completion_tokens_details":{"reasoning_tokens":0}}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    assert!(
        result.is_ok(),
        "chat_completions should succeed with both finish_reason and stop_reason"
    );

    assert_eq!(
        metrics.number_output_tokens, 10,
        "Output tokens should change"
    );
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Length,
        "Finish reason should be Length from finish_reason field"
    );
}

#[tokio::test]
async fn test_chat_completions_stop_reason_only() {
    let mock_server = MockServer::start().await;

    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"stop_reason":"end_turn","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"completion_tokens":10,"prompt_tokens":10,"total_tokens":20,"completion_tokens_details":{"reasoning_tokens":0}}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    assert!(
        result.is_ok(),
        "chat_completions should succeed with stop_reason only"
    );

    assert_eq!(
        metrics.number_output_tokens, 10,
        "Output tokens should change"
    );
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Length,
        "Finish reason should be Length from stop_reason (alias: end_turn)"
    );
}

#[tokio::test]
async fn test_chat_completions_unrecognized_finish_reason() {
    // Backends may send values like "rejected", "error", "cancel" etc.
    // These should land in `Other` instead of failing deserialization.
    let mock_server = MockServer::start().await;

    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"stop_reason":"rejected","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"completion_tokens":10,"prompt_tokens":10,"total_tokens":20,"completion_tokens_details":{"reasoning_tokens":0}}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;

    assert!(
        result.is_ok(),
        "chat_completions should succeed with unrecognized finish reason"
    );
    assert!(metrics.finish_reason.is_some(), "Should have finish reason");
    assert_eq!(
        metrics.finish_reason.unwrap(),
        FinishReason::Other,
        "Unrecognized stop_reason should map to Other"
    );
}

#[tokio::test]
async fn test_chat_completions_cached_tokens() {
    let mock_server = MockServer::start().await;
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"stop","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"prompt_tokens":580,"prompt_tokens_details":{"cached_read_tokens":384,"cached_tokens":384},"completion_tokens":152,"completion_tokens_details":{},"total_tokens":732}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );
    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;
    assert!(
        result.is_ok(),
        "Chat completions should complete without error"
    );
    assert_eq!(metrics.cached_tokens, Some(384));
    assert_eq!(metrics.number_input_tokens, 580);
}

#[tokio::test]
async fn test_chat_completions_cached_tokens_none_when_prompt_tokens_details_absent() {
    let mock_server = MockServer::start().await;
    // GLM-style response: usage emitted, but no `prompt_tokens_details` key
    // at all (the field is absent, not present-with-null). cached_tokens has
    // nowhere to come from, so it stays None.
    let stream_response = concat!(
        r#"data: {"choices":[{"index":0,"delta":{"content":" Hello"}}]"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{"content":" World"}}]"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"finish_reason":"stop","index":0,"delta":{}}]"#,
        "\r\n\r\n",
        r#"data: {"choices":[{"index":0,"delta":{}}],"usage":{"prompt_tokens":196,"completion_tokens":363,"completion_tokens_details":{},"total_tokens":559}}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;
    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("Say hello"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );
    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers: HashMap::new(),
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;
    assert!(
        result.is_ok(),
        "Chat completions should complete without error"
    );
    assert_eq!(
        metrics.cached_tokens, None,
        "cached_tokens must stay None when the response omits prompt_tokens_details"
    );
    assert_eq!(metrics.number_input_tokens, 196);
    assert_eq!(metrics.number_output_tokens, 363);
}

#[tokio::test]
async fn test_chat_completions_user_headers_reach_server() {
    let mock_server = MockServer::start().await;

    let stream_response = concat!(
        r#"data: {"choices":[{"finish_reason":"stop","index":0,"delta":{}}]}"#,
        "\r\n\r\n",
        r#"data: [DONE]"#,
        "\r\n\r\n",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .and(header("authorization", "Bearer test-key"))
        .and(header("x-trace", "abc-123"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream_response, "text/event-stream"))
        .mount(&mock_server)
        .await;

    let chat_completion = ChatCompletionRequest::from_messages(
        "test-model",
        vec![Message {
            role: "user".to_string(),
            content: Arc::from("hi"),
            reasoning: None,
        }],
        10,
        true,
        false,
    );

    let mut headers = HashMap::new();
    headers.insert("x-trace".to_string(), "abc-123".to_string());

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
        headers,
    };

    let mut metrics = Metrics::default();
    let client = reqwest::Client::new();
    let result =
        chat::chat_completions(&client, request, &mut metrics, &Duration::from_secs(10)).await;
    assert!(
        result.is_ok(),
        "request with user-supplied header should succeed: {result:?}"
    );
}
