use statrs::statistics::{Data, Distribution};
use token_benchmark::{api::models::FinishReason, metrics};

#[test]
fn test_metrics_integration_wo_null() {
    let metrics_1 = metrics::Metrics {
        ttft_s: 10.0,
        end_to_end_latency_s: 20.0,
        itl_ms_vec: vec![1.0, 2.0, 3.0],
        number_input_tokens: 10,
        number_output_tokens: 20,
        prefill_throughput_tps: 30.0,
        decode_throughput_tps: 40.0,
        finish_reason: Some(FinishReason::Stop),
        ..Default::default()
    };
    let metrics_2 = metrics::Metrics {
        ttft_s: 20.0,
        end_to_end_latency_s: 30.0,
        itl_ms_vec: vec![4.0, 5.0, 6.0],
        number_input_tokens: 20,
        number_output_tokens: 30,
        prefill_throughput_tps: 50.0,
        decode_throughput_tps: 60.0,
        finish_reason: Some(FinishReason::Stop),
        ..Default::default()
    };
    let vec = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let data = Data::new(vec);
    let stddev = data.std_dev().unwrap();
    let mut builder = metrics::SummaryBuilder::default();
    builder.add_metric(&metrics_1);
    builder.add_metric(&metrics_2);

    let summary = builder.build();
    assert_eq!(summary.decode_throughput_tps.mean, 50.0);
    assert_eq!(summary.prefill_throughput_tps.mean, 40.0);
    assert_eq!(summary.ttft.mean, 15.0);
    assert_eq!(summary.end_to_end_latency.mean, 25.0);
    assert_eq!(summary.itl.stddev, stddev);
    assert_eq!(summary.input_tokens.mean, 15.0);
    assert_eq!(summary.output_tokens.mean, 25.0);
}

#[test]
fn test_metrics_integration_error() {
    let metrics_1 = metrics::Metrics {
        ttft_s: 10.0,
        end_to_end_latency_s: 20.0,
        itl_ms_vec: vec![1.0, 2.0, 3.0],
        number_input_tokens: 10,
        number_output_tokens: 20,
        prefill_throughput_tps: 30.0,
        decode_throughput_tps: 40.0,
        finish_reason: Some(FinishReason::Stop),
        ..Default::default()
    };
    let metrics_2 = metrics::Metrics {
        ttft_s: 20.0,
        end_to_end_latency_s: 30.0,
        itl_ms_vec: vec![4.0, 5.0, 6.0],
        number_input_tokens: 20,
        number_output_tokens: 30,
        prefill_throughput_tps: 50.0,
        decode_throughput_tps: 60.0,
        error_code: Some(1),
        error_msg: Some("error".to_string()),
        ..Default::default()
    };
    let vec = vec![1.0, 2.0, 3.0];
    let data = Data::new(vec);
    let stddev = data.std_dev().unwrap();
    let mut builder = metrics::SummaryBuilder::default();
    builder.add_metric(&metrics_1);
    builder.add_metric(&metrics_2);

    let summary = builder.build();
    // Validate the the errored metrics are not added
    assert_eq!(summary.decode_throughput_tps.mean, 40.0);
    assert_eq!(summary.prefill_throughput_tps.mean, 30.0);
    assert_eq!(summary.ttft.mean, 10.0);
    assert_eq!(summary.end_to_end_latency.mean, 20.0);
    assert_eq!(summary.itl.stddev, stddev);
    assert_eq!(summary.input_tokens.mean, 10.0);
    assert_eq!(summary.output_tokens.mean, 20.0);
}
