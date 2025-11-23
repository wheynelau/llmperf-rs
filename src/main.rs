use anyhow::{Error, Result};
use clap::Parser;
use futures::stream::{self, StreamExt};
use indicatif::ProgressBar;
use log::{error, info, warn};
use token_benchmark::file::load_tokenizer;
use tokio::time::{Duration, Instant, timeout};

// Set up dhat global allocator for heap profiling
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// Import modules from lib.rs
use token_benchmark::api;
use token_benchmark::args;
use token_benchmark::file;
use token_benchmark::metrics;
use token_benchmark::prompt;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize dhat profiler if enabled
    let _profiler = if std::env::var("DHAT_PROF").is_ok() {
        Some(dhat::Profiler::new_heap())
    } else {
        None
    };

    // Initialize logger with warn level by default
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn)
        .init();
    let config = args::Cli::parse();

    // Check if model and tokenizer don't match
    if config.model != config.tokenizer {
        warn!(
            "Tokenizer not provided, will default to {}. Due to differences in tokenization, the actual input tokens may not equal {}",
            config.tokenizer, config.mean_input_tokens
        );
    }

    // Init the default tokenizer
    let tokenizer = load_tokenizer(&config.tokenizer);

    let mean_input_tokens: u32 = config.mean_input_tokens;
    let mean_output_tokens: u32 = config.mean_output_tokens;
    let stddev_output_tokens: u32 = config.stddev_output_tokens;
    let timeout_seconds: u64 = config.timeout;

    let api_key = std::env::var("OPENAI_API_KEY").ok();
    let url = std::env::var("OPENAI_API_BASE")?;

    // Perform endpoint check if not disabled
    if !config.no_check_endpoint {
        match api::check_endpoint(&url, api_key.clone()).await {
            Ok(msg) => {
                info!("{}", msg);
            }
            Err(e) => {
                error!("Use --no-check-endpoint to skip this check if needed");
                error!("For detailed logging, use: RUST_LOG=INFO");
                return Err(e);
            }
        }
    } else {
        info!("Skipping endpoint connectivity check");
    }

    let model = config.model.clone();

    // Initialize results saver
    let results_saver = file::ResultsSaver::new(
        config.results_dir.clone(),
        model.clone(),
        mean_input_tokens,
        mean_output_tokens,
        &config.metadata,
    );

    info!(
        "Starting {} tasks with concurrency of {}",
        config.max_num_completed_requests, config.num_concurrent_requests
    );

    // Load sonnet file once to avoid redundant file I/O
    let sonnet_lines = prompt::read_sonnet_file(&tokenizer, "sonnet.txt")?;

    let inputs: Vec<(String, u32, u32)> = (0..config.max_num_completed_requests)
        .map(|_| {
            let output_tokens =
                prompt::sample_random_positive_int(mean_output_tokens, stddev_output_tokens, 0);
            let (prompt, prompt_tokens) = prompt::randomly_sample_sonnet_lines_prompt(
                config.mean_input_tokens,
                config.stddev_input_tokens,
                output_tokens,
                &tokenizer,
                &sonnet_lines,
            );
            (prompt, output_tokens, prompt_tokens)
        })
        .collect();

    let time = Instant::now();
    // Create a stream of task IDs
    let tasks = stream::iter(inputs.into_iter())
        .map(|(prompt, output_tokens, prompt_tokens)| {
            // TODO? Is there a better way to do this?
            let url = url.clone();
            let api_key = api_key.clone();
            let model = model.clone();
            async move {
                // Perform the HTTP GET request
                let completion = api::models::ChatCompletionRequest::from_prompt(
                    model,
                    prompt,
                    output_tokens,
                    true,
                );
                let post_request = api::models::Request::new(&url, api_key, completion);
                let mut metrics = metrics::Metrics::default();
                // Populate the variables we already know
                metrics.number_input_tokens = prompt_tokens;
                metrics.number_output_tokens = output_tokens;
                metrics.number_total_tokens =
                    metrics.number_input_tokens + metrics.number_output_tokens;
                api::chat_completions(post_request, &mut metrics)
                    .await
                    .map_err(|e| anyhow::anyhow!("API error: {}", e))?;

                Ok::<metrics::Metrics, Error>(metrics)
            }
        })
        .buffered(config.num_concurrent_requests);

    // Set up the timeout duration
    let timeout_duration = Duration::from_secs(timeout_seconds);

    info!(
        "Processing tasks with hard timeout of {} seconds...",
        timeout_seconds
    );

    // Set up progress bar
    let pb = ProgressBar::new(config.max_num_completed_requests as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .expect("Failed to set progress bar style")
            .progress_chars("#>-"),
    );

    // Start the ticking
    pb.enable_steady_tick(Duration::from_millis(200));

    // Process the stream with timeout and track progress
    let mut completed_tasks = 0u32;
    let mut failed_tasks = 0u32;
    let mut collected_metrics = Vec::new();
    let mut stream = tasks;

    match timeout(timeout_duration, async {
        while let Some(result) = stream.next().await {
            match result {
                Ok(metrics) => {
                    completed_tasks += 1;
                    collected_metrics.push(metrics);
                }
                Err(e) => {
                    failed_tasks += 1;
                    error!("Task failed: {}", e);
                }
            }
            pb.inc(1);
        }
    })
    .await
    {
        Ok(_) => {
            pb.finish_with_message("Task processing completed");
        }
        Err(_) => {
            pb.abandon_with_message("Timeout reached");
        }
    }

    let elapsed = time.elapsed();
    // Newline separator for console output after progress bar

    // Always display results and process collected metrics
    if completed_tasks + failed_tasks == config.max_num_completed_requests {
        info!("All tasks completed successfully!");
    } else {
        warn!("Timeout reached after {} seconds!", timeout_seconds);
        warn!("Remaining tasks were cancelled");
        warn!("Note: Some tasks may have been interrupted mid-execution");
    }

    info!("Completed tasks: {}", completed_tasks);
    if failed_tasks > 0 {
        warn!("Failed tasks: {}", failed_tasks);
    }
    info!("Total elapsed time: {:?}", elapsed);
    info!("Collected metrics from {} tasks", collected_metrics.len());

    // Check if any tasks were completed
    if completed_tasks == 0 {
        warn!("No tasks completed successfully. Skipping summary generation and results saving.");
        return Ok(());
    }

    let mut summary_builder = metrics::SummaryBuilder::new();
    summary_builder
        .num_completed_requests(completed_tasks)
        .num_requests_started(completed_tasks + failed_tasks)
        .add_metrics(&collected_metrics)
        .time(elapsed.as_secs_f64())
        .args(config);

    let summary = summary_builder.build();

    // Display summary metrics
    println!("\n{:#?}", summary);

    // Save results if results_dir is specified
    if let Err(e) = results_saver.save_results(&summary, &collected_metrics) {
        warn!("Failed to save results: {}", e);
    }

    Ok(())
}
