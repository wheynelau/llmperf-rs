use llmperf::{api::models::FinishReason, args::Cli, metrics};
use statrs::statistics::{Data, Distribution};
use std::time::Duration;

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
    assert_eq!(summary.ttft_s.mean, 15.0);
    assert_eq!(summary.end_to_end_latency_s.mean, 25.0);
    assert_eq!(summary.itl_ms.stddev, stddev);
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
    assert_eq!(summary.ttft_s.mean, 10.0);
    assert_eq!(summary.end_to_end_latency_s.mean, 20.0);
    assert_eq!(summary.itl_ms.stddev, stddev);
    assert_eq!(summary.input_tokens.mean, 10.0);
    assert_eq!(summary.output_tokens.mean, 20.0);
}

/// Build a metric carrying only the fields the cache-hit-rate depends on:
/// `number_total_tokens` (denominator contribution for non-last turns) and
/// `cached_tokens` (numerator), at a given `turn_index`.
fn turn(total_tokens: u32, cached: Option<u32>, turn_index: usize) -> metrics::Metrics {
    metrics::Metrics {
        number_total_tokens: total_tokens,
        cached_tokens: cached,
        turn_index,
        ..Default::default()
    }
}

/// A `SummaryBuilder` with `multi_turn` configured. `args` MUST be set before
/// metrics are added: the denominator needs to know which turn is each
/// session's last turn.
fn builder(multi_turn: u32) -> metrics::SummaryBuilder {
    let mut b = metrics::SummaryBuilder::default();
    b.args(Cli {
        multi_turn,
        ..Default::default()
    });
    b
}

fn add(b: &mut metrics::SummaryBuilder, m: &metrics::Metrics) {
    b.add_metric(m);
}

/// `cache_hit_rate` = Σ cached / Σ total-of-non-last-turns.
/// Two 50% sessions: (50 + 50) / (100 + 100) = 0.5.
#[test]
fn test_cache_hit_rate_50_percent_across_sessions() {
    let mut b = builder(2);
    add(&mut b, &turn(100, Some(0), 0));
    add(&mut b, &turn(50, Some(50), 1));
    add(&mut b, &turn(100, Some(0), 0));
    add(&mut b, &turn(50, Some(50), 1));
    assert_eq!(b.build().cache_hit_rate, Some(0.5));
}

#[test]
fn test_cache_hit_rate_zero_when_nothing_cached() {
    let mut b = builder(2);
    add(&mut b, &turn(20, Some(0), 0));
    add(&mut b, &turn(10, Some(0), 1));
    assert_eq!(b.build().cache_hit_rate, Some(0.0));
}

/// An unobserved turn (`None`) skips the numerator but stays in the denominator.
#[test]
fn test_cache_hit_rate_skips_unobserved_numerator_keeps_denominator() {
    let mut b = builder(2);
    add(&mut b, &turn(20, Some(0), 0));
    add(&mut b, &turn(10, None, 1));
    assert_eq!(b.build().cache_hit_rate, Some(0.0));
}

#[test]
fn test_cache_hit_rate_with_unobserved_turn_keeps_partial_denominator() {
    // Three-turn session (turn 2 is last, excluded from the denominator).
    let mut b = builder(3);
    // turn 0: 5 cached of 20 total. turn 1: None (skipped from numerator,
    // but 10 stay in denominator). turn 2: last, excluded.
    add(&mut b, &turn(20, Some(5), 0));
    add(&mut b, &turn(10, None, 1));
    add(&mut b, &turn(5, Some(0), 2));
    // numerator = 5, denominator = 20 + 10 = 30 => 1/6.
    let rate = b.build().cache_hit_rate;
    let expected = Some(5.0 / 30.0);
    assert_eq!(
        rate, expected,
        "unobserved turn must dilute the observed rate, not null it out"
    );
}

/// All turns omitting `cached_tokens` leaves the rate `None`, not `Some(0.0)`.
#[test]
fn test_cache_hit_rate_none_when_all_turns_unobserved() {
    let mut b = builder(3);
    add(&mut b, &turn(20, None, 0));
    add(&mut b, &turn(10, None, 1));
    add(&mut b, &turn(5, None, 2));
    assert_eq!(
        b.build().cache_hit_rate,
        None,
        "all-None run must report `cache_hit_rate = None`, not Some(0.0)"
    );
}

/// Every turn reported `cached_tokens = Some(0)`: we observed zeros, so the
/// rate is `Some(0.0)`. Distinct from the all-None case.
#[test]
fn test_cache_hit_rate_zero_when_all_observations_are_zero() {
    let mut b = builder(3);
    add(&mut b, &turn(20, Some(0), 0));
    add(&mut b, &turn(10, Some(0), 1));
    add(&mut b, &turn(5, Some(0), 2));
    assert_eq!(b.build().cache_hit_rate, Some(0.0));
}

/// Perfect caching: turn 1 cached the entire prior conversation (700 = turn 0's
/// total), so the rate is exactly 1.0 — no clamping needed.
#[test]
fn test_cache_hit_rate_one_when_all_prior_history_cached() {
    let mut b = builder(2);
    add(&mut b, &turn(700, Some(0), 0));
    add(&mut b, &turn(50, Some(700), 1));
    assert_eq!(b.build().cache_hit_rate, Some(1.0));
}

/// Three-turn session: denominator = `total_0` + `total_1` (turn 2 is last); the
/// last turn's total is excluded from the denominator but its cached reads are
/// still counted in the numerator.
#[test]
fn test_cache_hit_rate_three_turn_session() {
    let mut b = builder(3);
    add(&mut b, &turn(100, Some(0), 0));
    add(&mut b, &turn(200, Some(60), 1));
    add(&mut b, &turn(300, Some(120), 2));
    // (0 + 60 + 120) / (100 + 200) = 180 / 300 = 0.6
    assert_eq!(b.build().cache_hit_rate, Some(0.6));
}

/// Single-turn mode (`multi_turn` = 1): every turn is the last turn, so there is
/// no cacheable history → rate is None.
#[test]
fn test_cache_hit_rate_none_in_single_turn_mode() {
    let mut b = builder(1);
    add(&mut b, &turn(100, Some(0), 0));
    assert_eq!(b.build().cache_hit_rate, None);
}

#[test]
fn test_cache_hit_rate_zero_when_warm_turn_reports_zero_reuse() {
    let mut b = builder(2);
    add(&mut b, &turn(1000, Some(0), 0));
    add(&mut b, &turn(50, Some(0), 1));
    assert_eq!(b.build().cache_hit_rate, Some(0.0));
}

/// Two complete sessions aggregate as Σ cached / Σ total-of-non-last-turns.
#[test]
fn test_cache_hit_rate_aggregates_across_sessions() {
    let mut b = builder(2);
    add(&mut b, &turn(800, Some(0), 0));
    add(&mut b, &turn(50, Some(600), 1));
    add(&mut b, &turn(400, Some(0), 0));
    add(&mut b, &turn(50, Some(200), 1));
    // (600 + 200) / (800 + 400) = 800 / 1200
    assert_eq!(b.build().cache_hit_rate, Some(800.0 / 1200.0));
}

/// `decode_throughput_tps` = `number_output_tokens` / `e2e_time`.
fn decode_metrics(output_tokens: u32) -> metrics::Metrics {
    metrics::Metrics {
        number_output_tokens: output_tokens,
        ..Default::default()
    }
}

#[test]
fn test_calculate_decode_tps_token_based() {
    // 10 output tokens over 1.0s e2e → 10.0 tps
    let m = decode_metrics(10);
    let tps = metrics::calculate_decode_tps(&m, &Duration::from_secs(1));
    assert!((tps - 10.0).abs() < 1e-12, "got {tps}");
}

/// Zero output tokens → 0.0 tps.
#[test]
fn test_calculate_decode_tps_zero_when_no_output_tokens() {
    let m = decode_metrics(0);
    assert_eq!(
        metrics::calculate_decode_tps(&m, &Duration::from_secs(1)),
        0.0
    );
}

/// Larger payload: 363 tokens over 2.177s (Nebius MiniMax-M3 latency) ≈ 166.7.
#[test]
fn test_calculate_decode_tps_matches_fixture_scale() {
    let m = decode_metrics(363);
    let tps = metrics::calculate_decode_tps(&m, &Duration::from_millis(2177));
    assert!((tps - (363.0 / 2.177)).abs() < 1e-9, "got {tps}");
}
