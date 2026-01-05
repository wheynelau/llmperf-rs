# This is sample file to compare the results, should be used as a reference only.
# uv run postprocess/compare_results.py
# assumes that you have a results folder in the upper directory

import os
import json
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

def load_parquet(file_path: str) -> pd.DataFrame:
    return pd.read_parquet(file_path)


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
    