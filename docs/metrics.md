# Metrics

## How Tokens Are Counted

This benchmark tool gets token counts directly from the API responses:

```rust
// Check if usage is provided, some endpoints will send their usage.  
// This will correctly reflect the tokens that were received, so metrics are more accurate.  
if let Some(usage) = response.usage {
    metrics.number_input_tokens = usage.prompt_tokens;
    metrics.number_output_tokens = usage.completion_tokens;
    metrics.number_total_tokens = usage.total_tokens;
}
```

If the endpoint does not provide, it uses the calculated values, based on the arguments provided.  

## For correctness

If you need to get the exact token count, you can pass in a tokenizer to the tool. 
```
llmperf --tokenizer <hf tokenizer path>
```

Even then, the input tokens may not be the same due to chat templates, but from some testing it was less than 20 tokens off. 

The `output tokens` are <= `max_tokens`, more on this below. 

## What the Metrics Mean

- **Input tokens**: Tokens in your prompt/request
- **Output tokens**: Tokens generated in response to your request
- **Total tokens**: Input + Output tokens

## Per Response Metrics

This may require `arrow` or `fastparquet` to be installed.

Per response metrics are stored in a parquet file for storage efficiency.  

```python
import pandas as pd

df = pd.read_parquet("<path to parquet file>")
df.columns
# Index(['itl_ms_mean', 'itl_ms_stddev', 'itl_ms_vec', 'ttft_s',
#        'end_to_end_latency_s', 'number_input_tokens', 'number_output_tokens',
#        'number_total_tokens', 'prefill_throughput_tps', 'error_msg',
#        'error_code', 'number_errors', 'decode_throughput_tps', 'response',
#        'finish_reason'],
#       dtype='object')

# Note that itl_ms_vec is an array of f64, despite dtype showing object
# The above columns may change
```

## SummaryMetrics

Most metrics are summed and aggregated if they are successful, the only exception is `error_code_frequency` which is a counter of error codes.

For benchmarking performance, the most important metrics are TTFT, ITL and possibly RPM, as these are user facing metrics. 

### Inter Token Latency (ITL)

ITL is the latency between tokens.  

As such each itl_vec always has N - 1 elements, where N is the max tokens.

The ITL is aggregated over all responses. 

For example
  
Response 1 ITL_vec = [100, 200, 300]  
Response 2 ITL_vec = [400, 500, 600]  

The ITL for the entire run would be [100, 200, 300, 400, 500, 600]  

And the metrics are aggregated across the 6 elements.  

### TTFT (Time To First Token)

TTFT calculated as the difference in timing from the POST request to the first token. 

### End to End Latency

End to end latency is the total time from the POST request to the final token. To be precise, the time stops when the response returns with `finish_reason`,   
rather than end of stream. 

### Prefill Throughput Tokens Per Second (TPS)

Prefill Throughput is the number of tokens processed per second during the prefill phase.  

$`\frac{\mathrm{number\_input\_tokens}}{\mathrm{ttft}}`$

This explains why the correct input tokens are needed.  

### Decode Throughput Tokens Per Second (TPS)

Decode Throughput is the number of tokens processed per second during the decode phase.  

$`\frac{\mathrm{number\_output\_tokens}}{\mathrm{final\_time} - \mathrm{decode\_start\_time}}`$


### Finish Reason

This is for confirming that all runs end with the max tokens. Occasionally, due to prompts or model behavior, the run may not end with the max tokens. 

Then you would encounter Finish Reason = stop, rather than Finish Reason = length. For an accurate metric, all runs should end with Finish Reason = length.

However for the current codebase, finish reason stop is included. The variance will occur in RPM and E2E latency. It is left to the user to decide if they want to aggregate with or without the stopped sequences. 

## Conclusion

The role of llmperf is to help with LLM configuration, selection of models should still be done via other evaluations. A faster model does not mean a better model. 
