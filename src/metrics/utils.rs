use super::models::{DetailedStats, Metrics};
use statrs::statistics::{Data, Distribution, Max, Min, OrderStatistics, Statistics};
use tokio::time::{Duration, Instant};

pub fn calculate_prefill_tps(
    prefill_start: Instant,
    decode_start: Instant,
    input_tokens: u32,
) -> f64 {
    let time = decode_start.duration_since(prefill_start);
    input_tokens as f64 / time.as_secs_f64()
}

pub fn calculate_decode_tps(decode_start: Instant, final_time: Instant, output_tokens: u32) -> f64 {
    let time = final_time.duration_since(decode_start);
    output_tokens as f64 / time.as_secs_f64()
}
pub fn populate_metrics(
    metrics: &mut Metrics,
    prefill_start: Instant,
    ttft: Option<Duration>,
    itl: Vec<Duration>,
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

pub fn calculate_stats(itl_vec: &[f64]) -> (f64, f64) {
    // Data::new requires an owned container that implements AsMut<[f64]>, so convert the slice to a Vec
    let data = Data::new(itl_vec.to_vec());
    //
    let mean = data.mean().expect("NAN should not appear in itl");
    let stddev = itl_vec.std_dev();
    (mean, stddev)
}

pub fn calculate_percentiles_f64(vec: &[f64]) -> DetailedStats<f64> {
    if vec.is_empty() {
        return DetailedStats::default();
    }

    let mut data = Data::new(vec.to_vec());

    DetailedStats::new(
        data.percentile(25),
        data.percentile(50),
        data.percentile(75),
        data.percentile(90),
        data.percentile(95),
        data.percentile(99),
        data.mean().unwrap_or(0.0),
        data.median(),
        data.min(),
        data.max(),
    )
}
