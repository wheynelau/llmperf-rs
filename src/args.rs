use clap::Parser;
use serde::Serialize;

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
    #[arg(long, default_value = "550")]
    pub mean_input_tokens: u32,

    /// The standard deviation of number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "150")]
    pub stddev_input_tokens: u32,

    /// The mean number of tokens to generate from each llm request.
    #[arg(
        long,
        default_value = "150",
        long_help = "The mean number of tokens to generate from each llm request. This is the max_tokens param for the completions API. \nNote that this is not always the number of tokens returned."
    )]
    pub mean_output_tokens: u32,

    /// The standard deviation on the number of tokens to generate per llm request.
    #[arg(long, default_value = "80")]
    pub stddev_output_tokens: u32,

    /// The number of concurrent requests to send.
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

    /// A comma separated list of metadata to include in the results, e.g. name=foo,bar=1.
    /// These will be added to the metadata field of the results.
    #[arg(long, default_value = "")]
    pub metadata: String,

    /// Disable API sanity check before running benchmark.
    #[arg(
        long,
        default_value = "false",
        action = clap::ArgAction::SetTrue,
        long_help = "Disable API sanity check before running benchmark.\n\nThis posts a GET request to the /models endpoint to ensure the API is reachable.\nIf your endpoint does not support this, you can disable this check."
    )]
    pub no_check_endpoint: bool,
}
