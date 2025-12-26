use super::models::{DetailedStats, Metrics};
use statrs::statistics::{Data, Distribution, Max, Min, OrderStatistics, Statistics};
use tokio::time::Duration;

pub fn calculate_prefill_tps(ttft: &f64, input_tokens: u32) -> f64 {
    input_tokens as f64 / ttft
}

pub fn calculate_decode_tps(itl: &[Duration]) -> f64 {
    // Don't need the output tokens, just the ITL
    if itl.is_empty() {
        return 0.0;
    }
    let total_time = itl.iter().sum::<Duration>().as_secs_f64();
    itl.len() as f64 / total_time
}
pub fn populate_metrics(metrics: &mut Metrics, itl: Vec<Duration>, response: String) {
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
        data.std_dev().unwrap_or(0.0),
        data.min(),
        data.max(),
    )
}
