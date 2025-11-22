use crate::models::FinishReason;
use serde::{Deserialize, Serialize};
use serde_with::with_prefix;
use statrs::statistics::{Data, Distribution, Max, Min, OrderStatistics, Statistics};

#[derive(Default, Serialize, Debug)]
pub struct DetailedStats<T> {
    quantiles_p25: f64,
    quantiles_p50: f64,
    quantiles_p75: f64,
    quantiles_p90: f64,
    quantiles_p95: f64,
    quantiles_p99: f64,
    mean: f64,
    median: f64,
    min: T,
    max: T,
}

pub fn calculate_stats(itl_vec: &[f64]) -> (f64, f64) {
    // Data::new requires an owned container that implements AsMut<[f64]>, so convert the slice to a Vec
    let data = Data::new(itl_vec.to_vec());
    //
    let mean = data.mean().expect("NAN should not appear in itl");
    let stddev = itl_vec.std_dev();
    (mean, stddev)
}

pub fn calculate_percentiles_ord<T>(vec: &[T]) -> DetailedStats<T>
where
    T: Ord + Copy + Into<f64> + Default,
{
    if vec.is_empty() {
        return DetailedStats::default();
    }

    // Use iterator methods for Ord types - more efficient and preserves original type
    let min = vec.iter().min().copied().unwrap();
    let max = vec.iter().max().copied().unwrap();

    // Convert to f64 for statistical calculations
    let float_vec: Vec<f64> = vec.iter().map(|&x| x.into()).collect();
    let mut data = Data::new(float_vec);

    DetailedStats {
        quantiles_p25: data.percentile(25),
        quantiles_p50: data.percentile(50),
        quantiles_p75: data.percentile(75),
        quantiles_p90: data.percentile(90),
        quantiles_p95: data.percentile(95),
        quantiles_p99: data.percentile(99),
        mean: data.mean().unwrap_or(0.0),
        median: data.median(),
        min,
        max,
    }
}

pub fn calculate_percentiles_f64(vec: &[f64]) -> DetailedStats<f64> {
    if vec.is_empty() {
        return DetailedStats::default();
    }

    let mut data = Data::new(vec.to_vec());

    DetailedStats {
        quantiles_p25: data.percentile(25),
        quantiles_p50: data.percentile(50),
        quantiles_p75: data.percentile(75),
        quantiles_p90: data.percentile(90),
        quantiles_p95: data.percentile(95),
        quantiles_p99: data.percentile(99),
        mean: data.mean().unwrap_or(0.0),
        median: data.median(),
        min: data.min(),
        max: data.max(),
    }
}

with_prefix!(itl "itl_ms_");
with_prefix!(ttft "ttft_");
with_prefix!(end_to_end_latency "end_to_end_latency_");
with_prefix!(prefill_throughput_tps "prefill_throughput_tps_");
with_prefix!(decode_throughput_tps "decode_throughput_tps_");
with_prefix!(input_tokens "input_tokens_");
with_prefix!(output_tokens "output_tokens_");

#[derive(Default, Serialize, Debug)]
pub struct SummaryMetrics {
    #[serde(flatten, with = "itl")]
    pub itl: DetailedStats<f64>,
    #[serde(flatten, with = "ttft")]
    pub ttft: DetailedStats<f64>,
    #[serde(flatten, with = "end_to_end_latency")]
    pub end_to_end_latency: DetailedStats<f64>,
    #[serde(flatten, with = "prefill_throughput_tps")]
    pub prefill_throughput_tps: DetailedStats<f64>,
    #[serde(flatten, with = "decode_throughput_tps")]
    pub decode_throughput_tps: DetailedStats<f64>,
    #[serde(flatten, with = "input_tokens")]
    pub input_tokens: DetailedStats<u32>,
    #[serde(flatten, with = "output_tokens")]
    pub output_tokens: DetailedStats<u32>,
    pub error_code_frequency: std::collections::HashMap<u16, u32>,
    pub finish_reasons: std::collections::HashMap<FinishReason, u32>,
    pub number_errors: u32,
    pub num_requests_started: u32,
    pub num_completed_requests: u32,
    pub num_completed_requests_per_min: f64,
    pub error_rate: f64,
    pub timestamp: u64,
}
#[derive(Default, Serialize, Deserialize)]
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
    pub number_errors: u32,
    pub decode_throughput_tps: f64,
    pub response: String,
    pub finish_reason: Option<FinishReason>,
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
    error_code_frequency: std::collections::HashMap<u16, u32>,
    finish_reasons: std::collections::HashMap<FinishReason, u32>,
    number_errors: u32,
    num_requests_started: u32,
    num_completed_requests: u32,
    total_time_seconds: f64,
}

impl SummaryBuilder {
    pub fn new() -> Self {
        SummaryBuilder::default()
    }

    fn add_ttft(&mut self, ttft: f64) {
        self.ttft_vec.push(ttft)
    }

    fn add_e2e_latency(&mut self, e2e_latency: f64) {
        self.end_to_end_latency_vec.push(e2e_latency)
    }

    fn add_prefill_throughput_tps(&mut self, prefill_throughput_tps: f64) {
        self.prefill_throughput_tps_vec.push(prefill_throughput_tps)
    }

    fn add_decode_throughput_tps(&mut self, decode_throughput_tps: f64) {
        self.decode_throughput_tps_vec.push(decode_throughput_tps)
    }

    fn add_input_tokens(&mut self, input_tokens: u32) {
        self.input_tokens_vec.push(input_tokens)
    }
    fn add_output_tokens(&mut self, output_tokens: u32) {
        self.output_tokens_vec.push(output_tokens)
    }

    fn add_error_code(&mut self, error_code: Option<u16>) {
        if let Some(error_code) = error_code {
            *self.error_code_frequency.entry(error_code).or_insert(0) += 1;
            self.number_errors += 1;
        }
    }

    fn add_finish_reason(&mut self, finish_reason: Option<FinishReason>) {
        if let Some(finish_reason) = finish_reason {
            *self.finish_reasons.entry(finish_reason).or_insert(0) += 1;
        }
    }

    fn add_metric(&mut self, metric: &Metrics) -> &mut Self {
        self.itl_vec.extend_from_slice(&metric.itl_ms_vec);
        self.add_ttft(metric.ttft_s);
        self.add_e2e_latency(metric.end_to_end_latency_s);
        self.add_prefill_throughput_tps(metric.prefill_throughput_tps);
        self.add_decode_throughput_tps(metric.decode_throughput_tps);
        self.add_input_tokens(metric.number_input_tokens);
        self.add_output_tokens(metric.number_output_tokens);
        self.add_error_code(metric.error_code);
        self.add_finish_reason(metric.finish_reason.clone());
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

        SummaryMetrics {
            itl: calculate_percentiles_f64(&self.itl_vec),
            ttft: calculate_percentiles_f64(&self.ttft_vec),
            end_to_end_latency: calculate_percentiles_f64(&self.end_to_end_latency_vec),
            prefill_throughput_tps: calculate_percentiles_f64(&self.prefill_throughput_tps_vec),
            decode_throughput_tps: calculate_percentiles_f64(&self.decode_throughput_tps_vec),
            input_tokens: calculate_percentiles_ord(&self.input_tokens_vec),
            output_tokens: calculate_percentiles_ord(&self.output_tokens_vec),
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
        }
    }
}
