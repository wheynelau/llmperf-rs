use super::models::{DetailedStats, Metrics};
use statrs::statistics::{Data, Distribution, Max, Min, OrderStatistics, Statistics};
use std::sync::Arc;
use tokio::time::Duration;

pub fn calculate_percentiles_ord<T>(vec: &[T]) -> DetailedStats<T>
where
    T: Ord + Copy + Into<f64> + Default,
{
    if vec.is_empty() {
        return DetailedStats::default();
    }

    let min = vec.iter().min().copied().unwrap();
    let max = vec.iter().max().copied().unwrap();

    // Convert to f64 and pass ownership directly to avoid a second .to_vec() inside calculate_percentiles_f64
    let float_vec: Vec<f64> = vec.iter().map(|&x| x.into()).collect();
    let f64_stats = calculate_percentiles_f64_owned(float_vec);

    DetailedStats {
        quantiles_p25: f64_stats.quantiles_p25,
        quantiles_p50: f64_stats.quantiles_p50,
        quantiles_p75: f64_stats.quantiles_p75,
        quantiles_p90: f64_stats.quantiles_p90,
        quantiles_p95: f64_stats.quantiles_p95,
        quantiles_p99: f64_stats.quantiles_p99,
        mean: f64_stats.mean,
        median: f64_stats.median,
        stddev: f64_stats.stddev,
        min,
        max,
    }
}

pub fn calculate_prefill_tps(ttft: Option<&Duration>, input_tokens: u32) -> f64 {
    if let Some(ttft_duration) = ttft {
        input_tokens as f64 / ttft_duration.as_secs_f64()
    } else {
        0.0
    }
}

pub fn calculate_decode_tps(itl: &[Duration]) -> f64 {
    // Calculate decode tps with itl
    if itl.is_empty() {
        return 0.0;
    }
    let total_time = itl.iter().sum::<Duration>().as_secs_f64();
    itl.len() as f64 / total_time
}
pub fn populate_metrics(
    metrics: &mut Metrics,
    ttft: Option<Duration>,
    itl: Vec<Duration>,
    content: String,
    reasoning: String,
) {
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
    metrics.content = Some(Arc::from(content));
    metrics.reasoning = Some(Arc::from(reasoning));
}

pub fn calculate_stats(itl_vec: &[f64]) -> (f64, f64) {
    // Data::new requires an owned container that implements AsMut<[f64]>, so convert the slice to a Vec
    let data = Data::new(itl_vec.to_vec());
    let mean = data.mean().unwrap_or(0.0);
    let stddev = itl_vec.std_dev();
    (mean, stddev)
}

pub fn calculate_percentiles_f64(vec: &[f64]) -> DetailedStats<f64> {
    if vec.is_empty() {
        return DetailedStats::default();
    }
    calculate_percentiles_f64_owned(vec.to_vec())
}

fn calculate_percentiles_f64_owned(vec: Vec<f64>) -> DetailedStats<f64> {
    let mut data = Data::new(vec);

    DetailedStats {
        quantiles_p25: data.percentile(25),
        quantiles_p50: data.percentile(50),
        quantiles_p75: data.percentile(75),
        quantiles_p90: data.percentile(90),
        quantiles_p95: data.percentile(95),
        quantiles_p99: data.percentile(99),
        mean: data.mean().unwrap_or(0.0),
        median: data.median(),
        stddev: data.std_dev().unwrap_or(0.0),
        min: data.min(),
        max: data.max(),
    }
}
