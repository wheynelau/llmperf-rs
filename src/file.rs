use crate::metrics::{Metrics, SummaryMetrics};
use anyhow::{Error, Result};
use log::info;
use regex::Regex;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use tokenizers::{FromPretrainedParameters, Tokenizer};

const TIME_FORMAT: &str = "%Y%m%d_%H%M%S%z";

#[derive(Default)]
pub struct ResultsSaver {
    results_dir: Option<String>,
    model: String,
    mean_input_tokens: u32,
    mean_output_tokens: u32,
    timestamp: String,
}

impl ResultsSaver {
    pub fn new(
        results_dir: Option<String>,
        model: String,
        mean_input_tokens: u32,
        mean_output_tokens: u32,
    ) -> Self {
        let timestamp = chrono::Local::now().format(TIME_FORMAT).to_string();

        Self {
            results_dir,
            model,
            mean_input_tokens,
            mean_output_tokens,
            timestamp,
        }
    }

    fn sanitize_filename(&self, base: &str) -> String {
        let re = Regex::new(r"[^\w\d-]+").unwrap();
        let sanitized = re.replace_all(base, "-");
        let re_double_dash = Regex::new(r"-{2,}").unwrap();
        re_double_dash.replace_all(&sanitized, "-").to_string()
    }

    fn generate_filenames(&self) -> (String, String) {
        let base_filename = format!(
            "{}_{}_{}_{}",
            self.timestamp, self.model, self.mean_input_tokens, self.mean_output_tokens
        );
        let sanitized_base = self.sanitize_filename(&base_filename);

        let summary_filename = format!("{sanitized_base}_summary");
        let individual_responses_filename = format!("{sanitized_base}_individual_responses");

        (summary_filename, individual_responses_filename)
    }

    fn setup_writer(&self, filepath: &Path) -> Result<Box<dyn Write>> {
        let outfile = File::create(filepath)?;
        let writer: Box<dyn Write> = if filepath
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

    fn save_metrics_jsonl(&self, metrics: &[Metrics], filepath: &Path) -> Result<()> {
        let mut writer = self.setup_writer(filepath)?;

        for metric in metrics {
            let json = serde_json::to_string(metric)?;
            writeln!(writer, "{json}")?;
        }

        Ok(())
    }

    pub fn save_results(
        &self,
        summary: SummaryMetrics,
        individual_responses: &[Metrics],
    ) -> Result<()> {
        let results_dir = match &self.results_dir {
            Some(dir) => dir,
            None => return Ok(()),
        };

        let results_path = Path::new(results_dir);

        // Create directory if it doesn't exist
        if !results_path.exists() {
            fs::create_dir_all(results_path)?;
        } else if !results_path.is_dir() {
            return Err(Error::msg(format!("{results_dir} is not a directory")));
        }

        let (summary_filename, individual_responses_filename) = self.generate_filenames();

        // Save summary to JSON file
        let summary_path = results_path.join(format!("{summary_filename}.json"));
        let summary_json = serde_json::to_string_pretty(&summary)?;
        fs::write(summary_path, summary_json)?;

        // Save individual responses to zstd-compressed JSONL
        let jsonl_path = results_path.join(format!("{individual_responses_filename}.jsonl.zst"));

        self.save_metrics_jsonl(individual_responses, &jsonl_path)?;

        info!("Results saved to {}/", results_path.display());
        info!("  - {summary_filename}.json");
        info!("  - {individual_responses_filename}.jsonl.zst");

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
            anyhow::anyhow!("Failed to load tokenizer from local file '{}': {}", path, e)
        });
    }

    // If the path does not exist locally, try to load from a pretrained source.
    let token = std::env::var("HF_TOKEN").ok();
    let params = FromPretrainedParameters {
        token,
        ..Default::default()
    };
    Tokenizer::from_pretrained(path, Some(params))
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer from '{}': {}", path, e))
}
