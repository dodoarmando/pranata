mod common;

use std::collections::HashSet;

use common::test_state;
use pranata::jobs::{self, Job};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

async fn enqueue_now(pool: &PgPool, job: Job) {
    let mut conn = pool.acquire().await.unwrap();
    jobs::enqueue(&mut conn, job, OffsetDateTime::now_utc())
        .await
        .unwrap();
}

#[sqlx::test]
async fn job_retries_with_backoff(pool: PgPool) {
    let state = test_state(pool.clone());
    // No such asset, so the handler fails every time.
    enqueue_now(
        &pool,
        Job::ProcessImage {
            asset_id: Uuid::new_v4(),
        },
    )
    .await;

    for expected in 1..=3_i32 {
        // Make the job due again, as if the backoff had elapsed.
        sqlx::query!("UPDATE jobs SET run_at = now()")
            .execute(&pool)
            .await
            .unwrap();
        assert!(jobs::claim_and_run(&state).await.unwrap());
        let row = sqlx::query!(
            r#"SELECT attempts, last_error, done_at, failed_at,
                      EXTRACT(EPOCH FROM (run_at - now()))::float8 AS "delay!"
               FROM jobs"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.attempts, expected);
        assert!(row.last_error.is_some());
        assert!(row.done_at.is_none() && row.failed_at.is_none());
        let want = f64::from(2_i32.pow(expected.unsigned_abs()));
        assert!(
            (row.delay - want).abs() < 1.5,
            "attempt {expected}: delay {} should be about {want}",
            row.delay
        );
    }

    // Not due yet, so nothing is claimed.
    assert!(!jobs::claim_and_run(&state).await.unwrap());
}

#[sqlx::test]
async fn backoff_is_capped_and_attempts_are_bounded(pool: PgPool) {
    assert_eq!(jobs::backoff_secs(1), 2);
    assert_eq!(jobs::backoff_secs(8), 256);
    assert_eq!(jobs::backoff_secs(9), 300);
    assert_eq!(jobs::backoff_secs(40), 300);

    let state = test_state(pool.clone());
    enqueue_now(
        &pool,
        Job::ProcessImage {
            asset_id: Uuid::new_v4(),
        },
    )
    .await;
    for _ in 0..jobs::MAX_ATTEMPTS {
        sqlx::query!("UPDATE jobs SET run_at = now()")
            .execute(&pool)
            .await
            .unwrap();
        assert!(jobs::claim_and_run(&state).await.unwrap());
    }
    let row = sqlx::query!("SELECT attempts, failed_at FROM jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.attempts, 8);
    assert!(row.failed_at.is_some());
    sqlx::query!("UPDATE jobs SET run_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!jobs::claim_and_run(&state).await.unwrap());
}

#[sqlx::test]
async fn two_workers_never_claim_the_same_job(pool: PgPool) {
    // A claim holds the row lock, so a second claimant skips that job.
    enqueue_now(&pool, Job::PurgePublicCache).await;
    let mut first = pool.begin().await.unwrap();
    let mut second = pool.begin().await.unwrap();
    let claimed = jobs::claim(&mut first).await.unwrap().expect("first claim");
    assert!(jobs::claim(&mut second).await.unwrap().is_none());
    assert_eq!(claimed.job.unwrap(), Job::PurgePublicCache);
    first.rollback().await.unwrap();
    second.rollback().await.unwrap();

    // Many jobs, two concurrent workers: every job runs exactly once.
    for _ in 0..24 {
        enqueue_now(&pool, Job::PurgePublicCache).await;
    }
    let total = 25;
    let mut workers = Vec::new();
    for _ in 0..2 {
        let state = test_state(pool.clone());
        workers.push(tokio::spawn(async move {
            let mut ran = 0_u32;
            while jobs::claim_and_run(&state).await.unwrap() {
                ran += 1;
            }
            ran
        }));
    }
    let mut ran = 0;
    for worker in workers {
        ran += worker.await.unwrap();
    }
    assert_eq!(ran, total);

    let rows = sqlx::query!("SELECT id, attempts, done_at FROM jobs")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 25);
    assert!(rows.iter().all(|row| row.done_at.is_some()));
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<HashSet<_>>().len(),
        25
    );
}

#[sqlx::test]
async fn worker_runs_due_jobs_and_stops_on_cancel(pool: PgPool) {
    let state = test_state(pool.clone());
    enqueue_now(&pool, Job::PurgePublicCache).await;
    let shutdown = tokio_util::sync::CancellationToken::new();
    let worker = tokio::spawn(jobs::run_worker_every(
        state,
        shutdown.clone(),
        std::time::Duration::from_millis(20),
    ));

    let done = async {
        loop {
            let open =
                sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM jobs WHERE done_at IS NULL"#)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if open == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), done)
        .await
        .expect("the worker picks the job up");

    shutdown.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(5), worker)
        .await
        .expect("the worker stops after cancellation")
        .unwrap();
}

async fn attempts_of(pool: &PgPool) -> i32 {
    sqlx::query_scalar!("SELECT attempts FROM jobs")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn attempts_increment_before_handler_runs(pool: PgPool) {
    let state = test_state(pool.clone());
    enqueue_now(&pool, Job::PurgePublicCache).await;
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(-1));
    let observer = seen.clone();
    let observed_pool = pool.clone();
    assert!(
        jobs::claim_and_run_with(&state, move |_state, _job| async move {
            // The claim is already committed, and no claim transaction is open.
            observer.store(
                attempts_of(&observed_pool).await,
                std::sync::atomic::Ordering::SeqCst,
            );
            Ok(())
        })
        .await
        .unwrap()
    );
    assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[sqlx::test]
async fn expired_lease_is_reclaimed(pool: PgPool) {
    let state = test_state(pool.clone());
    enqueue_now(&pool, Job::PurgePublicCache).await;

    // A worker claims the job and then crashes: the claim commits, nothing
    // else happens.
    let mut tx = pool.begin().await.unwrap();
    assert!(jobs::claim(&mut tx).await.unwrap().is_some());
    tx.commit().await.unwrap();
    assert_eq!(attempts_of(&pool).await, 1, "the crash is already counted");

    // The lease is alive, so nobody else takes the job.
    assert!(!jobs::claim_and_run(&state).await.unwrap());

    sqlx::query!("UPDATE jobs SET locked_until = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(jobs::claim_and_run(&state).await.unwrap());
    let row = sqlx::query!("SELECT attempts, done_at, locked_until FROM jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.attempts, 2);
    assert!(row.done_at.is_some());
    assert!(row.locked_until.is_none());
}

#[sqlx::test]
async fn panicking_handler_does_not_kill_worker(pool: PgPool) {
    let state = test_state(pool.clone());
    enqueue_now(&pool, Job::PurgePublicCache).await;
    let ran = jobs::claim_and_run_with(&state, |_state, _job| async move {
        panic!("poison job");
        #[allow(unreachable_code)]
        Ok(())
    })
    .await
    .unwrap();
    assert!(ran);
    let row = sqlx::query!("SELECT attempts, last_error, done_at, locked_until FROM jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.attempts, 1);
    assert!(row.last_error.unwrap().contains("panicked"));
    assert!(row.done_at.is_none() && row.locked_until.is_none());

    // The worker is still usable: the next due job runs normally.
    enqueue_now(&pool, Job::PurgePublicCache).await;
    assert!(jobs::claim_and_run(&state).await.unwrap());
}
