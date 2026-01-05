use crate::api::check_endpoint;
use crate::file::{ResultsSaver, load_tokenizer};
use anyhow::Result;
use clap::Parser;
use log::{info, warn};
use tokio::time::Duration;

pub struct AppConfig {
    pub cli_config: crate::args::Cli,
    pub tokenizer: tokenizers::Tokenizer,
    pub api_key: Option<String>,
    pub api_base: String,
    pub api_timeout: Duration,
    pub model: String,
    pub results_saver: ResultsSaver,
}

fn load_and_validate_tokenizer(config: &crate::args::Cli) -> Result<tokenizers::Tokenizer> {
    let tokenizer = load_tokenizer(&config.tokenizer)?;

    // Check if model and tokenizer don't match
    if config.model != config.tokenizer {
        warn!(
            "Tokenizer != model. Due to differences in tokenization, the actual input tokens may not equal {}",
            config.mean_input_tokens
        );
        warn!(
            "You can ignore this warning if the tokenizer is a variation, as this is just a string match check."
        );
    }

    Ok(tokenizer)
}

async fn check_api_endpoint(
    url: &str,
    model: String,
    api_key: Option<String>,
    skip_check: bool,
) -> Result<()> {
    if skip_check {
        info!("Skipping endpoint connectivity check");
        return Ok(());
    }

    match check_endpoint(url, model, api_key).await {
        Ok(msg) => {
            info!("{}", msg);
        }
        Err(e) => {
            log::error!("Failed to connect to API endpoint: {}", e);
            log::error!("Use --no-check-endpoint to skip this check if needed");
            log::error!("For detailed logging, use: RUST_LOG=INFO");
            return Err(e);
        }
    }

    Ok(())
}

fn parse_environment_variables() -> Result<(Option<String>, String, Duration)> {
    let api_key = std::env::var("OPENAI_API_KEY").ok();

    // Read API timeout from environment variable
    let api_timeout = match std::env::var("OPENAI_API_TIMEOUT") {
        Ok(v) => match v.parse::<u64>() {
            Ok(timeout) => timeout,
            Err(_) => {
                warn!("Error: OPENAI_API_TIMEOUT='{}' is not a valid integer", v);
                warn!("Expected format: OPENAI_API_TIMEOUT=600");
                warn!("Defaulting to 600 seconds");
                600
            }
        },
        Err(_) => 600,
    };
    let api_timeout = Duration::from_secs(api_timeout);

    // Read API base from environment variable
    let url = match std::env::var("OPENAI_API_BASE") {
        Ok(v) => v,
        Err(_) => {
            return Err(anyhow::anyhow!("OPENAI_API_BASE is not set"));
        }
    };

    Ok((api_key, url, api_timeout))
}
// This function should handle all the preflight tasks
pub async fn load_configuration() -> Result<AppConfig> {
    // Initialize logger with warn level by default
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn)
        .init();

    // Parse CLI arguments
    let cli_config = crate::args::Cli::parse();

    // Parse environment variables
    let (api_key, api_base, api_timeout) = parse_environment_variables()?;

    // Load and validate tokenizer
    let tokenizer = load_and_validate_tokenizer(&cli_config)?;

    // Check API endpoint
    check_api_endpoint(
        &api_base,
        cli_config.model.clone(),
        api_key.clone(),
        cli_config.no_check_endpoint,
    )
    .await?;

    let model = cli_config.model.clone();

    // Initialize results saver
    let results_saver = ResultsSaver::new(
        cli_config.results_dir.clone(),
        model.clone(),
        cli_config.mean_input_tokens,
        cli_config.mean_output_tokens,
    );

    Ok(AppConfig {
        cli_config,
        tokenizer,
        api_key,
        api_base,
        api_timeout,
        model,
        results_saver,
    })
}
