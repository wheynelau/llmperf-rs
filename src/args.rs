use clap::Parser;
use log::warn;
use serde::{Serialize, Serializer};
use std::collections::HashMap;

/// validator for mean_input_tokens
fn validate_mean_input_tokens(value: &str) -> Result<u32, String> {
    let tokens: u32 = value.parse().map_err(|_| {
        format!("Invalid value '{}' for --mean-input-tokens: must be a valid number between 50 and 4294967295", value)
    })?;

    if tokens < 50 {
        return Err(format!(
            "Invalid value '{}' for --mean-input-tokens: must be at least 50 tokens",
            value
        ));
    }

    Ok(tokens)
}
/// validator for mean_output_tokens
fn validate_mean_output_tokens(value: &str) -> Result<u32, String> {
    let tokens: u32 = value.parse().map_err(|_| {
        format!("Invalid value '{}' for --mean-output-tokens: must be a valid number between 1 and 4294967295", value)
    })?;

    if tokens < 1 {
        return Err(format!(
            "Invalid value '{}' for --mean-output-tokens: must be at least 1 token",
            value
        ));
    }

    Ok(tokens)
}

#[derive(Parser, Default, Serialize, Debug)]
#[command(
    version,
    about = "Run a token throughput and latency benchmark.",
    long_about = None
)]
pub struct Cli {
    /// The model to use for this load test.
    #[arg(long, required = true)]
    pub model: String,

    /// The tokenizer used for calculating the number of input tokens.  
    #[arg(
        long,
        default_value = "hf-internal-testing/llama-tokenizer",
        long_help = "The tokenizer used for calculating the number of input tokens. The original llmperf code fixes this tokenizer, but you can pass in the path to a local tokenizer.json file or a model identifier from the huggingface hub."
    )]
    pub tokenizer: String,

    /// The mean number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "550", value_parser = validate_mean_input_tokens)]
    pub mean_input_tokens: u32,

    /// The standard deviation of number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "150")]
    pub stddev_input_tokens: u32,

    /// The mean number of tokens to generate from each llm request.
    #[arg(
        long,
        default_value = "150",
        value_parser = validate_mean_output_tokens,
        long_help = "The mean number of tokens to generate from each llm request. This is the max_tokens param for the completions API. \nNote that this is not always the number of tokens returned."
    )]
    pub mean_output_tokens: u32,

    /// The standard deviation on the number of tokens to generate per llm request.
    #[arg(long, default_value = "80")]
    pub stddev_output_tokens: u32,

    /// The number of concurrent requests to send. Its recommended to not set this value too high >10000.
    #[arg(long, default_value = "10")]
    pub num_concurrent_requests: usize,

    /// The hard timeout for the test in seconds.
    #[arg(long, default_value = "90")]
    pub timeout: u64,

    /// The number of requests to complete before finishing the test.
    #[arg(
        long,
        default_value = "10",
        long_help = "The number of requests to complete before finishing the test. \nNote that it's possible for the test to timeout first."
    )]
    pub max_num_completed_requests: u32,

    /// Additional sampling params to send with each request to the LLM API.
    /// No additional sampling params are sent.
    /// Currently not in use.
    #[arg(long, default_value = "{}")]
    pub additional_sampling_params: String,

    /// The directory to save the results to. If not specified, results are not saved.
    #[arg(long)]
    pub results_dir: Option<String>,

    /// The name of the llm api to use. Can select from supported APIs. Only supports `openai` now.
    #[arg(long, default_value = "openai")]
    pub llm_api: String,

    /// Metadata to include in the results, e.g. name=foo,bar=1.
    /// These will be added to the metadata field of the results.
    /// Can be specified multiple times: --metadata name=foo --metadata bar=1
    #[arg(long, value_name = "KEY=VALUE", num_args = 0..)]
    #[serde(serialize_with = "serialize_metadata")]
    pub metadata: Vec<String>,

    /// Disable API sanity check before running benchmark.
    #[arg(
        long,
        default_value = "false",
        action = clap::ArgAction::SetTrue,
        long_help = "Disable API sanity check before running benchmark.\n\nThis posts a GET request to the /models endpoint to ensure the API is reachable.\nIf your endpoint does not support this, you can disable this check."
    )]
    pub no_check_endpoint: bool,
}

/// Converts Vec<String> of "key=value" pairs to HashMap<String, String>
fn serialize_metadata<S>(metadata: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = HashMap::new();
    for item in metadata {
        match item.split_once('=') {
            Some((k, v)) => {
                map.insert(k.to_string(), v.to_string());
            }
            None => {
                warn!("Ignoring malformed metadata entry '{item}': expected format 'key=value'");
            }
        }
    }
    map.serialize(serializer)
}
