# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "psycopg[binary]",
#     "pandas",
# ]
# ///

"""
Query and analyze benchmark results from PostgreSQL database.

Usage:
    # Set database URL
    export DB_URL="postgresql://user:pass@host:5432/dbname"

    # Run script
    uv run from_db.py
"""

import os

import pandas as pd
import psycopg


def get_db_url() -> str:
    """Get database URL from environment."""
    if db_url := os.getenv("DB_URL"):
        return db_url

    raise ValueError("DB_URL environment variable not set")


def connect_to_db() -> psycopg.Connection:
    """Connect to PostgreSQL database."""
    db_url = get_db_url()

    print(f"Connecting to: {db_url.split('@')[1] if '@' in db_url else db_url}")
    return psycopg.connect(db_url)


def query_latest_results(conn: psycopg.Connection, limit: int = 20) -> pd.DataFrame:
    """Query latest benchmark results from database."""
    query = "SELECT * FROM benchmark_results ORDER BY timestamp DESC LIMIT %s"
    df = pd.read_sql(query, conn, params=(limit,))

    # Convert timestamp from Unix epoch to datetime
    df["timestamp"] = pd.to_datetime(df["timestamp"], unit="s")

    return df


def print_summary(df: pd.DataFrame):
    """Print summary statistics of benchmark results."""
    # Group by model
    for model_name, group in df.groupby("model"):
        print(f"Model: {model_name}")
        print(f"  Runs: {len(group)}")
        print(f"  Latest: {group['timestamp'].max()}")
        print(f"  Max TTFT (p95): {group['ttft_s_p95'].max():.3f}s")
        print(f"  Max ITL (p95): {group['itl_ms_p95'].max():.3f}ms")
        print()


def main():
    """Main entry point."""
    conn = connect_to_db()
    df = query_latest_results(conn, limit=20)

    print_summary(df)

    conn.close()


if __name__ == "__main__":
    main()
