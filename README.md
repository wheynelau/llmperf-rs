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
llmperf --model Qwen/Qwen3-4B-Instruct-2507
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

## Roadmap

There is currently no planned features as it is subject to common issues or concerns. The goal is to provide a simple tool, which does not have very heavy configurations.

Some features that were considered but dropped:

- Non streaming requests
- Json inputs

## Additional details

- Some additional docs or details can be found in the [docs](docs) directory.
- A local copy of sonnet.txt is no longer needed as it is baked into the binary with `include_str!`. However, compiling the binary from source needs this file to be present in the root directory.

## Tested endpoints and their notes

- VLLM: Works well, supports streaming. When running against a reasoning model, they do not send back the `</think>` token, but they send a different json schema:

_Note: server was ran with `reasoning_parser: deepseek_r1`_

Example:
```
# Reasoning model response
{"choices":[{"index":0,"delta":{"reasoning":" on","reasoning_content":" on"},"finish_reason":null}]}
# Content
{"choices":[{"index":0,"delta":{"content":" math","reasoning_content":null},"finish_reason":null}]}
```

As such, there could be a missing token for `</think>` but I will have to do more tests to confirm.

- llamacpp: Same as vLLM

_Note: server was ran with `reasoning-format : deepseek`_

