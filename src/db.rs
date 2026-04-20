use crate::api::models::FinishReason;
use crate::metrics::models::SummaryMetrics;
use anyhow::Result;
use sqlx::PgPool;

pub async fn connect(url: &str) -> Result<PgPool> {
    let pool = PgPool::connect(url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database '{}': {}", url, e))?;
    ensure_table(&pool).await?;
    Ok(pool)
}
// Design choice: Hybrid can help to clean up the columns
// but having it as flat can be easier to deal with like if you are using pandas
// or something similar
pub async fn ensure_table(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS benchmark_results (
            id                             BIGSERIAL PRIMARY KEY,
            timestamp                      BIGINT NOT NULL,
            model                          TEXT NOT NULL,
            num_concurrent_requests        BIGINT NOT NULL,
            mean_input_tokens              BIGINT NOT NULL,
            mean_output_tokens             BIGINT NOT NULL,
            args                           JSONB NOT NULL,
            finish_reason_stop             BIGINT NOT NULL DEFAULT 0,
            finish_reason_length           BIGINT NOT NULL DEFAULT 0,
            error_code_frequency           JSONB NOT NULL,
            number_errors                  BIGINT NOT NULL,
            num_requests_started           BIGINT NOT NULL,
            num_completed_requests         BIGINT NOT NULL,
            num_completed_requests_per_min DOUBLE PRECISION NOT NULL,
            error_rate                     DOUBLE PRECISION NOT NULL,
            itl_ms_mean                    DOUBLE PRECISION NOT NULL,
            itl_ms_median                  DOUBLE PRECISION NOT NULL,
            itl_ms_stddev                  DOUBLE PRECISION NOT NULL,
            itl_ms_min                     DOUBLE PRECISION NOT NULL,
            itl_ms_max                     DOUBLE PRECISION NOT NULL,
            itl_ms_p25                     DOUBLE PRECISION NOT NULL,
            itl_ms_p50                     DOUBLE PRECISION NOT NULL,
            itl_ms_p75                     DOUBLE PRECISION NOT NULL,
            itl_ms_p90                     DOUBLE PRECISION NOT NULL,
            itl_ms_p95                     DOUBLE PRECISION NOT NULL,
            itl_ms_p99                     DOUBLE PRECISION NOT NULL,
            ttft_s_mean                    DOUBLE PRECISION NOT NULL,
            ttft_s_median                  DOUBLE PRECISION NOT NULL,
            ttft_s_stddev                  DOUBLE PRECISION NOT NULL,
            ttft_s_min                     DOUBLE PRECISION NOT NULL,
            ttft_s_max                     DOUBLE PRECISION NOT NULL,
            ttft_s_p25                     DOUBLE PRECISION NOT NULL,
            ttft_s_p50                     DOUBLE PRECISION NOT NULL,
            ttft_s_p75                     DOUBLE PRECISION NOT NULL,
            ttft_s_p90                     DOUBLE PRECISION NOT NULL,
            ttft_s_p95                     DOUBLE PRECISION NOT NULL,
            ttft_s_p99                     DOUBLE PRECISION NOT NULL,
            e2e_latency_s_mean             DOUBLE PRECISION NOT NULL,
            e2e_latency_s_median           DOUBLE PRECISION NOT NULL,
            e2e_latency_s_stddev           DOUBLE PRECISION NOT NULL,
            e2e_latency_s_min              DOUBLE PRECISION NOT NULL,
            e2e_latency_s_max              DOUBLE PRECISION NOT NULL,
            e2e_latency_s_p25              DOUBLE PRECISION NOT NULL,
            e2e_latency_s_p50              DOUBLE PRECISION NOT NULL,
            e2e_latency_s_p75              DOUBLE PRECISION NOT NULL,
            e2e_latency_s_p90             DOUBLE PRECISION NOT NULL,
            e2e_latency_s_p95             DOUBLE PRECISION NOT NULL,
            e2e_latency_s_p99             DOUBLE PRECISION NOT NULL,
            prefill_tps_mean               DOUBLE PRECISION NOT NULL,
            prefill_tps_median             DOUBLE PRECISION NOT NULL,
            prefill_tps_stddev             DOUBLE PRECISION NOT NULL,
            prefill_tps_min                DOUBLE PRECISION NOT NULL,
            prefill_tps_max                DOUBLE PRECISION NOT NULL,
            prefill_tps_p25                DOUBLE PRECISION NOT NULL,
            prefill_tps_p50                DOUBLE PRECISION NOT NULL,
            prefill_tps_p75                DOUBLE PRECISION NOT NULL,
            prefill_tps_p90                DOUBLE PRECISION NOT NULL,
            prefill_tps_p95                DOUBLE PRECISION NOT NULL,
            prefill_tps_p99                DOUBLE PRECISION NOT NULL,
            decode_tps_mean                DOUBLE PRECISION NOT NULL,
            decode_tps_median              DOUBLE PRECISION NOT NULL,
            decode_tps_stddev              DOUBLE PRECISION NOT NULL,
            decode_tps_min                 DOUBLE PRECISION NOT NULL,
            decode_tps_max                 DOUBLE PRECISION NOT NULL,
            decode_tps_p25                 DOUBLE PRECISION NOT NULL,
            decode_tps_p50                 DOUBLE PRECISION NOT NULL,
            decode_tps_p75                 DOUBLE PRECISION NOT NULL,
            decode_tps_p90                 DOUBLE PRECISION NOT NULL,
            decode_tps_p95                 DOUBLE PRECISION NOT NULL,
            decode_tps_p99                 DOUBLE PRECISION NOT NULL,
            input_tokens_mean              DOUBLE PRECISION NOT NULL,
            input_tokens_median            DOUBLE PRECISION NOT NULL,
            input_tokens_stddev            DOUBLE PRECISION NOT NULL,
            input_tokens_min               BIGINT NOT NULL,
            input_tokens_max               BIGINT NOT NULL,
            input_tokens_p25               DOUBLE PRECISION NOT NULL,
            input_tokens_p50               DOUBLE PRECISION NOT NULL,
            input_tokens_p75               DOUBLE PRECISION NOT NULL,
            input_tokens_p90               DOUBLE PRECISION NOT NULL,
            input_tokens_p95               DOUBLE PRECISION NOT NULL,
            input_tokens_p99               DOUBLE PRECISION NOT NULL,
            output_tokens_mean             DOUBLE PRECISION NOT NULL,
            output_tokens_median           DOUBLE PRECISION NOT NULL,
            output_tokens_stddev           DOUBLE PRECISION NOT NULL,
            output_tokens_min              BIGINT NOT NULL,
            output_tokens_max              BIGINT NOT NULL,
            output_tokens_p25              DOUBLE PRECISION NOT NULL,
            output_tokens_p50              DOUBLE PRECISION NOT NULL,
            output_tokens_p75              DOUBLE PRECISION NOT NULL,
            output_tokens_p90              DOUBLE PRECISION NOT NULL,
            output_tokens_p95              DOUBLE PRECISION NOT NULL,
            output_tokens_p99              DOUBLE PRECISION NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_summary(pool: &PgPool, summary: &SummaryMetrics) -> Result<()> {
    let finish_stop = *summary
        .finish_reasons
        .get(&FinishReason::Stop)
        .unwrap_or(&0) as i64;
    let finish_length = *summary
        .finish_reasons
        .get(&FinishReason::Length)
        .unwrap_or(&0) as i64;
    let args_json = serde_json::to_value(&summary.args)?;
    let error_freq_json = serde_json::to_value(&summary.error_code_frequency)?;

    // LLM generated
    // Proc-macro sees unexpanded declarative macros as single tokens.
    // Nested macro_rules! don't work inside sqlx::query!() -
    // all 91 parameters must be explicit.
    // Would appreciate if anyone has a neater way to do this
    sqlx::query!(
        r#"
        INSERT INTO benchmark_results (
            timestamp, model, num_concurrent_requests,
            mean_input_tokens, mean_output_tokens, args,
            finish_reason_stop, finish_reason_length,
            error_code_frequency, number_errors, num_requests_started,
            num_completed_requests, num_completed_requests_per_min, error_rate,
            itl_ms_mean, itl_ms_median, itl_ms_stddev, itl_ms_min, itl_ms_max,
            itl_ms_p25, itl_ms_p50, itl_ms_p75, itl_ms_p90, itl_ms_p95, itl_ms_p99,
            ttft_s_mean, ttft_s_median, ttft_s_stddev, ttft_s_min, ttft_s_max,
            ttft_s_p25, ttft_s_p50, ttft_s_p75, ttft_s_p90, ttft_s_p95, ttft_s_p99,
            e2e_latency_s_mean, e2e_latency_s_median, e2e_latency_s_stddev,
            e2e_latency_s_min, e2e_latency_s_max,
            e2e_latency_s_p25, e2e_latency_s_p50, e2e_latency_s_p75,
            e2e_latency_s_p90, e2e_latency_s_p95, e2e_latency_s_p99,
            prefill_tps_mean, prefill_tps_median, prefill_tps_stddev,
            prefill_tps_min, prefill_tps_max,
            prefill_tps_p25, prefill_tps_p50, prefill_tps_p75,
            prefill_tps_p90, prefill_tps_p95, prefill_tps_p99,
            decode_tps_mean, decode_tps_median, decode_tps_stddev,
            decode_tps_min, decode_tps_max,
            decode_tps_p25, decode_tps_p50, decode_tps_p75,
            decode_tps_p90, decode_tps_p95, decode_tps_p99,
            input_tokens_mean, input_tokens_median, input_tokens_stddev,
            input_tokens_min, input_tokens_max,
            input_tokens_p25, input_tokens_p50, input_tokens_p75,
            input_tokens_p90, input_tokens_p95, input_tokens_p99,
            output_tokens_mean, output_tokens_median, output_tokens_stddev,
            output_tokens_min, output_tokens_max,
            output_tokens_p25, output_tokens_p50, output_tokens_p75,
            output_tokens_p90, output_tokens_p95, output_tokens_p99
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
            $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
            $41, $42, $43, $44, $45, $46, $47, $48, $49, $50,
            $51, $52, $53, $54, $55, $56, $57, $58, $59, $60,
            $61, $62, $63, $64, $65, $66, $67, $68, $69, $70,
            $71, $72, $73, $74, $75, $76, $77, $78, $79, $80,
            $81, $82, $83, $84, $85, $86, $87, $88, $89, $90,
            $91
        )
        "#,
        summary.timestamp as i64,                     // $1
        &summary.args.model,                          // $2
        summary.args.num_concurrent_requests as i64,  // $3
        summary.args.mean_input_tokens as i64,        // $4
        summary.args.mean_output_tokens as i64,       // $5
        args_json,                                    // $6
        finish_stop,                                  // $7
        finish_length,                                // $8
        error_freq_json,                              // $9
        summary.number_errors as i64,                 // $10
        summary.num_requests_started as i64,          // $11
        summary.num_completed_requests as i64,        // $12
        summary.num_completed_requests_per_min,       // $13
        summary.error_rate,                           // $14
        summary.itl_ms.mean,                          // $15
        summary.itl_ms.median,                        // $16
        summary.itl_ms.stddev,                        // $17
        summary.itl_ms.min,                           // $18
        summary.itl_ms.max,                           // $19
        summary.itl_ms.quantiles_p25,                 // $20
        summary.itl_ms.quantiles_p50,                 // $21
        summary.itl_ms.quantiles_p75,                 // $22
        summary.itl_ms.quantiles_p90,                 // $23
        summary.itl_ms.quantiles_p95,                 // $24
        summary.itl_ms.quantiles_p99,                 // $25
        summary.ttft_s.mean,                          // $26
        summary.ttft_s.median,                        // $27
        summary.ttft_s.stddev,                        // $28
        summary.ttft_s.min,                           // $29
        summary.ttft_s.max,                           // $30
        summary.ttft_s.quantiles_p25,                 // $31
        summary.ttft_s.quantiles_p50,                 // $32
        summary.ttft_s.quantiles_p75,                 // $33
        summary.ttft_s.quantiles_p90,                 // $34
        summary.ttft_s.quantiles_p95,                 // $35
        summary.ttft_s.quantiles_p99,                 // $36
        summary.end_to_end_latency_s.mean,            // $37
        summary.end_to_end_latency_s.median,          // $38
        summary.end_to_end_latency_s.stddev,          // $39
        summary.end_to_end_latency_s.min,             // $40
        summary.end_to_end_latency_s.max,             // $41
        summary.end_to_end_latency_s.quantiles_p25,   // $42
        summary.end_to_end_latency_s.quantiles_p50,   // $43
        summary.end_to_end_latency_s.quantiles_p75,   // $44
        summary.end_to_end_latency_s.quantiles_p90,   // $45
        summary.end_to_end_latency_s.quantiles_p95,   // $46
        summary.end_to_end_latency_s.quantiles_p99,   // $47
        summary.prefill_throughput_tps.mean,          // $48
        summary.prefill_throughput_tps.median,        // $49
        summary.prefill_throughput_tps.stddev,        // $50
        summary.prefill_throughput_tps.min,           // $51
        summary.prefill_throughput_tps.max,           // $52
        summary.prefill_throughput_tps.quantiles_p25, // $53
        summary.prefill_throughput_tps.quantiles_p50, // $54
        summary.prefill_throughput_tps.quantiles_p75, // $55
        summary.prefill_throughput_tps.quantiles_p90, // $56
        summary.prefill_throughput_tps.quantiles_p95, // $57
        summary.prefill_throughput_tps.quantiles_p99, // $58
        summary.decode_throughput_tps.mean,           // $59
        summary.decode_throughput_tps.median,         // $60
        summary.decode_throughput_tps.stddev,         // $61
        summary.decode_throughput_tps.min,            // $62
        summary.decode_throughput_tps.max,            // $63
        summary.decode_throughput_tps.quantiles_p25,  // $64
        summary.decode_throughput_tps.quantiles_p50,  // $65
        summary.decode_throughput_tps.quantiles_p75,  // $66
        summary.decode_throughput_tps.quantiles_p90,  // $67
        summary.decode_throughput_tps.quantiles_p95,  // $68
        summary.decode_throughput_tps.quantiles_p99,  // $69
        summary.input_tokens.mean,                    // $70
        summary.input_tokens.median,                  // $71
        summary.input_tokens.stddev,                  // $72
        summary.input_tokens.min as i64,              // $73
        summary.input_tokens.max as i64,              // $74
        summary.input_tokens.quantiles_p25,           // $75
        summary.input_tokens.quantiles_p50,           // $76
        summary.input_tokens.quantiles_p75,           // $77
        summary.input_tokens.quantiles_p90,           // $78
        summary.input_tokens.quantiles_p95,           // $79
        summary.input_tokens.quantiles_p99,           // $80
        summary.output_tokens.mean,                   // $81
        summary.output_tokens.median,                 // $82
        summary.output_tokens.stddev,                 // $83
        summary.output_tokens.min as i64,             // $84
        summary.output_tokens.max as i64,             // $85
        summary.output_tokens.quantiles_p25,          // $86
        summary.output_tokens.quantiles_p50,          // $87
        summary.output_tokens.quantiles_p75,          // $88
        summary.output_tokens.quantiles_p90,          // $89
        summary.output_tokens.quantiles_p95,          // $90
        summary.output_tokens.quantiles_p99,          // $91
    )
    .execute(pool)
    .await?;

    Ok(())
}
