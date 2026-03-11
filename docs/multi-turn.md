# Multi-Turn Benchmarking

## Overview

Multi-turn benchmarking simulates realistic conversation flows where each request builds on previous exchanges. Instead of isolated requests, the tool now tracks conversation state across multiple turns.

## How It Works

### Core Components

**Session State (`session.rs`)**
- `MultiTurnSession` struct maintains conversation history
- Stores accumulated `messages` (user + assistant roles)
- Tracks `turn_index` to know which turn we're on
- Builds requests with full message context

**Prompt Generation (`prompt.rs`)**
- `PromptConfig`: Shared state for generating prompts across turns
- `SessionInput`: Bundles session + token counts + config for each benchmark run
- `create_session_inputs()`: Creates initial sessions with first user prompt
- `run_session()`: Loops through turns, making requests and advancing conversation

**Flow**
1. Initial prompt generated from sonnet sampling
2. Session created with `num_turns` configuration
3. For each turn:
   - Build request with current message history
   - Send request, stream response
   - Store assistant response in message history
   - Generate next user prompt (if more turns remain)
   - Increment turn counter
4. Session completes when all turns done

### CLI Usage

```bash
# Single turn (default)
llmperf --model my-model --mean-input-tokens 500 --max-num-completed-requests 10

# Multi-turn: 5 conversation turns per session
llmperf --model my-model --multi-turn 5 --max-num-completed-requests 10
```

**Key Options**
- `--multi-turn N`: Number of turns per session (default: 1)
- `--turn-delay-mean`: Mean delay between turns in seconds (default: 0)
- `--turn-delay-stddev`: Stddev on turn delay (default: 0)

### Progress Tracking

Progress bar shows total turns: `requests × turns`

Example: 10 requests × 5 turns = 50 progress steps

### Metrics

Each turn produces separate metrics with `turn_index`:
- `turn_index`: 0-based turn number
- Input/output tokens tracked per turn
- Full message history included in requests

### Concurrency

Sessions run concurrently via `buffer_unordered()`. Each session is independent - no shared state between concurrent sessions.

## Notes on results

Input tokens will no longer be accurate as Metrics are tracked per request

For example in one session, this is how the metrics will look like for these parameters:
```bash
--mean-input-tokens=2000
--mean-output-tokens=100
--num-turns=5
# stddev variables at 0
```

We assume that the chat template does not contribute additional tokens, then this is how metrics will look like:

Metric 1:
input_tokens = 2000
output_tokens = 100

Metric 2:
input_tokens = 2000 + 100 + 2000 = 4100
output_tokens = 100

...

Metric N
input_tokens = ( 2000 + 100 ) * (N-1) + 2000
output_tokens = 100

Also note that reasoning contents are not included, following some of the guidelines from openai and anthropic. It is common to discard reasoning in the message history.
Reference: [openai](https://developers.openai.com/api/docs/guides/reasoning#how-reasoning-works) and [anthropic](https://platform.claude.com/docs/en/build-with-claude/extended-thinking#the-context-window-with-extended-thinking)
