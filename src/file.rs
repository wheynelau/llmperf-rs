use crate::metrics::{Metrics, SummaryMetrics};
use anyhow::{Error, Result};
use log::info;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokenizers::{FromPretrainedParameters, Tokenizer};

use arrow::datatypes::FieldRef;
use parquet::{
    arrow::arrow_writer::ArrowWriter, basic::Compression, file::properties::WriterProperties,
};
use serde_arrow::schema::{SchemaLike, TracingOptions};

#[derive(Default)]
pub struct ResultsSaver {
    results_dir: Option<String>,
    model: String,
    mean_input_tokens: u32,
    mean_output_tokens: u32,
    metadata: HashMap<String, Value>,
    timestamp: String,
}

impl ResultsSaver {
    pub fn new(
        results_dir: Option<String>,
        model: String,
        mean_input_tokens: u32,
        mean_output_tokens: u32,
        metadata_str: &str,
    ) -> Self {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();

        Self {
            results_dir,
            model,
            mean_input_tokens,
            mean_output_tokens,
            metadata: Self::parse_metadata(metadata_str),
            timestamp,
        }
    }

    fn parse_metadata(metadata_str: &str) -> HashMap<String, Value> {
        let mut metadata = HashMap::new();

        if metadata_str.is_empty() {
            return metadata;
        }

        for pair in metadata_str.split(',') {
            if let Some((key, value)) = pair.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                metadata.insert(key, Value::String(value));
            }
        }

        metadata
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

    fn save_metrics_arrow(
        &self,
        metrics: &[Metrics],
        filepath: &Path,
    ) -> Result<(), anyhow::Error> {
        let mut tracing_options = TracingOptions::default();
        tracing_options.enums_without_data_as_strings = true;
        let fields = Vec::<FieldRef>::from_type::<Metrics>(tracing_options)?;

        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(Default::default())) // Use ZSTD
            .build();

        // Build the record batch
        let batch = serde_arrow::to_record_batch(&fields, &metrics)?;

        let file = fs::File::create(filepath)?;
        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;

        writer.write(&batch)?;
        writer.close()?;

        Ok(())
    }

    pub fn save_results(
        &self,
        summary: &SummaryMetrics,
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

        // Prepare summary with metadata
        let mut summary_with_metadata = json!(summary);
        if let Some(summary_obj) = summary_with_metadata.as_object_mut() {
            for (key, value) in &self.metadata {
                summary_obj.insert(key.clone(), value.clone());
            }
        }

        // Save summary to JSON file
        let summary_path = results_path.join(format!("{summary_filename}.json"));
        let summary_json = serde_json::to_string_pretty(&summary_with_metadata)?;
        fs::write(summary_path, summary_json)?;

        // Deprecated but kept here for reference
        // Depending on the runs, the space required can be up to 5x the parquet size
        // Save individual responses to JSON file
        // let responses_path = results_path.join(format!("{}.json", individual_responses_filename));
        // let responses_json = serde_json::to_string_pretty(&individual_responses)?;
        // fs::write(responses_path, responses_json)?;

        let parquet_path = results_path.join(format!("{individual_responses_filename}.parquet"));

        self.save_metrics_arrow(individual_responses, &parquet_path)?;

        info!("Results saved to {}/", results_path.display());
        info!("  - {summary_filename}.json");
        info!("  - {individual_responses_filename}.parquet");

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
