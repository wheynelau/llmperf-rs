# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.1] - 2026-08-04

### Added

- `cache_hit_rate` is now persisted to the Postgres `benchmark_results` table as a nullable `DOUBLE PRECISION` column, so existing databases are auto-migrated via an idempotent `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` on connect.

### Changed

- CI now runs `cargo clippy --all-targets -- -D warnings`, matching the local pre-commit hook so test and example code is linted consistently.

## [0.9.0] - 2026-08-04

### Changed

- Renamed the library crate from `token_benchmark` to `llmperf`, matching the package and tool name.
- The summary now includes metrics from turns interrupted by a hard timeout, instead of dropping partial-turn metrics that reached the file saver but not the aggregate. The run loop is the single consumer of the per-turn metrics channel and drives both summary accumulation and file persistence.

## [0.8.0] - 2026-08-04

### Changed

- `cache_hit_rate` no longer nulls the whole run when a turn omits `cached_tokens`. An unobserved turn is now skipped from the numerator but its re-sent history stays in the denominator, so it dilutes the ratio instead. An all-`None` run still reports `None` (distinct from observed `0.0`).
- TTFT is now measured from before the request is sent (so it includes network round-trip + server prefill) and ignores empty/role-announcement deltas, so it reflects time to the first real token.

## [0.7.1] - 2026-07-31

### Added

- Default tokenizer (`hf-internal-testing/llama-tokenizer`) is now baked into the binary. A `build.rs` compresses the tokenizer JSON with zstd at compile time, so the default path requires no network access and no HuggingFace Hub dependency.

### Changed

- Tests now use the baked-in tokenizer instead of downloading from HuggingFace Hub.

## [0.7.0-rc1] - 2026-07-31

### Added

- `cache_hit_rate` summary metric for multi-turn runs, plus per-response `cached_tokens`, `turn_index`, and `cumulative_prior_tokens` fields. The rate is `Σ cached_tokens / Σ total_tokens of non-final turns`: it measures how much of the previously-sent conversation history was actually served from cache, not how much of the current input was cached. See [docs/metrics.md](docs/metrics.md).
- `--headers KEY=VALUE` flag (repeatable on the CLI, comma-separated via the `HEADERS` env var) to send extra HTTP headers on every chat-completions request. `Content-Type` and `Authorization` cannot be overridden.
- Pre-commit hooks (`cargo check`, `cargo clippy`, `cargo fmt`) via `.pre-commit-config.yaml`.

### Changed

- A single shared `reqwest::Client` is now built once and cloned into every task, so concurrent requests reuse the same keep-alive connection pool instead of each creating its own client. Per-request timeouts are set on the request itself rather than on the client.
- `decode_throughput_tps` is now `output_tokens / end_to_end_latency` instead of being derived from inter-token latencies, which was inaccurate for endpoints that emit chunked text.
- Multi-turn requests now echo the previous turn's `reasoning_content` on the assistant message so providers that support prefix caching over reasoning can reuse the KV cache across turns.
- The progress bar's steady tick starts only after the preflight endpoint check, avoiding races between the ticker and preflight INFO logs on stderr.

### Fixed

- Clippy pedantic lint warnings across the codebase.

## [0.6.3] - 2026-04-27

### Added

- `FinishReason::ContentFilter` variant for OpenAI content filter finish reasons.

### Fixed

- `finish_reasons` serializing as `{}` when the backend sends both `finish_reason` and `stop_reason` in the same JSON chunk. The `#[serde(alias = "stop_reason")]` attribute treated them as the same field, causing a "duplicate field" error that silently dropped the entire event.
- `FinishReason` now includes an `Other` catch-all variant to handle unrecognized values from backends.

## [0.6.2] - 2026-04-21

### Changed
- Removed env values in arg.rs, left defaults
- Fix env variables showing in clap help, add `hide_env_values`

## [0.6.1] - 2026-04-21

### Changed

- Extract `StreamState` and `handle_response` from the SSE streaming loop.
- Share the `User-Agent` header across the streaming client and `/models` health check via a `USER_AGENT` const.

## [0.6.0] - 2026-04-21

### Added

- PostgreSQL database output: pass a `--db-url` to upload benchmark results directly to a database.
- Environment variable support for all CLI flags via `clap`, every option can now be set through environment variables instead of command-line arguments.
- Example Python script for reading results from the database with pandas.
- This changelog.

### Changed

- Replaced `eventsource-client` with a `reqwest`-based SSE parser.

## [0.5.0] - 2026-04-15

### Added

- Incremental metrics via an `mpsc` sender, allowing results to be streamed out of `run_session` as they arrive.

[Unreleased]: https://github.com/wheynelau/llmperf-rs/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/wheynelau/llmperf-rs/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/wheynelau/llmperf-rs/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/wheynelau/llmperf-rs/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/wheynelau/llmperf-rs/compare/v0.7.0-rc1...v0.7.1
[0.7.0-rc1]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.3...v0.7.0-rc1
[0.6.3]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/wheynelau/llmperf-rs/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/wheynelau/llmperf-rs/releases/tag/v0.5.0
