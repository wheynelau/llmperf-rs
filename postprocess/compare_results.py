# /// script
# requires-python = ">=3.14"
# dependencies = [
#     "pandas",
# ]
# ///

# need 3.14 due to zstd support, 3.12 is an option but not provided here
# Its best to run this with uv, because it locks the version
# `uv run compare_results.py`

import json
import os

import pandas as pd


def load_results(results_dir) -> list[dict]:
    results = []
    for filename in os.listdir(results_dir):
        if filename.endswith(".json"):
            with open(os.path.join(results_dir, filename), "r") as f:
                data = json.load(f)
                data["filename"] = filename
                results.append(data)
    return results


def load_jsonl_zst(file_path: str) -> pd.DataFrame:
    from compression import zstd

    with open(file_path, "rb") as f:
        decompressed = zstd.decompress(f.read())
    lines = decompressed.decode("utf-8").strip().split("\n")
    records = [json.loads(line) for line in lines]
    return pd.DataFrame(records)


if __name__ == "__main__":
    results_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), "results")
    results = load_results(results_dir)

    # Convert to DataFrame
    df = pd.DataFrame(results)

    print(df.columns)

    # your column of interests
    columns_of_interest = [
        "filename",
        "ttft_s_quantiles_p99",
        "ttft_s_max",
        "itl_ms_quantiles_p99",
        "itl_ms_max",
        "num_completed_requests_per_min",
    ]

    comparison_df = df[columns_of_interest]
    print(comparison_df)
