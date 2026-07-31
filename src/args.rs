use clap::Parser;
use log::warn;
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::sync::Once;

const MIN_INPUT_TOKENS: u32 = 50;
const MIN_OUTPUT_TOKENS: u32 = 1;

/// validator for token arguments with a minimum value
fn validate_tokens(value: &str, field_name: &str, min: u32) -> Result<u32, String> {
    let tokens: u32 = value.parse().map_err(|_| {
        format!(
            "Invalid value '{value}' for --{field_name}: must be a valid number between {min} and {max}",
            max = u32::MAX
        )
    })?;

    if tokens < min {
        return Err(format!(
            "Invalid value '{}' for --{field_name}: must be at least {min} token{}",
            value,
            if min > 1 { "s" } else { "" }
        ));
    }

    Ok(tokens)
}

/// validator for `mean_input_tokens`
fn validate_mean_input_tokens(value: &str) -> Result<u32, String> {
    validate_tokens(value, "mean-input-tokens", MIN_INPUT_TOKENS)
}

/// validator for `mean_output_tokens`
fn validate_mean_output_tokens(value: &str) -> Result<u32, String> {
    validate_tokens(value, "mean-output-tokens", MIN_OUTPUT_TOKENS)
}

/// validator for `multi_turn`
fn validate_multi_turn(value: &str) -> Result<u32, String> {
    let num_turns: u32 = value.parse().map_err(|_| {
        format!("Invalid value '{value}' for --multi-turn: must be a positive integer")
    })?;

    if num_turns < 1 {
        return Err(format!(
            "Invalid value '{value}' for --multi-turn: must be at least 1"
        ));
    }

    Ok(num_turns)
}

#[derive(Parser, Default, Serialize, Debug, Clone)]
#[command(
    version,
    about = "Run a token throughput and latency benchmark.",
    long_about = None
)]
pub struct Cli {
    /// The model to use for this load test.
    #[arg(long, required = true, env)]
    pub model: String,

    /// The tokenizer used for calculating the number of input tokens.
    #[arg(
        long,
        default_value = "hf-internal-testing/llama-tokenizer",
        long_help = "The tokenizer used for calculating the number of input tokens. The original llmperf code fixes this tokenizer, but you can pass in the path to a local tokenizer.json file or a model identifier from the huggingface hub.",
        env
    )]
    pub tokenizer: String,

    /// The mean number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "550", value_parser = validate_mean_input_tokens, env)]
    pub mean_input_tokens: u32,

    /// The standard deviation of number of tokens to send in the prompt for the request.
    #[arg(long, default_value = "150", env)]
    pub stddev_input_tokens: u32,

    /// The mean number of tokens to generate from each llm request.
    #[arg(
        long,
        default_value = "150",
        value_parser = validate_mean_output_tokens,
        long_help = "The mean number of tokens to generate from each llm request. This is the max_tokens param for the completions API. \nNote that this is not always the number of tokens returned.",
        env
    )]
    pub mean_output_tokens: u32,

    /// The standard deviation on the number of tokens to generate per llm request.
    #[arg(long, default_value = "80", env)]
    pub stddev_output_tokens: u32,

    /// The number of concurrent requests to send. Its recommended to not set this value too high >10000.
    #[arg(long, default_value = "10", env)]
    pub num_concurrent_requests: usize,

    /// The hard timeout for the test in seconds. Set to 0 for no timeout.
    #[arg(long, default_value = "90", env)]
    pub timeout: u64,

    /// The number of requests to complete before finishing the test.
    #[arg(
        long,
        default_value = "10",
        long_help = "The number of requests to complete before finishing the test. \nNote that it's possible for the test to timeout first.",
        env
    )]
    pub max_num_completed_requests: u32,

    /// Additional sampling params to send with each request to the LLM API.
    /// No additional sampling params are sent.
    /// Currently not in use.
    #[arg(long, default_value = "{}", env)]
    pub additional_sampling_params: String,

    /// The directory to save the results to. If not specified, results are not saved.
    #[arg(long, env)]
    pub results_dir: Option<String>,

    /// The name of the llm api to use. Can select from supported APIs. Only supports `openai` now.
    #[arg(long, default_value = "openai", env)]
    pub llm_api: String,

    /// Metadata to include in the results, e.g. name=foo,bar=1.
    #[arg(
        long,
        long_help = concat!(
    "These will be added to the metadata field of the results. ",
    "Can be specified multiple times: --metadata name=foo --metadata bar=1 ",
    "As environment variable: METADATA='name=foo,bar=1' (comma-separated)"),
        value_name = "KEY=VALUE",
        num_args = 0..,
        env)]
    #[serde(serialize_with = "serialize_metadata")]
    pub metadata: Vec<String>,

    /// Disable API sanity check before running benchmark.
    #[arg(
        long,
        default_value = "false",
        action = clap::ArgAction::SetTrue,
        long_help = "Disable API sanity check before running benchmark.\n\nThis posts a GET request to the /models endpoint to ensure the API is reachable.\nIf your endpoint does not support this, you can disable this check.",
        env
    )]
    pub no_check_endpoint: bool,

    /// Disable reasoning on endpoints
    #[arg(
        long = "no-thinking",
        default_value = "true",
        action = clap::ArgAction::SetFalse,
        long_help = "Disable reasoning on endpoints. The endpoint needs to support chat_template_kwargs, and it sends thinking: false and enable_thinking: false in the request body.",
        env
    )]
    pub thinking: bool,

    /// Number of conversation turns for multi-turn benchmarking.
    /// If not specified, runs single-turn mode.
    #[arg(
        long,
        value_parser = validate_multi_turn,
        default_value = "1",
        long_help = "Number of conversation turns for multi-turn benchmarking. Each turn uses the previous response to build the message history. If not specified, runs single-turn mode.",
        env
    )]
    pub multi_turn: u32,

    /// Db url for reporting run
    /// Not reported in the output or anywhere else
    #[arg(
        long,
        long_help = "DB Url to report results to, recommend to use EnvVars instead due to secrets",
        env,
        hide_env_values = true
    )]
    #[serde(skip_serializing)]
    pub db_url: Option<String>,

    /// Additional HTTP request headers sent with every chat-completions request.
    #[arg(
        long,
        long_help = concat!(
            "Extra HTTP headers sent on every chat-completions request. ",
            "Useful for backend debug toggles (e.g. `x-bf-store-raw-request-response`, ",
            "provider-side trace knobs) that aren't first-class CLI flags. ",
            "Repeatable on the CLI; comma-separated in the `HEADERS` env var. ",
            "Content-Type and Authorization cannot be overridden."
        ),
        value_name = "KEY=VALUE",
        num_args = 0..,
        env
    )]
    #[serde(serialize_with = "serialize_headers")]
    pub headers: Vec<String>,
}

impl Cli {
    /// Parse `--headers` into a `HashMap`, dropping any entry that the
    /// transport manages. Warns once per process on the first blocked entry.
    pub fn extra_headers(&self) -> HashMap<String, String> {
        static BLOCKED_WARN_ONCE: Once = Once::new();
        let mut out = HashMap::new();
        for kv in &self.headers {
            let Some((name, value)) = kv.split_once('=') else {
                warn!("Ignoring malformed --headers entry '{kv}': expected 'key=value'");
                continue;
            };
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "content-type" | "authorization"
            ) {
                BLOCKED_WARN_ONCE.call_once(|| {
                    warn!(
                        "--headers cannot override transport-managed header '{name}'; dropping it. \
                         Subsequent override attempts will be silently ignored."
                    );
                });
                continue;
            }
            out.insert(name.to_string(), value.to_string());
        }
        out
    }
}

/// Converts Vec<String> of "key=value" pairs to `HashMap`<String, String>
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

/// Converts Vec<String> of "key=value" pairs to `HashMap`<String, String> for
/// the `headers` field. Same shape as `serialize_metadata`; standalone copy to
/// keep the two serde attributes independent.
fn serialize_headers<S>(headers: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = HashMap::new();
    for item in headers {
        match item.split_once('=') {
            Some((k, v)) => {
                map.insert(k.to_string(), v.to_string());
            }
            None => {
                warn!("Ignoring malformed headers entry '{item}': expected format 'key=value'");
            }
        }
    }
    map.serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli_with_headers(headers: &[&str]) -> Cli {
        Cli {
            headers: headers.iter().map(|s| (*s).to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn extra_headers_passes_valid_entries_through() {
        let cli = cli_with_headers(&["X-Foo=bar", "x-other=value"]);
        let h = cli.extra_headers();
        assert_eq!(h.get("X-Foo"), Some(&"bar".to_string()));
        assert_eq!(h.get("x-other"), Some(&"value".to_string()));
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn extra_headers_drops_content_type_with_mixed_case() {
        let cli = cli_with_headers(&[
            "X-Foo=bar",
            "content-type=text/plain",
            "Content-Type=application/json",
        ]);
        let h = cli.extra_headers();
        assert_eq!(h.get("X-Foo"), Some(&"bar".to_string()));
        assert!(
            !h.keys().any(|k| k.eq_ignore_ascii_case("content-type")),
            "Content-Type must never be settable via --headers"
        );
    }

    #[test]
    fn extra_headers_drops_authorization_with_mixed_case() {
        let cli = cli_with_headers(&[
            "X-Foo=bar",
            "Authorization=Bearer hacked",
            "AUTHORIZATION=x",
        ]);
        let h = cli.extra_headers();
        assert_eq!(h.get("X-Foo"), Some(&"bar".to_string()));
        assert!(
            !h.keys().any(|k| k.eq_ignore_ascii_case("authorization")),
            "Authorization must never be settable via --headers"
        );
    }

    #[test]
    fn extra_headers_drops_malformed_entries() {
        let cli = cli_with_headers(&["ok=value", "missing-equals"]);
        let h = cli.extra_headers();
        assert_eq!(h.get("ok"), Some(&"value".to_string()));
        assert_eq!(h.len(), 1);
    }
}
