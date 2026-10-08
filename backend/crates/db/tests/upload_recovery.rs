#![allow(clippy::unwrap_used, clippy::panic)]

use chrono::Duration;
use grove_db::{
    PgPool, files, s3_registry as s3,
    upload_recovery::{self, Job},
};
use sqlx::migrate::Migrate;
use uuid::Uuid;

#[path = "support/lifecycle.rs"]
mod lifecycle;
#[path = "support/lock_wait.rs"]
#[allow(clippy::expect_used)]
mod lock_wait;
#[path = "support/time.rs"]
mod time;

const JOBS: [Job; 4] = [
    Job::NativeComplete,
    Job::NativeCleanup,
    Job::S3Complete,
    Job::S3Cleanup,
];

async fn seed(pool: &PgPool, job: Job) -> Uuid {
    let files::CreateOutcome::Created(file) = files::create(
        pool,
        files::CreateSpec {
            part_size: Some(5),
            ..lifecycle::spec(10)
        },
    )
    .await
    .unwrap() else {
        panic!("fixture client missing")
    };
    let (protocol, state) = match job {
        Job::NativeComplete => ("native", "completing"),
        Job::NativeCleanup => ("native", "cleaning"),
        Job::S3Complete => ("s3", "completing"),
        Job::S3Cleanup => ("s3", "cleaning"),
    };
    sqlx::query(
        "INSERT INTO uploads(file_id,protocol,key,multipart,state,expected_size,expected_etag)
        VALUES($1,$2,CASE WHEN $2='s3' THEN 'key' END,true,$3,
            CASE WHEN $2='s3' AND $3='completing' THEN 10 END,
            CASE WHEN $2='native' OR $3='completing' THEN 'etag' END)",
    )
    .bind(file.file_id)
    .bind(protocol)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE leases SET expires_at=grove_time.transaction_now()-interval '1 second' WHERE id=$1",
    )
    .bind(file.lease_id)
    .execute(pool)
    .await
    .unwrap();
    file.file_id
}

async fn candidates(pool: &PgPool, job: Job, limit: i64) -> Vec<Uuid> {
    match job {
        Job::NativeComplete => files::completion_candidates(pool, limit)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
        Job::NativeCleanup => files::completion_cleanup_candidates(pool, limit)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
        Job::S3Complete => s3::completion_candidates(pool, limit)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
        Job::S3Cleanup => s3::cleanup_candidates(pool, limit)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_failed_batch_does_not_starve_unattempted_uploads(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 10_000).await;
    for job in JOBS {
        let mut ids = Vec::new();
        for _ in 0..21 {
            ids.push(seed(&pool, job).await);
        }
        ids.sort();
        let first_batch = ids.get(..20).unwrap();
        assert_eq!(candidates(&pool, job, 20).await, first_batch);
        for id in first_batch {
            assert!(upload_recovery::claim(&pool, *id, job, 30).await.unwrap());
        }
        // Failed/abandoned attempts keep their owner but leave the next batch.
        assert_eq!(candidates(&pool, job, 20).await, ids.get(20..).unwrap());
        let unchanged: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM uploads WHERE file_id=ANY($1) AND updated_at=created_at",
        )
        .bind(&ids)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unchanged, 21);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn retries_become_eligible_exactly_at_the_injected_deadline(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    for job in JOBS {
        time::set_time(&pool, time::base()).await;
        let id = seed(&pool, job).await;
        assert!(upload_recovery::claim(&pool, id, job, 30).await.unwrap());
        time::set_time(
            &pool,
            time::base() + Duration::seconds(30) - Duration::microseconds(1),
        )
        .await;
        assert!(candidates(&pool, job, 20).await.is_empty());
        assert!(!upload_recovery::claim(&pool, id, job, 30).await.unwrap());
        time::set_time(&pool, time::base() + Duration::seconds(30)).await;
        assert_eq!(candidates(&pool, job, 20).await, [id]);
        // Another connection observes the persisted schedule, not process-local state.
        let other = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(pool.connect_options().as_ref().clone())
            .await
            .unwrap();
        assert!(upload_recovery::claim(&other, id, job, 30).await.unwrap());
        other.close().await;
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_recovery_attempts_have_one_winner(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let id = seed(&pool, Job::S3Complete).await;
    let (a, b) = tokio::join!(
        upload_recovery::claim(&pool, id, Job::S3Complete, 30),
        upload_recovery::claim(&pool, id, Job::S3Complete, 30)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    assert!(
        !upload_recovery::claim(&pool, id, Job::NativeComplete, 30)
            .await
            .unwrap()
    );
    assert!(
        !upload_recovery::claim(&pool, id, Job::S3Cleanup, 30)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn renewed_lease_and_changed_owner_are_rechecked_after_lock_wait(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 100).await;
    for change in ["renew", "clean"] {
        let id = seed(&pool, Job::S3Complete).await;
        let mut owner = pool.begin().await.unwrap();
        let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *owner)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
            .bind(id)
            .execute(&mut *owner)
            .await
            .unwrap();
        let other = pool.clone();
        let waiting = tokio::spawn(async move {
            upload_recovery::claim(&other, id, Job::S3Complete, 30)
                .await
                .unwrap()
        });
        lock_wait::wait_for_blocker(&pool, blocker).await;
        if change == "renew" {
            sqlx::query("UPDATE leases SET expires_at=grove_time.wall_now()+interval '1 hour' WHERE file_id=$1").bind(id).execute(&mut *owner).await.unwrap();
        } else {
            sqlx::query("UPDATE uploads SET state='cleaning',expected_size=NULL,expected_etag=NULL WHERE file_id=$1").bind(id).execute(&mut *owner).await.unwrap();
        }
        owner.commit().await.unwrap();
        assert!(!waiting.await.unwrap());
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn claim_uses_wall_time_after_a_file_lock_wait(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 100).await;
    let id = seed(&pool, Job::S3Complete).await;
    sqlx::query("UPDATE uploads SET recovery_after=grove_time.transaction_now()+interval '30 seconds' WHERE file_id=$1").bind(id).execute(&pool).await.unwrap();
    let mut owner = pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *owner)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *owner)
        .await
        .unwrap();
    let other = pool.clone();
    let waiting = tokio::spawn(async move {
        upload_recovery::claim(&other, id, Job::S3Complete, 30)
            .await
            .unwrap()
    });
    lock_wait::wait_for_blocker(&pool, blocker).await;
    time::set_time(&pool, time::base() + Duration::seconds(30)).await;
    owner.commit().await.unwrap();
    assert!(waiting.await.unwrap());
    let due: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT recovery_after FROM uploads WHERE file_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(due, time::base() + Duration::seconds(60));
}

#[sqlx::test(migrations = "./migrations")]
async fn moving_to_cleanup_is_immediately_eligible(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 100).await;
    for (job, cleanup) in [
        (Job::NativeComplete, Job::NativeCleanup),
        (Job::S3Complete, Job::S3Cleanup),
    ] {
        let id = seed(&pool, job).await;
        assert!(upload_recovery::claim(&pool, id, job, 30).await.unwrap());
        match job {
            Job::NativeComplete => {
                assert!(files::claim_cleanup(&pool, id).await.unwrap());
            }
            Job::S3Complete => {
                assert!(s3::mark_completion_aborting(&pool, id).await.unwrap());
            }
            _ => panic!("expected completion job"),
        }
        assert_eq!(candidates(&pool, cleanup, 20).await, [id]);
        assert!(
            upload_recovery::claim(&pool, id, cleanup, 30)
                .await
                .unwrap()
        );
    }
}

#[sqlx::test(migrations = false)]
async fn upgrade_preserves_existing_completion_and_cleanup_intents(pool: PgPool) {
    {
        let mut connection = pool.acquire().await.unwrap();
        connection.ensure_migrations_table().await.unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version <= 6)
        {
            connection.apply(migration).await.unwrap();
        }
    }
    lifecycle::wire(&pool, 100).await;
    let mut before = Vec::new();
    for job in JOBS {
        let id = seed(&pool, job).await;
        let row: String =
            sqlx::query_scalar("SELECT to_jsonb(u)::text FROM uploads u WHERE file_id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        before.push((id, row));
    }
    grove_db::migrate(&pool).await.unwrap();
    for (id, row) in before {
        let (after, due): (String, bool) = sqlx::query_as("SELECT (to_jsonb(u)-'recovery_after')::text,recovery_after<=grove_time.transaction_now() FROM uploads u WHERE file_id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(after, row);
        assert!(due);
    }
}
