use crate::metrics::{Metrics, SummaryMetrics};
use anyhow::{Error, Result};
use log::info;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
}

impl ResultsSaver {
    pub fn new(
        results_dir: Option<String>,
        model: String,
        mean_input_tokens: u32,
        mean_output_tokens: u32,
        metadata_str: &str,
    ) -> Self {
        Self {
            results_dir,
            model,
            mean_input_tokens,
            mean_output_tokens,
            metadata: Self::parse_metadata(metadata_str),
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
            "{}_{}_{}",
            self.model, self.mean_input_tokens, self.mean_output_tokens
        );
        let sanitized_base = self.sanitize_filename(&base_filename);

        let summary_filename = format!("{}_summary", sanitized_base);
        let individual_responses_filename = format!("{}_individual_responses", sanitized_base);

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
            return Err(Error::msg(format!("{} is not a directory", results_dir)));
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
        let summary_path = results_path.join(format!("{}.json", summary_filename));
        let summary_json = serde_json::to_string_pretty(&summary_with_metadata)?;
        fs::write(summary_path, summary_json)?;

        // Save individual responses to JSON file
        let responses_path = results_path.join(format!("{}.json", individual_responses_filename));
        let responses_json = serde_json::to_string_pretty(&individual_responses)?;
        fs::write(responses_path, responses_json)?;

        // Experimental, parquet
        let parquet_path = results_path.join(format!("{}.parquet", individual_responses_filename));

        self.save_metrics_arrow(individual_responses, &parquet_path)?;

        info!("Results saved to {}/", results_path.display());
        info!("  - {}.json", summary_filename);
        info!("  - {}.json", individual_responses_filename);
        info!("  - {}.parquet", individual_responses_filename);

        Ok(())
    }
}
