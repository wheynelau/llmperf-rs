use anyhow::Result;
use futures::StreamExt;
use indicatif::ProgressBar;
use log::{error, info, warn};
use tokio::time::{Duration, Instant, timeout};

// Import modules from lib.rs
use token_benchmark::config;
use token_benchmark::metrics;
use token_benchmark::prompt;

#[tokio::main]
async fn main() -> Result<()> {
    let app_config = config::load_configuration().await?;

    info!(
        "Starting {} tasks with concurrency of {}",
        app_config.cli_config.max_num_completed_requests,
        app_config.cli_config.num_concurrent_requests
    );

    // Load once
    let sonnet_lines = prompt::read_sonnet_file(&app_config.tokenizer, "sonnet.txt")?;

    let inputs = prompt::create_inputs(&sonnet_lines, &app_config);

    // Create a stream of tasks
    let mut stream = prompt::create_tasks(
        inputs,
        app_config.api_timeout,
        app_config.cli_config.num_concurrent_requests,
    );

    let time = Instant::now();
    // Set up the timeout duration
    let timeout_duration = Duration::from_secs(app_config.cli_config.timeout);

    info!(
        "Processing tasks with hard timeout of {} seconds...",
        app_config.cli_config.timeout
    );

    // Set up progress bar
    let pb = ProgressBar::new(app_config.cli_config.max_num_completed_requests as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .expect("Failed to set progress bar style")
            .progress_chars("#>-"),
    );

    // Start the ticking
    pb.enable_steady_tick(Duration::from_millis(40));

    // Process the stream with timeout and track progress
    let mut completed_tasks = 0u32;
    let mut failed_tasks = 0u32;
    let mut collected_metrics = Vec::new();

    match timeout(timeout_duration, async {
        while let Some(result) = stream.next().await {
            match result {
                (metrics, Ok(())) => {
                    completed_tasks += 1;
                    collected_metrics.push(metrics);
                }
                (metrics, Err(e)) => {
                    failed_tasks += 1;
                    error!("Task failed: {}", e);
                    collected_metrics.push(metrics);
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
    if completed_tasks + failed_tasks == app_config.cli_config.max_num_completed_requests {
        info!("All tasks completed successfully!");
    } else {
        warn!(
            "Timeout reached after {} seconds!",
            app_config.cli_config.timeout
        );
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
        .args(app_config.cli_config);

    let summary = summary_builder.build();

    // Display summary metrics
    println!("\n{:#?}", summary);

    // Save results if results_dir is specified
    if let Err(e) = app_config
        .results_saver
        .save_results(&summary, &collected_metrics)
    {
        warn!("Failed to save results: {}", e);
    }

    Ok(())
}
