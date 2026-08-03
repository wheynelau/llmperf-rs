use super::utils::{calculate_percentiles_f64, calculate_percentiles_ord};
use crate::api::models::FinishReason;
use crate::args::Cli;
use serde::{Deserialize, Serialize};
use serde_with::with_prefix;
use std::sync::Arc;

#[derive(Default, Serialize, Debug)]
pub struct DetailedStats<T> {
    pub quantiles_p25: f64,
    pub quantiles_p50: f64,
    pub quantiles_p75: f64,
    pub quantiles_p90: f64,
    pub quantiles_p95: f64,
    pub quantiles_p99: f64,
    pub mean: f64,
    pub median: f64,
    pub stddev: f64,
    pub min: T,
    pub max: T,
}

with_prefix!(itl "itl_ms_");
with_prefix!(ttft "ttft_s_");
with_prefix!(end_to_end_latency "end_to_end_latency_s_");
with_prefix!(prefill_throughput_tps "prefill_throughput_tps_");
with_prefix!(decode_throughput_tps "decode_throughput_tps_");
with_prefix!(input_tokens "input_tokens_");
with_prefix!(output_tokens "output_tokens_");

#[derive(Default, Serialize, Debug)]
pub struct SummaryMetrics {
    #[serde(flatten, with = "itl")]
    pub itl_ms: DetailedStats<f64>,
    #[serde(flatten, with = "ttft")]
    pub ttft_s: DetailedStats<f64>,
    #[serde(flatten, with = "end_to_end_latency")]
    pub end_to_end_latency_s: DetailedStats<f64>,
    #[serde(flatten, with = "prefill_throughput_tps")]
    pub prefill_throughput_tps: DetailedStats<f64>,
    #[serde(flatten, with = "decode_throughput_tps")]
    pub decode_throughput_tps: DetailedStats<f64>,
    #[serde(flatten, with = "input_tokens")]
    pub input_tokens: DetailedStats<u32>,
    #[serde(flatten, with = "output_tokens")]
    pub output_tokens: DetailedStats<u32>,
    pub cache_hit_rate: Option<f64>,
    pub error_code_frequency: std::collections::HashMap<u16, u32>,
    pub finish_reasons: std::collections::HashMap<FinishReason, u32>,
    pub number_errors: u32,
    pub num_requests_started: u32,
    pub num_completed_requests: u32,
    pub num_completed_requests_per_min: f64,
    pub error_rate: f64,
    pub timestamp: u64,
    pub args: Cli,
}
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Metrics {
    pub itl_ms_mean: f64,
    pub itl_ms_stddev: f64,
    pub itl_ms_vec: Vec<f64>,
    pub ttft_s: f64,
    pub end_to_end_latency_s: f64,
    pub number_input_tokens: u32,
    pub number_output_tokens: u32,
    pub number_total_tokens: u32,
    pub prefill_throughput_tps: f64,
    pub error_msg: Option<String>,
    pub error_code: Option<u16>,
    pub decode_throughput_tps: f64,
    pub content: Option<Arc<str>>,
    pub reasoning: Option<Arc<str>>,
    pub finish_reason: Option<FinishReason>,
    /// `None` when the endpoint does not report `prompt_tokens_details`.
    /// `Some(0)` means the endpoint explicitly reported 0 cached tokens.
    pub cached_tokens: Option<u32>,
    /// Turn index (0-based). Always 0 for single-turn mode.
    pub turn_index: usize,
    /// Sum of `number_input_tokens + number_output_tokens` across all
    /// *prior* turns of this session at the point this metric's request was
    /// built. Used as the cache-hit-rate denominator (Definition B):
    /// "fraction of cacheable prior history the API actually returned from
    /// cache." Always 0 on turn 0 (cold start; no prior history exists).
    pub cumulative_prior_tokens: u32,
}
// Due to complexity, might be better to have a builder
#[derive(Default)]
pub struct SummaryBuilder {
    itl_vec: Vec<f64>,
    ttft_vec: Vec<f64>,
    end_to_end_latency_vec: Vec<f64>,
    prefill_throughput_tps_vec: Vec<f64>,
    decode_throughput_tps_vec: Vec<f64>,
    input_tokens_vec: Vec<u32>,
    output_tokens_vec: Vec<u32>,
    /// Sum of observed `cached_tokens`. `None` = no turn reported one yet, so
    /// `cache_hit_rate` is `None` (vs `Some(0.0)` for observed zero hits).
    cached_tokens_sum: Option<u64>,
    cached_total_tokens_sum: u64,
    error_code_frequency: std::collections::HashMap<u16, u32>,
    finish_reasons: std::collections::HashMap<FinishReason, u32>,
    number_errors: u32,
    num_requests_started: u32,
    num_completed_requests: u32,
    total_time_seconds: f64,
    args: Cli,
}

impl SummaryBuilder {
    pub fn new() -> Self {
        SummaryBuilder::default()
    }

    fn add_ttft(&mut self, ttft: f64) {
        self.ttft_vec.push(ttft);
    }

    fn add_e2e_latency(&mut self, e2e_latency: f64) {
        self.end_to_end_latency_vec.push(e2e_latency);
    }

    fn add_prefill_throughput_tps(&mut self, prefill_throughput_tps: f64) {
        if prefill_throughput_tps == 0.0 {
            return;
        }
        self.prefill_throughput_tps_vec.push(prefill_throughput_tps);
    }

    fn add_decode_throughput_tps(&mut self, decode_throughput_tps: f64) {
        // Should we skip 0.0?
        if decode_throughput_tps == 0.0 {
            return;
        }
        self.decode_throughput_tps_vec.push(decode_throughput_tps);
    }

    fn add_input_tokens(&mut self, input_tokens: u32) {
        self.input_tokens_vec.push(input_tokens);
    }
    fn add_output_tokens(&mut self, output_tokens: u32) {
        self.output_tokens_vec.push(output_tokens);
    }
    fn add_total_tokens(&mut self, total_tokens: u32, turn_index: usize) {
        // Cache-hit-rate denominator: every turn except the last of a session,
        // since the last turn's content is never re-sent (so never cacheable).
        // This includes None turns, whose re-sent history is still cacheable.
        // `multi_turn` must be set (via `args`) before metrics are added.
        if self.args.multi_turn > 1 && turn_index as u32 != self.args.multi_turn - 1 {
            self.cached_total_tokens_sum += total_tokens as u64;
        }
    }

    /// Sum reported `cached_tokens` into the numerator. A `None` turn contributes
    /// nothing (its re-sent history still counts toward the denominator via
    /// `add_total_tokens`). Kept `Option` so an all-None run stays `None`
    /// instead of collapsing to `Some(0.0)`.
    fn add_cached_tokens(&mut self, cached_tokens: Option<u32>) {
        if let Some(cached) = cached_tokens {
            self.cached_tokens_sum = Some(self.cached_tokens_sum.unwrap_or(0) + cached as u64);
        }
    }

    fn add_error_code(&mut self, error_code: u16) {
        *self.error_code_frequency.entry(error_code).or_insert(0) += 1;
        self.number_errors += 1;
    }

    fn add_finish_reason(&mut self, finish_reason: Option<FinishReason>) {
        if let Some(finish_reason) = finish_reason {
            *self.finish_reasons.entry(finish_reason).or_insert(0) += 1;
        }
    }

    pub fn add_metric(&mut self, metric: &Metrics) -> &mut Self {
        if let Some(error_code) = metric.error_code {
            self.add_error_code(error_code);
        } else {
            // Only add these if there is no error
            // Previously the slice would always extend
            // But would there be a case where we have itl but errored?
            self.itl_vec.extend_from_slice(&metric.itl_ms_vec);
            self.add_ttft(metric.ttft_s);
            self.add_e2e_latency(metric.end_to_end_latency_s);
            self.add_prefill_throughput_tps(metric.prefill_throughput_tps);
            self.add_decode_throughput_tps(metric.decode_throughput_tps);
            self.add_input_tokens(metric.number_input_tokens);
            self.add_output_tokens(metric.number_output_tokens);
            self.add_total_tokens(metric.number_total_tokens, metric.turn_index);
            self.add_cached_tokens(metric.cached_tokens);
            self.add_finish_reason(metric.finish_reason.clone());
        }

        self
    }

    pub fn add_metrics(&mut self, metrics: &[Metrics]) -> &mut Self {
        for metric in metrics {
            self.add_metric(metric);
        }
        self
    }

    pub fn time(&mut self, time: f64) -> &mut Self {
        self.total_time_seconds = time;
        self
    }

    pub fn num_requests_started(&mut self, num_requests_started: u32) -> &mut Self {
        self.num_requests_started = num_requests_started;
        self
    }
    pub fn num_completed_requests(&mut self, num_completed_requests: u32) -> &mut Self {
        self.num_completed_requests = num_completed_requests;
        self
    }
    pub fn args(&mut self, args: Cli) -> &mut Self {
        self.args = args;
        self
    }

    pub fn build(mut self) -> SummaryMetrics {
        let num_completed_requests_per_min = if self.total_time_seconds > 0.0 {
            (self.num_completed_requests as f64 / self.total_time_seconds) * 60.0
        } else {
            0.0
        };

        let error_rate = if self.num_requests_started > 0 {
            self.number_errors as f64 / self.num_requests_started as f64
        } else {
            0.0
        };

        let cache_hit_rate = match self.cached_tokens_sum {
            Some(sum) if self.cached_total_tokens_sum > 0 => {
                Some(sum as f64 / self.cached_total_tokens_sum as f64)
            }
            _ => None,
        };

        SummaryMetrics {
            itl_ms: calculate_percentiles_f64(&self.itl_vec),
            ttft_s: calculate_percentiles_f64(&self.ttft_vec),
            end_to_end_latency_s: calculate_percentiles_f64(&self.end_to_end_latency_vec),
            prefill_throughput_tps: calculate_percentiles_f64(&self.prefill_throughput_tps_vec),
            decode_throughput_tps: calculate_percentiles_f64(&self.decode_throughput_tps_vec),
            input_tokens: calculate_percentiles_ord(&self.input_tokens_vec),
            output_tokens: calculate_percentiles_ord(&self.output_tokens_vec),
            cache_hit_rate,
            error_code_frequency: std::mem::take(&mut self.error_code_frequency),
            finish_reasons: std::mem::take(&mut self.finish_reasons),
            number_errors: self.number_errors,
            num_requests_started: self.num_requests_started,
            num_completed_requests: self.num_completed_requests,
            num_completed_requests_per_min,
            error_rate,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            args: self.args,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use statrs::statistics::{Data, Distribution};
    #[test]
    fn test_itl() {
        let vec = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let data = Data::new(vec);
        let mean = data.mean().unwrap();
        let stddev = data.std_dev().unwrap();

        let metrics_1 = Metrics {
            itl_ms_vec: vec![1.0, 2.0, 3.0],
            ..Default::default()
        };
        let metrics_2 = Metrics {
            itl_ms_vec: vec![4.0, 5.0, 6.0],
            ..Default::default()
        };
        let mut builder = SummaryBuilder::new();
        builder.add_metric(&metrics_1);
        builder.add_metric(&metrics_2);
        assert_eq!(builder.itl_vec, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

        let summary = builder.build();
        assert_eq!(summary.itl_ms.mean, mean);
        assert_eq!(summary.itl_ms.stddev, stddev);
    }
    #[test]
    fn test_decode_null_tps() {
        let metrics_1 = Metrics {
            decode_throughput_tps: 10.0,
            ..Default::default()
        };
        let metrics_2 = Metrics {
            decode_throughput_tps: 0.0,
            ..Default::default()
        };
        let mut builder = SummaryBuilder::new();
        builder.add_metric(&metrics_1);
        builder.add_metric(&metrics_2);
        assert_eq!(builder.decode_throughput_tps_vec, vec![10.0]);

        let summary = builder.build();
        assert_eq!(summary.decode_throughput_tps.mean, 10.0);
    }
    #[test]
    fn test_cache_hit_rate_keeps_none_turns_in_denominator() {
        use crate::args::Cli;
        let mut args = Cli::default();
        // multi_turn=3 means turn indices 0 and 1 are re-sent (cacheable), turn 2 is last.
        args.multi_turn = 3;

        let mut builder = SummaryBuilder::new();
        builder.args(args);

        // Turn 0: observed cached=100 of re-sent 200 tokens.
        builder.add_metric(&Metrics {
            number_total_tokens: 200,
            cached_tokens: Some(100),
            turn_index: 0,
            ..Metrics::default()
        });
        // Turn 1: unobserved (None) -- skipped from numerator but stays in denominator.
        builder.add_metric(&Metrics {
            number_total_tokens: 100,
            cached_tokens: None,
            turn_index: 1,
            ..Metrics::default()
        });
        // Turn 2: last turn, never re-sent, excluded from denominator.
        builder.add_metric(&Metrics {
            number_total_tokens: 50,
            cached_tokens: Some(0),
            turn_index: 2,
            ..Metrics::default()
        });

        let summary = builder.build();
        // numerator=100, denominator=200+100=300 => 1/3. No longer None.
        assert_eq!(summary.cache_hit_rate, Some(1.0 / 3.0));
    }

    #[test]
    fn test_cache_hit_rate_none_when_no_cacheable_history() {
        use crate::args::Cli;
        let mut args = Cli::default();
        // Single-turn mode: no turn is re-sent, so nothing is cacheable.
        args.multi_turn = 1;

        let mut builder = SummaryBuilder::new();
        builder.args(args);
        builder.add_metric(&Metrics {
            number_total_tokens: 100,
            cached_tokens: Some(100),
            turn_index: 0,
            ..Metrics::default()
        });

        let summary = builder.build();
        assert_eq!(summary.cache_hit_rate, None);
    }
}
