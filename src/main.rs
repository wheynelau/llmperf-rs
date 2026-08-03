use anyhow::Result;
use futures::StreamExt;
use log::{info, warn};
use std::sync::Once;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, timeout};

// Import modules from lib.rs
use token_benchmark::config;
use token_benchmark::metrics;
use token_benchmark::prompt;

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    // Log one warning if forwarding a metric to the saver fails; sends stay
    // non-fatal. Declared first so clippy doesn't flag items-after-statements.
    static SAVER_SEND_WARNED: Once = Once::new();

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

    // Drop the main-thread sender so the channel closes once every session
    // sender drops, letting `receiver.recv()` return None and the loop end.
    drop(task_stream.sender);

    // `stream` is a local impl Stream + already Unpin (TaskStream::stream), so
    // pin it once and poll it in select so the session futures keep sending into
    // the channel. The receiver closes when every session sender drops.
    let process = async {
        tokio::pin!(stream);

        // Shared by both arms so every metric is: (1) counted, (2) shoved into
        // `collected_metrics` stripped (content/reasoning = None) for the
        // summary, and (3) forwarded ORIGINAL to `saver_tx` so the
        // individual-responses file keeps the full data.
        let mut handle_metric = |metric: metrics::Metrics| {
            if metric.error_msg.is_some() {
                failed_tasks += 1;
            } else {
                completed_tasks += 1;
            }

            let mut lean = metric.clone();
            lean.content = None;
            lean.reasoning = None;
            collected_metrics.push(lean);

            if saver_tx.send(metric).is_err() {
                SAVER_SEND_WARNED.call_once(|| {
                    warn!(
                        "Failed to forward a metric to the results saver; \
                         the saver may not be active"
                    );
                });
            }
        };

        loop {
            tokio::select! {
                stream_item = stream.next() => {
                    // Stream ended: drain any buffered metrics, then stop.
                    if stream_item.is_none() {
                        while let Some(metric) = receiver.recv().await {
                            handle_metric(metric);
                        }
                        break;
                    }
                    // `Some(Vec<Metrics>)` — the per-session bundle. Intentionally
                    // discarded: metrics already flowed one-by-one via the
                    // channel. Single delivery path; do NOT forward it again.
                }
                maybe_metric = receiver.recv() => {
                    let Some(metric) = maybe_metric else { break; };
                    handle_metric(metric);
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
    info!("Collected {} metrics", collected_metrics.len());

    // Check if any metrics were collected
    if collected_metrics.is_empty() {
        warn!("No metrics were collected. Skipping summary generation and results saving.");
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
