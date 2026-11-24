//! Background jobs kept in PostgreSQL (D-14).
//!
//! A worker claims the oldest due job with `FOR UPDATE SKIP LOCKED` inside a
//! transaction and runs it before committing, so the row lock is the claim:
//! two workers never run the same job, and a worker that dies mid-job just
//! releases it. A failing job is retried with exponential backoff and ends as
//! `failed` after [`MAX_ATTEMPTS`] attempts.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::AppState;

/// Attempts before a job is given up on.
pub const MAX_ATTEMPTS: i32 = 8;

/// Longest wait between two attempts, in seconds.
const MAX_BACKOFF_SECS: i64 = 300;

/// How long an idle worker waits before it looks for work again.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Work a worker can be asked to do. The variant name is stored in `jobs.kind`
/// and the fields in `jobs.payload`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum Job {
    /// Derive the metadata-free original and the WebP variants of an upload.
    ProcessImage { asset_id: Uuid },
    /// Drop cached public pages. Does nothing until the purger is wired in.
    PurgePublicCache,
}

impl Job {
    fn to_parts(&self) -> Result<(String, Value)> {
        let Value::Object(mut object) =
            serde_json::to_value(self).context("serializing the job")?
        else {
            return Err(anyhow!("a job serializes to an object"));
        };
        let Some(Value::String(kind)) = object.remove("kind") else {
            return Err(anyhow!("a job has a kind"));
        };
        Ok((kind, object.remove("payload").unwrap_or_else(|| json!({}))))
    }

    fn from_parts(kind: &str, payload: Value) -> Result<Self> {
        // A job without fields is stored with an empty payload and read back
        // without a `payload` key.
        let value = match payload {
            Value::Object(fields) if fields.is_empty() => json!({ "kind": kind }),
            payload => json!({ "kind": kind, "payload": payload }),
        };
        serde_json::from_value(value).with_context(|| format!("reading a `{kind}` job"))
    }
}

/// Queue a job. Call it with the transaction that carries the change the job
/// belongs to, so the job exists exactly when the change does.
///
/// # Errors
/// Fails when the insert fails.
pub async fn enqueue(conn: &mut PgConnection, job: Job, run_at: OffsetDateTime) -> Result<()> {
    let (kind, payload) = job.to_parts()?;
    sqlx::query!(
        "INSERT INTO jobs (kind, payload, run_at) VALUES ($1, $2, $3)",
        kind,
        payload,
        run_at,
    )
    .execute(conn)
    .await
    .context("enqueueing a job")?;
    Ok(())
}

/// Seconds to wait after the given number of failed attempts: 2, 4, 8, … up to
/// five minutes.
#[must_use]
pub fn backoff_secs(attempts: i32) -> i64 {
    let exponent = attempts.clamp(0, 30).unsigned_abs();
    (1_i64 << exponent).min(MAX_BACKOFF_SECS)
}

/// A job whose row this transaction holds locked.
#[derive(Debug)]
pub struct Claimed {
    pub id: i64,
    /// Failed attempts so far.
    pub attempts: i32,
    /// The decoded job, or why the stored row could not be read.
    pub job: Result<Job>,
}

/// Lock the oldest due job, skipping jobs other workers hold. The lock lasts
/// until the transaction ends.
///
/// # Errors
/// Fails when the query fails.
pub async fn claim(conn: &mut PgConnection) -> Result<Option<Claimed>> {
    let row = sqlx::query!(
        "SELECT id, kind, payload, attempts FROM jobs
         WHERE done_at IS NULL AND failed_at IS NULL AND run_at <= now()
         ORDER BY run_at, id
         FOR UPDATE SKIP LOCKED
         LIMIT 1"
    )
    .fetch_optional(conn)
    .await
    .context("claiming a job")?;
    Ok(row.map(|row| Claimed {
        id: row.id,
        attempts: row.attempts,
        job: Job::from_parts(&row.kind, row.payload),
    }))
}

/// Run one job; the dispatch point for new job kinds.
#[allow(clippy::unused_async)] // the image handler that awaits arrives with the media module
async fn handle(_state: &AppState, job: Job) -> Result<()> {
    match job {
        Job::ProcessImage { .. } => Err(anyhow!("image processing is not available yet")),
        Job::PurgePublicCache => {
            tracing::info!("public cache purge requested; no purger is wired in yet");
            Ok(())
        }
    }
}

/// Claim one due job and run it. Returns `false` when nothing was due.
///
/// # Errors
/// Fails on database errors. A failing job is not an error here: it is
/// rescheduled or marked failed.
pub async fn claim_and_run(state: &AppState) -> Result<bool> {
    let mut tx = state.db.begin().await.context("starting a transaction")?;
    let Some(claimed) = claim(&mut tx).await? else {
        return Ok(false);
    };
    let outcome = match claimed.job {
        Ok(job) => handle(state, job).await,
        Err(error) => Err(error),
    };
    match outcome {
        Ok(()) => {
            sqlx::query!("UPDATE jobs SET done_at = now() WHERE id = $1", claimed.id)
                .execute(&mut *tx)
                .await
                .context("marking a job done")?;
        }
        Err(error) => {
            let attempts = claimed.attempts + 1;
            tracing::warn!(job_id = claimed.id, attempts, error = ?error, "job failed");
            let exhausted = attempts >= MAX_ATTEMPTS;
            #[allow(clippy::cast_precision_loss)] // at most 300
            let delay = backoff_secs(attempts) as f64;
            sqlx::query!(
                "UPDATE jobs
                 SET attempts = $2,
                     last_error = $3,
                     run_at = now() + make_interval(secs => $4),
                     failed_at = CASE WHEN $5 THEN now() END
                 WHERE id = $1",
                claimed.id,
                attempts,
                format!("{error:#}"),
                delay,
                exhausted,
            )
            .execute(&mut *tx)
            .await
            .context("rescheduling a failed job")?;
        }
    }
    tx.commit().await.context("committing the job result")?;
    Ok(true)
}

/// Run jobs until `shutdown` is cancelled, looking for work every
/// [`POLL_INTERVAL`] when idle.
pub async fn run_worker(state: AppState, shutdown: CancellationToken) {
    run_worker_every(state, shutdown, POLL_INTERVAL).await;
}

/// [`run_worker`] with a chosen idle interval (tests use a short one).
pub async fn run_worker_every(state: AppState, shutdown: CancellationToken, interval: Duration) {
    tracing::info!("job worker started");
    while !shutdown.is_cancelled() {
        let idle = match claim_and_run(&state).await {
            Ok(ran) => !ran,
            Err(error) => {
                tracing::error!(error = ?error, "job worker error");
                true
            }
        };
        if idle {
            tokio::select! {
                () = shutdown.cancelled() => {}
                () = tokio::time::sleep(interval) => {}
            }
        }
    }
    tracing::info!("job worker stopped");
}
