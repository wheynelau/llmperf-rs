# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.2...HEAD
[0.6.2]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/wheynelau/llmperf-rs/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/wheynelau/llmperf-rs/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/wheynelau/llmperf-rs/releases/tag/v0.5.0
