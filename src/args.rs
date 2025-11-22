use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about = "Run a token throughput and latency benchmark.",
    long_about = None
)]
pub struct Cli {
    /// The model to use for this load test.
    #[arg(long, required = true)]
    pub model: String,

    /// The mean number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "550")]
    pub mean_input_tokens: u32,

    /// The standard deviation of number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "150")]
    pub stddev_input_tokens: u32,

    /// The mean number of tokens to generate from each llm request. This is the max_tokens param
    /// for the completions API. Note that this is not always the number of tokens returned.
    #[arg(long, default_value = "150")]
    pub mean_output_tokens: u32,

    /// The standard deviation on the number of tokens to generate per llm request.
    #[arg(long, default_value = "80")]
    pub stddev_output_tokens: u32,

    /// The number of concurrent requests to send.
    #[arg(long, default_value = "10")]
    pub num_concurrent_requests: usize,

    /// The amount of time to run the load test for.
    #[arg(long, default_value = "90")]
    pub timeout: u64,

    /// The number of requests to complete before finishing the test. Note that it's possible
    /// for the test to timeout first.
    #[arg(long, default_value = "10")]
    pub max_num_completed_requests: u32,

    /// Additional sampling params to send with each request to the LLM API.
    /// (default: {}) No additional sampling params are sent.
    #[arg(long, default_value = "{}")]
    pub additional_sampling_params: String,

    /// The directory to save the results to. (default: "") No results are saved.
    #[arg(long)]
    pub results_dir: Option<String>,

    /// The name of the llm api to use. Can select from supported APIs.
    #[arg(long, default_value = "openai")]
    pub llm_api: String,

    /// A comma separated list of metadata to include in the results, e.g. name=foo,bar=1.
    /// These will be added to the metadata field of the results.
    #[arg(long, default_value = "")]
    pub metadata: String,

    /// Disable API endpoint connectivity check before running benchmark.
    #[arg(long = "no-check-endpoint", default_value = "false", action = clap::ArgAction::SetTrue)]
    pub no_check_endpoint: bool,
}
