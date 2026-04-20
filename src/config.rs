use crate::api::check_endpoint;
use crate::args::Cli;
use crate::file::{ResultsSaver, load_tokenizer};
use anyhow::Result;
use clap::Parser;
use indicatif::ProgressBar;
use log::{info, warn};
use tokio::time::Duration;

#[derive(Clone)]
pub struct AppConfig {
    pub cli_config: crate::args::Cli,
    pub tokenizer: tokenizers::Tokenizer,
    pub api_key: Option<String>,
    pub api_base: String,
    pub api_timeout: Duration,
    pub results_saver: Option<ResultsSaver>,
    pub db_pool: Option<sqlx::PgPool>,
    pub progress_bar: ProgressBar,
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

pub async fn check_api_endpoint(
    url: &str,
    model: &str,
    api_key: Option<&str>,
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
    let url = std::env::var("OPENAI_BASE_URL")
        .or_else(|_| std::env::var("OPENAI_API_BASE"))
        .map_err(|_| anyhow::anyhow!("Neither OPENAI_API_BASE nor OPENAI_BASE_URL is set"))?;

    Ok((api_key, url, api_timeout))
}

fn build_progress_bar(cli_config: &Cli) -> Result<ProgressBar, anyhow::Error> {
    // Set up progress bar — total is requests * turns since each turn increments by 1
    let total_turns = cli_config.max_num_completed_requests * cli_config.multi_turn;
    let pb = ProgressBar::new(total_turns as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
            .progress_chars("#>-"),
    );
    pb.enable_steady_tick(Duration::from_millis(40));
    Ok(pb)
}

// This function should handle all the preflight tasks
pub async fn load_configuration() -> Result<AppConfig, anyhow::Error> {
    // Parse CLI arguments
    let cli_config = crate::args::Cli::parse();

    // Parse environment variables
    let (api_key, api_base, api_timeout) = parse_environment_variables()?;

    // Load and validate tokenizer
    let tokenizer = load_and_validate_tokenizer(&cli_config)?;

    // Initialize results saver
    let results_saver = if let Some(ref results_dir) = cli_config.results_dir {
        Some(ResultsSaver::try_new(
            results_dir,
            &cli_config.model,
            cli_config.mean_input_tokens,
            cli_config.mean_output_tokens,
        )?)
    } else {
        info!("No results directory specified; skipping results saving.");
        None
    };

    // Initialise database connection if db_url is provided
    let db_pool = if let Some(ref url) = cli_config.db_url {
        info!("Connecting to database...");
        // Connect early to fail early if we are unable to connect
        let pool = crate::db::connect(url).await?;
        info!("Database connected and schema ready.");
        Some(pool)
    } else {
        None
    };

    // we could just log everything here first
    info!(
        "Starting {total_task} tasks with concurrency of {concurrency} with num_turns {num_turns}",
        total_task = &cli_config.max_num_completed_requests,
        concurrency = &cli_config.num_concurrent_requests,
        num_turns = &cli_config.multi_turn
    );

    // Set up the timeout duration (0 means no timeout)
    if cli_config.timeout == 0 {
        info!("Processing tasks with no timeout (will run until completion)...");
    } else {
        info!(
            "Processing tasks with hard timeout of {} seconds...",
            cli_config.timeout
        );
    }

    let progress_bar = build_progress_bar(&cli_config)?;

    Ok(AppConfig {
        cli_config,
        tokenizer,
        api_key,
        api_base,
        api_timeout,
        results_saver,
        db_pool,
        progress_bar,
    })
}
