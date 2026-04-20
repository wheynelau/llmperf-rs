use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand::Rng;
use rand::rng;
use rand_distr::{Distribution, Normal};
use statrs::distribution::{ContinuousCDF, Normal as StatrsNormal};
use std::hint::black_box;

/// Current rejection sampling method - loops until sample >= min_value
fn sample_rejection(mean: u32, stddev: u32, min_value: u32) -> u32 {
    if stddev == 0 {
        return mean;
    }

    let mut rng = rng();
    let normal = Normal::new(mean as f64, stddev as f64).unwrap();

    loop {
        let sample_f64 = normal.sample(&mut rng);
        let sample_u32 = sample_f64.round() as u32;

        if sample_u32 >= min_value {
            return sample_u32;
        }
    }
}

/// Inverse CDF (quantile function) method - no loop, direct computation
/// Uses the inverse of the normal CDF to generate samples directly
fn sample_inverse_cdf(mean: u32, stddev: u32, min_value: u32) -> u32 {
    if stddev == 0 {
        return mean;
    }

    let mut rng = rng();

    // Generate uniform random variable in (0, 1)
    let u = rng.random_range(0.0..1.0);

    // Use Box-Muller transform to generate normal sample
    // This is equivalent to using the inverse CDF but more numerically stable
    let u1 = (u + f64::EPSILON).max(1.0 - f64::EPSILON);
    let u2 = (rng.random_range(0.0..1.0) as f64).max(f64::EPSILON);

    let z = (2.0_f64 * u2.ln()).sqrt() * ((2.0_f64 * std::f64::consts::PI * u1).cos());

    let sample_f64 = mean as f64 + stddev as f64 * z;
    let sample_u32 = sample_f64.round() as i64;

    // Clamp to minimum value instead of looping
    sample_u32.max(min_value as i64) as u32
}

/// PPF (Percent Point Function / Inverse CDF) method using statrs
/// This is the proper truncated normal sampling method
fn sample_ppf(mean: u32, stddev: u32, min_value: u32) -> u32 {
    if stddev == 0 {
        return mean;
    }

    let dist = StatrsNormal::new(mean as f64, stddev as f64).unwrap();

    // 1. Get the CDF at the minimum bound
    let p_min = dist.cdf(min_value as f64);

    // 2. Generate a random value between p_min and 1.0
    let mut rng = rand::rng();
    let u: f64 = rng.random_range(p_min..1.0);

    // 3. Use the Percent Point Function (Inverse CDF)
    dist.inverse_cdf(u) as u32
}

fn benchmark_sampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("sampling_methods");
    group.sample_size(1000);

    // Test case 1: Normal case - mean well above minimum
    group.bench_with_input(
        BenchmarkId::new("rejection", "mean_1000_stddev_100_min_1"),
        &(1000, 100, 1),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_rejection(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("inverse_cdf", "mean_1000_stddev_100_min_1"),
        &(1000, 100, 1),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_inverse_cdf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("ppf", "mean_1000_stddev_100_min_1"),
        &(1000, 100, 1),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_ppf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    // Test case 2: Truncation near the mean
    group.bench_with_input(
        BenchmarkId::new("rejection", "mean_100_stddev_50_min_50"),
        &(100, 50, 50),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_rejection(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("inverse_cdf", "mean_100_stddev_50_min_50"),
        &(100, 50, 50),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_inverse_cdf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("ppf", "mean_100_stddev_50_min_50"),
        &(100, 50, 50),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_ppf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    // Test case 3: Heavy truncation (minimum far in the tail)
    group.bench_with_input(
        BenchmarkId::new("rejection", "mean_100_stddev_30_min_150"),
        &(100, 30, 150),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_rejection(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("inverse_cdf", "mean_100_stddev_30_min_150"),
        &(100, 30, 150),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_inverse_cdf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("ppf", "mean_100_stddev_30_min_150"),
        &(100, 30, 150),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_ppf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    // Test case 4: Zero stddev (edge case)
    group.bench_with_input(
        BenchmarkId::new("rejection", "mean_100_stddev_0_min_1"),
        &(100, 0, 1),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_rejection(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("inverse_cdf", "mean_100_stddev_0_min_1"),
        &(100, 0, 1),
        |b: &mut criterion::Bencher, &(mean, stddev, min)| {
            b.iter(|| sample_inverse_cdf(black_box(mean), black_box(stddev), black_box(min)));
        },
    );

    group.finish();
}

fn benchmark_distribution_quality(c: &mut Criterion) {
    let mut group = c.benchmark_group("distribution_quality");
    group.sample_size(1000);

    // Benchmark sampling 1000 values and computing statistics (reduced from 10000 for faster benchmarks)
    group.bench_function("rejection_1k_samples", |b| {
        b.iter(|| {
            let samples: Vec<u32> = (0..1000).map(|_| sample_rejection(100, 30, 1)).collect();
            let mean: f64 = samples.iter().map(|&x| x as f64).sum::<f64>() / 1000.0;
            black_box(mean)
        });
    });

    group.bench_function("inverse_cdf_1k_samples", |b| {
        b.iter(|| {
            let samples: Vec<u32> = (0..1000).map(|_| sample_inverse_cdf(100, 30, 1)).collect();
            let mean: f64 = samples.iter().map(|&x| x as f64).sum::<f64>() / 1000.0;
            black_box(mean)
        });
    });

    group.bench_function("ppf_1k_samples", |b| {
        b.iter(|| {
            let samples: Vec<u32> = (0..1000).map(|_| sample_ppf(100, 30, 1)).collect();
            let mean: f64 = samples.iter().map(|&x| x as f64).sum::<f64>() / 1000.0;
            black_box(mean)
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_sampling, benchmark_distribution_quality);
criterion_main!(benches);
