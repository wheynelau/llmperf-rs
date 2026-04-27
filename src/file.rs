use crate::metrics::{Metrics, SummaryMetrics};
use anyhow::{Error, Result};
use log::info;
use regex::Regex;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use tokenizers::{FromPretrainedParameters, Tokenizer};
use tokio::sync::mpsc;

const TIME_FORMAT: &str = "%Y%m%d_%H%M%S%z";

/// Ensure the results directory exists and is valid.
fn ensure_results_dir_exists(results_dir: &str) -> Result<&Path> {
    let results_path = Path::new(results_dir);

    // Create directory if it doesn't exist
    if !results_path.exists() {
        fs::create_dir_all(results_path)?;
    } else if !results_path.is_dir() {
        return Err(Error::msg(format!("{results_dir} is not a directory")));
    }

    Ok(results_path)
}

fn sanitize_filename(base: &str) -> String {
    let re = Regex::new(r"[^\w\d-]+").unwrap();
    let sanitized = re.replace_all(base, "-");
    let re_double_dash = Regex::new(r"-{2,}").unwrap();
    re_double_dash.replace_all(&sanitized, "-").to_string()
}

#[derive(Clone)]
pub struct ResultsSaver {
    pub results_dir: String,
    pub summary_path: std::path::PathBuf,
    pub individual_responses_path: std::path::PathBuf,
}

impl ResultsSaver {
    pub fn try_new(
        results_dir: &str,
        model: &str,
        mean_input_tokens: u32,
        mean_output_tokens: u32,
    ) -> Result<Self> {
        ensure_results_dir_exists(results_dir)?;

        let timestamp = chrono::Local::now().format(TIME_FORMAT).to_string();
        let base_filename = format!("{timestamp}_{model}_{mean_input_tokens}_{mean_output_tokens}");
        let sanitized_base = sanitize_filename(&base_filename);

        let summary_filename = format!("{sanitized_base}_summary");
        let individual_responses_filename = format!("{sanitized_base}_individual_responses");

        let results_path = Path::new(results_dir);
        let summary_path = results_path.join(format!("{summary_filename}.json"));
        let individual_responses_path =
            results_path.join(format!("{individual_responses_filename}.jsonl.zst"));

        // we can immediate log here
        info!(
            "Summary will be saved to {} at the end.",
            summary_path.display()
        );
        info!(
            "Individual responses will be saved to {}",
            individual_responses_path.display()
        );
        info!(
            "Individual responses are zstd-compressed and written incrementally. Please do not edit until completion."
        );
        info!("You can still use tools like zstdcat to read partial files.");

        Ok(Self {
            results_dir: results_dir.to_string(),
            summary_path,
            individual_responses_path,
        })
    }

    fn setup_writer(filepath: &Path) -> Result<Box<dyn Write + Send>> {
        let outfile = File::create(filepath)?;
        let writer: Box<dyn Write + Send> = if filepath
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zst"))
        {
            let encoder = zstd::stream::write::Encoder::new(outfile, 3)?;
            Box::new(encoder.auto_finish())
        } else {
            Box::new(BufWriter::new(outfile))
        };
        Ok(writer)
    }

    pub async fn save_metrics_jsonl_from_channel(
        &self,
        mut receiver: mpsc::UnboundedReceiver<Metrics>,
        filepath: &Path,
    ) -> Result<u32> {
        // Open writer once at the start (reuse existing setup_writer)
        let mut writer = ResultsSaver::setup_writer(filepath)?;
        let mut count = 0u32;

        // Consume metrics from channel as they arrive
        while let Some(metric) = receiver.recv().await {
            // Serialize and write each metric
            let json = serde_json::to_string(&metric)?;
            writeln!(writer, "{json}")?;
            count += 1;
        }

        // Explicitly flush to ensure all data is written and zstd stream is finalized
        writer.flush()?;

        Ok(count)
    }

    pub fn save_summary(&self, summary: &SummaryMetrics) -> Result<()> {
        let summary_json = serde_json::to_string_pretty(summary)?;
        fs::write(&self.summary_path, summary_json)?;

        info!("Summary saved to {}", self.summary_path.display());
        Ok(())
    }
}

pub fn load_tokenizer(path: &str) -> Result<Tokenizer, Error> {
    // Check if the path is a local file
    if Path::new(path).exists() {
        // Attempt to load from the local file and return immediately if successful.
        // Convert the error type to `anyhow::Error` if loading fails.
        // Return error for clarity, don't fallback to pretrained.
        return Tokenizer::from_file(path).map_err(|e| {
            anyhow::anyhow!("Failed to load tokenizer from local file '{path}': {e}")
        });
    }

    // If the path does not exist locally, try to load from a pretrained source.
    let token = std::env::var("HF_TOKEN").ok();
    let params = FromPretrainedParameters {
        token,
        ..Default::default()
    };
    Tokenizer::from_pretrained(path, Some(params))
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer from '{path}': {e}"))
}

pub fn setup_streaming_saver(
    results_saver: &ResultsSaver,
    rx: mpsc::UnboundedReceiver<Metrics>,
) -> Option<tokio::task::JoinHandle<Result<u32>>> {
    let jsonl_path = results_saver.individual_responses_path.clone();

    // Spawn the background task to save metrics
    let saver = results_saver.clone();
    Some(tokio::spawn(async move {
        saver.save_metrics_jsonl_from_channel(rx, &jsonl_path).await
    }))
}
