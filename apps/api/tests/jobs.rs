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
