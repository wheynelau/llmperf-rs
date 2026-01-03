use serde_json::json;
use std::time::Duration;
use token_benchmark::api::chat;
use token_benchmark::api::models::{ChatCompletionRequest, FinishReason, Request};
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
    let chat_completion = ChatCompletionRequest::from_prompt("test-model", "Say hello", 10, true);

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let result = chat::chat_completions(request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {:?}", e);
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {:?}",
        result
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
    assert_eq!(
        metrics.decode_throughput_tps, 0.0,
        "Decode throughput should be 0 due to single token"
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
    let chat_completion = ChatCompletionRequest::from_prompt("test-model", "Say hello", 10, true);

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let result = chat::chat_completions(request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {:?}", e);
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {:?}",
        result
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
    let chat_completion = ChatCompletionRequest::from_prompt("test-model", "Say hello", 10, true);

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let result = chat::chat_completions(request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {:?}", e);
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {:?}",
        result
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

    let chat_completion = ChatCompletionRequest::from_prompt("test-model", "Test", 10, true);

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
    };

    let mut metrics = Metrics::default();
    let result = chat::chat_completions(request, &mut metrics, &Duration::from_secs(10)).await;

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

    let result = chat::check_endpoint(
        &format!("{}/", mock_server.uri()),
        Some("test-key".to_string()),
    )
    .await;

    assert!(result.is_ok(), "Endpoint check should succeed");
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

    let result = chat::check_endpoint(
        &format!("{}/", mock_server.uri()),
        Some("invalid-key".to_string()),
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
    let chat_completion = ChatCompletionRequest::from_prompt("test-model", "Say hello", 10, true);

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
    };
    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let result = chat::chat_completions(request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {:?}", e);
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {:?}",
        result
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
    let chat_completion = ChatCompletionRequest::from_prompt(
        "test-model",
        "Explain the theory of relativity",
        1500,
        true,
    );

    let request = Request {
        url: format!("{}/v1/chat/completions", mock_server.uri()),
        api_key: Some("test-key".to_string()),
        chat_completion,
    };

    let mut metrics = Metrics {
        number_input_tokens: 999,
        number_output_tokens: 999,
        ..Default::default()
    };

    let result = chat::chat_completions(request, &mut metrics, &Duration::from_secs(10)).await;

    if let Err(ref e) = result {
        println!("Error details: {:?}", e);
        println!("Error type: {}", std::any::type_name_of_val(&e));
    }

    assert!(
        result.is_ok(),
        "chat_completions should succeed: {:?}",
        result
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
    assert_eq!(metrics.content, " of physics are speed", "Content mismatch");
    assert_eq!(metrics.reasoning, "Okay, the point", "Reasoning mismatch");
}
