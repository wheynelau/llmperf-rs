# LLM Performance Benchmark

A Rust-based tool for running token throughput and latency benchmarks on language models.

## Installation

### From releases

Download the latest release from [releases](https://github.com/wheynelau/llmperf-rs/releases).

### From source

```bash
# Note that you will need rust for this
# Depending on your distro you may also need other dependencies
cargo build --release
```

## Usage

Run the benchmark with the following command:

```bash
llmperf --model <MODEL_NAME>
```

Replace `<MODEL_NAME>` with the model you want to test.

### Options

Run `llmperf --help` to see all available options and their defaults:

```bash
# Short help
llmperf -h
# Long help
llmperf --help
```

## Example

Basic usage with a specified model:

```bash
export OPENAI_API_BASE=http://localhost:8000/v1 # vLLM endpoint
llmperf --model gpt-3.5-turbo
```

### Environment variables

```bash
# default is warn
export RUST_LOG=INFO # Set log level, DEBUG, INFO, WARN, ERROR
# Default to 600 seconds, this is the timeout per request
export OPENAI_API_TIMEOUT=600 
# Base URL, throws an error if unset
export OPENAI_API_BASE=http://localhost:8000/v1
# API key, optional
export OPENAI_API_KEY=sk-secret-key
# HF_TOKEN, optional, for downloading private tokenizers
export HF_TOKEN=hf-abc123
```

## Additional details

Some additional docs or details can be found in the [docs](docs) directory.

