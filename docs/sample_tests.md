# Sample test

Below is a sample end to end example:

The use case is testing `vllm`, `sglang` and `llamacpp`.

## Commands

Server commands, this test is not meant to be replicated as it only serves as an example. Env and versions are intended not to be published. 

```bash
vllm serve Qwen/Qwen3-30B-A3B-Instruct-2507-FP8
sglang serve --model-path Qwen/Qwen3-30B-A3B-Instruct-2507-FP8
llama-server --host 127.0.0.1 --hf-repo unsloth/Qwen3-30B-A3B-Instruct-2507-GGUF:IQ4_NL
```

These results don't serve as the optimal parameters.

## Summary

The python script and `pyproject.toml` are also meant to be serve as reference, for bare minimum dependencies. Users should create their own scripts for post processing. 

```bash
uv run --directory postprocess/ compare_results.py
```

Result:
<table border="1" class="dataframe">
  <thead>
    <tr style="text-align: right;">
      <th></th>
      <th>filename</th>
      <th>ttft_s_quantiles_p99</th>
      <th>ttft_s_max</th>
      <th>itl_ms_quantiles_p99</th>
      <th>itl_ms_max</th>
      <th>num_completed_requests_per_min</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <th>0</th>
      <td>20260105_051247_Qwen-Qwen3-30B-A3B-Instruct-2507-FP8_8192_1024_summary.json</td>
      <td>4.442264</td>
      <td>4.442264</td>
      <td>13.138111</td>
      <td>167.959586</td>
      <td>35.074260</td>
    </tr>
    <tr>
      <th>1</th>
      <td>20260105_051642_unsloth-Qwen3-30B-A3B-Instruct-2507-GGUF-IQ4_NL_8192_1024_summary.json</td>
      <td>74.874757</td>
      <td>74.874757</td>
      <td>21.016201</td>
      <td>1988.899301</td>
      <td>6.546748</td>
    </tr>
    <tr>
      <th>2</th>
      <td>20260105_050854_Qwen-Qwen3-30B-A3B-Instruct-2507-FP8_8192_1024_summary.json</td>
      <td>1.723397</td>
      <td>1.723397</td>
      <td>12.123924</td>
      <td>1388.530732</td>
      <td>45.640330</td>
    </tr>
  </tbody>
</table>