use anyhow::Result;
use futures::StreamExt;
use log::{info, warn};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, timeout};

// Import modules from lib.rs
use token_benchmark::config;
use token_benchmark::metrics;
use token_benchmark::prompt;

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    env_logger::init();

    let app_config = config::load_configuration().await?;

    // Check API endpoint
    config::check_api_endpoint(
        &app_config.client,
        &app_config.api_base,
        &app_config.cli_config.model,
        app_config.api_key.as_deref(),
        app_config.cli_config.no_check_endpoint,
    )
    .await?;

    // Start the progress bar spin now that preflight INFO logs have settled.
    // Ticking during the endpoint check would race those logs and produce
    // scrambled stderr output.
    app_config
        .progress_bar
        .enable_steady_tick(Duration::from_millis(40));

    let task_stream = prompt::create_task_stream(&app_config)?;
    let stream = task_stream.stream;

    let cli_config = app_config.cli_config.clone();
    let time = Instant::now();

    let mut completed_tasks = 0u32;
    let mut failed_tasks = 0u32;
    let mut collected_metrics = Vec::new();

    // Separate channel the loop forwards each metric to so the file saver (a
    // distinct consumer) sees exactly the same metrics as the summary.
    let (saver_tx, saver_rx) = mpsc::unbounded_channel::<metrics::Metrics>();

    // Spawn the incremental file writer (reuses existing ResultsSaver logic).
    let saver_handle = if let Some(ref results_saver) = app_config.results_saver {
        let jsonl_path = results_saver.individual_responses_path.clone();
        let saver = results_saver.clone();
        Some(tokio::spawn(async move {
            saver
                .save_metrics_jsonl_from_channel(saver_rx, &jsonl_path)
                .await
        }))
    } else {
        None
    };

    let mut receiver = task_stream.receiver;

    // `stream` is a local impl Stream + already Unpin (TaskStream::stream), so
    // pin it once and poll it in select so the session futures keep sending into
    // the channel. The receiver closes when every session sender drops.
    let process = async {
        tokio::pin!(stream);
        loop {
            tokio::select! {
                stream_item = stream.next() => {
                    // Stream ended => every session finished. Drain any metrics
                    // still buffered in the channel, then stop.
                    if stream_item.is_none() {
                        while let Some(mut metric) = receiver.recv().await {
                            if metric.error_msg.is_some() {
                                failed_tasks += 1;
                            } else {
                                completed_tasks += 1;
                            }
                            metric.content = None;
                            metric.reasoning = None;
                            let _ = saver_tx.send(metric.clone());
                            collected_metrics.push(metric);
                        }
                        break;
                    }
                }
                maybe_metric = receiver.recv() => {
                    let Some(mut metric) = maybe_metric else { break; };
                    if metric.error_msg.is_some() {
                        failed_tasks += 1;
                    } else {
                        completed_tasks += 1;
                    }
                    metric.content = None;
                    metric.reasoning = None;
                    let _ = saver_tx.send(metric.clone());
                    collected_metrics.push(metric);
                }
            }
        }
    };

    let timed_out = if app_config.cli_config.timeout == 0 {
        process.await;
        false
    } else {
        let timeout_duration = Duration::from_secs(app_config.cli_config.timeout);
        timeout(timeout_duration, process).await.is_err()
    };

    // Close the file-writer channel now that the loop has ended so the saver
    // flushes and finalizes the zstd file.
    drop(saver_tx);

    if timed_out {
        app_config
            .progress_bar
            .abandon_with_message("Timeout reached");
    } else {
        app_config
            .progress_bar
            .finish_with_message("Task processing completed");
    }

    let elapsed = time.elapsed();

    // Always display results and process collected metrics
    if completed_tasks + failed_tasks
        == app_config.cli_config.max_num_completed_requests * app_config.cli_config.multi_turn
    {
        info!("All tasks completed successfully!");
    } else {
        warn!(
            "Timeout reached after {} seconds!",
            app_config.cli_config.timeout
        );
        warn!("Remaining tasks were cancelled");
        warn!("Note: Some tasks may have been interrupted mid-execution");
    }

    info!("Completed tasks: {completed_tasks}");
    if failed_tasks > 0 {
        warn!("Failed tasks: {failed_tasks}");
    }
    info!("Total elapsed time: {elapsed:?}");
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
        .args(cli_config)
        .add_metrics(&collected_metrics)
        .time(elapsed.as_secs_f64());

    let summary = summary_builder.build();

    // Display summary metrics
    println!("{}", serde_json::to_string_pretty(&summary)?);

    // Wait for the streaming saver to complete and log any errors
    if let Some(handle) = saver_handle {
        match handle.await {
            Ok(Ok(count)) => {
                info!("Saved {count} individual responses to disk");
            }
            Ok(Err(e)) => {
                warn!("Failed to save individual responses: {e}");
            }
            Err(e) => {
                warn!("Saver task panicked: {e}");
            }
        }
    }

    // Save summary if results_dir is specified
    if let Some(ref results_saver) = app_config.results_saver
        && let Err(e) = results_saver.save_summary(&summary)
    {
        warn!("Failed to save summary: {e}");
    }

    // Insert summary into database if db_pool is available
    if let Some(ref pool) = app_config.db_pool {
        match token_benchmark::db::insert_summary(pool, &summary).await {
            Ok(()) => info!("Summary inserted into database."),
            Err(e) => warn!("Failed to insert summary into database: {e}"),
        }
    }

    Ok(())
}
