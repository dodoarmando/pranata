//! Background jobs kept in PostgreSQL (D-14).
//!
//! A worker claims the oldest due job with `FOR UPDATE SKIP LOCKED` in a short
//! transaction that counts the attempt and leases the job, commits, and only
//! then runs the handler. Two workers never claim the same job, and a job that
//! crashes its worker is already counted and is offered again once the lease
//! ends. A failing job is retried with exponential backoff and ends as
//! `failed` after [`MAX_ATTEMPTS`] attempts.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{AppState, media};

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
    /// Ask the web server to drop its cached public pages.
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

/// Queue a job and return its id. Call it with the transaction that carries
/// the change the job belongs to, so the job exists exactly when the change
/// does.
///
/// # Errors
/// Fails when the insert fails.
pub async fn enqueue(conn: &mut PgConnection, job: Job, run_at: OffsetDateTime) -> Result<i64> {
    let (kind, payload) = job.to_parts()?;
    sqlx::query_scalar!(
        r#"INSERT INTO jobs (kind, payload, run_at) VALUES ($1, $2, $3) RETURNING id AS "id!""#,
        kind,
        payload,
        run_at,
    )
    .fetch_one(conn)
    .await
    .context("enqueueing a job")
}

/// Seconds to wait after the given number of failed attempts: 2, 4, 8, … up to
/// five minutes.
#[must_use]
pub fn backoff_secs(attempts: i32) -> i64 {
    let exponent = attempts.clamp(0, 30).unsigned_abs();
    (1_i64 << exponent).min(MAX_BACKOFF_SECS)
}

/// How long a claimed job stays reserved for its worker. A job that runs
/// longer may be started a second time, so handlers must be idempotent.
pub const LEASE_SECS: f64 = 300.0;

/// A job this worker has claimed and counted.
#[derive(Debug)]
pub struct Claimed {
    pub id: i64,
    /// Attempts including this one.
    pub attempts: i32,
    /// The decoded job, or why the stored row could not be read.
    pub job: Result<Job>,
}

/// Pick the oldest due job that is not under a live lease, count the attempt
/// and lease it. The row stays locked only until the transaction ends, so
/// call it in a short transaction and commit before running anything.
///
/// A job whose lease ran out after its final attempt crashed its worker every
/// time (the completion write of an ordinary failure never got to run). It is
/// not started again: it is marked failed here, with the give-up action of its
/// kind, and the next due job is considered.
///
/// # Errors
/// Fails when a query fails.
pub async fn claim(conn: &mut PgConnection) -> Result<Option<Claimed>> {
    loop {
        let Some(row) = sqlx::query!(
            "SELECT id, kind, payload, attempts FROM jobs
             WHERE done_at IS NULL AND failed_at IS NULL AND run_at <= now()
               AND (locked_until IS NULL OR locked_until <= now())
             ORDER BY run_at, id
             FOR UPDATE SKIP LOCKED
             LIMIT 1"
        )
        .fetch_optional(&mut *conn)
        .await
        .context("looking for a due job")?
        else {
            return Ok(None);
        };
        let job = Job::from_parts(&row.kind, row.payload);

        if row.attempts >= MAX_ATTEMPTS {
            tracing::error!(
                job_id = row.id,
                attempts = row.attempts,
                "job gave up: lease expired after the final attempt"
            );
            sqlx::query!(
                "UPDATE jobs
                 SET failed_at = now(), locked_until = NULL,
                     last_error = 'lease expired after final attempt'
                 WHERE id = $1",
                row.id
            )
            .execute(&mut *conn)
            .await
            .context("giving up on a job")?;
            if let Ok(Job::ProcessImage { asset_id }) = &job {
                media::mark_failed(&mut *conn, *asset_id).await?;
            }
            continue;
        }

        let attempts = sqlx::query_scalar!(
            "UPDATE jobs
             SET attempts = attempts + 1,
                 locked_until = now() + make_interval(secs => $2)
             WHERE id = $1
             RETURNING attempts",
            row.id,
            LEASE_SECS,
        )
        .fetch_one(&mut *conn)
        .await
        .context("leasing a job")?;
        return Ok(Some(Claimed {
            id: row.id,
            attempts,
            job,
        }));
    }
}

/// Run one job; the dispatch point for new job kinds.
async fn handle(state: AppState, job: Job) -> Result<()> {
    match job {
        Job::ProcessImage { asset_id } => media::process_asset(&state, asset_id).await,
        Job::PurgePublicCache => state.purger.purge_all().await,
    }
}

/// Claim one due job and run it. Returns `false` when nothing was due.
///
/// # Errors
/// Fails on database errors. A failing job is not an error here: it is
/// rescheduled or marked failed.
pub async fn claim_and_run(state: &AppState) -> Result<bool> {
    claim_and_run_with(state, handle).await
}

/// [`claim_and_run`] with a chosen handler (tests inject failing ones).
///
/// The claim commits before the handler starts, so no transaction is held
/// while it runs. The handler runs in its own task: a panic becomes a failed
/// attempt instead of ending the worker.
///
/// # Errors
/// Fails on database errors.
pub async fn claim_and_run_with<F, Fut>(state: &AppState, handler: F) -> Result<bool>
where
    F: FnOnce(AppState, Job) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    let mut tx = state.db.begin().await.context("starting a transaction")?;
    let claimed = claim(&mut tx).await?;
    tx.commit().await.context("committing the claim")?;
    let Some(claimed) = claimed else {
        return Ok(false);
    };

    let outcome = match &claimed.job {
        Ok(job) => match tokio::spawn(handler(state.clone(), job.clone())).await {
            Ok(result) => result,
            Err(join) if join.is_panic() => Err(anyhow!("the job handler panicked")),
            Err(join) => Err(anyhow!("the job handler was cancelled: {join}")),
        },
        Err(error) => Err(anyhow!("{error:#}")),
    };

    let mut tx = state.db.begin().await.context("starting a transaction")?;
    // Every write is tied to the attempt this worker claimed: when the lease
    // ran out and another worker took the job over, this result is stale.
    let applied = match outcome {
        Ok(()) => sqlx::query!(
            "UPDATE jobs SET done_at = now(), locked_until = NULL
                 WHERE id = $1 AND attempts = $2 AND done_at IS NULL AND failed_at IS NULL",
            claimed.id,
            claimed.attempts,
        )
        .execute(&mut *tx)
        .await
        .context("marking a job done")?
        .rows_affected(),
        Err(error) => {
            let attempts = claimed.attempts;
            tracing::warn!(job_id = claimed.id, attempts, error = ?error, "job failed");
            let exhausted = attempts >= MAX_ATTEMPTS;
            #[allow(clippy::cast_precision_loss)] // at most 300
            let delay = backoff_secs(attempts) as f64;
            let rows = sqlx::query!(
                "UPDATE jobs
                 SET last_error = $3,
                     locked_until = NULL,
                     run_at = now() + make_interval(secs => $4),
                     failed_at = CASE WHEN $5 THEN now() END
                 WHERE id = $1 AND attempts = $2 AND done_at IS NULL AND failed_at IS NULL",
                claimed.id,
                attempts,
                format!("{error:#}"),
                delay,
                exhausted,
            )
            .execute(&mut *tx)
            .await
            .context("rescheduling a failed job")?
            .rows_affected();
            // Giving up on an image must not leave its asset pending forever.
            if rows > 0
                && exhausted
                && let Ok(Job::ProcessImage { asset_id }) = &claimed.job
            {
                media::mark_failed(&mut tx, *asset_id).await?;
            }
            rows
        }
    };
    if applied == 0 {
        tracing::warn!(
            job_id = claimed.id,
            attempts = claimed.attempts,
            "stale job result discarded"
        );
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
