#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::Duration;
use grove_db::{PgPool, files, files::RecoveryJob as Job};
use sqlx::migrate::Migrate;
use uuid::Uuid;

#[path = "support/lifecycle.rs"]
mod lifecycle;
#[path = "support/lock_wait.rs"]
mod lock_wait;
#[path = "support/time.rs"]
mod time;

const JOBS: [Job; 3] = [Job::Observe, Job::Reclaim, Job::Purge];

async fn seed(pool: &PgPool, job: Job) -> Uuid {
    let file = lifecycle::create_ok(pool, 10).await;
    match job {
        Job::Observe => {}
        Job::Reclaim => {
            sqlx::query("UPDATE leases SET expires_at=grove_time.transaction_now()-interval '1 second' WHERE id=$1")
                .bind(file.lease_id).execute(pool).await.unwrap();
            let candidate = files::expired_pending(pool, 1000)
                .await
                .unwrap()
                .into_iter()
                .find(|c| c.file_id == file.file_id)
                .unwrap();
            assert!(files::finalize_reclaim(pool, &candidate).await.unwrap());
        }
        Job::Purge => {
            assert!(
                files::finalize_commit(pool, file.file_id, "etag")
                    .await
                    .unwrap()
            );
            assert!(matches!(
                files::mark_deleted(pool, "c", file.file_id).await.unwrap(),
                files::DeleteOutcome::Deleted
            ));
        }
    }
    file.file_id
}

async fn candidates(pool: &PgPool, job: Job) -> Vec<Uuid> {
    match job {
        Job::Observe => files::observed_commit_candidates(pool, 20)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
        Job::Reclaim => files::reclaim_cleanup_candidates(pool, 20)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
        Job::Purge => files::purgeable(pool, 20)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.file_id)
            .collect(),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_batches_leave_unattempted_candidates_and_retain_locations(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 10_000).await;
    for job in JOBS {
        let mut ids = Vec::new();
        for _ in 0..21 {
            ids.push(seed(&pool, job).await);
        }
        ids.sort();
        let first_batch = ids.get(..20).unwrap();
        assert_eq!(candidates(&pool, job).await, first_batch);
        for id in first_batch {
            assert!(files::claim_recovery(&pool, *id, job, 30).await.unwrap());
        }
        assert_eq!(candidates(&pool, job).await, ids.get(20..).unwrap());
        let locations: i64 =
            sqlx::query_scalar("SELECT count(*) FROM locations WHERE file_id=ANY($1)")
                .bind(&ids)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(locations, 21);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn retries_are_durable_and_due_exactly_at_the_injected_deadline(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    for job in JOBS {
        time::set_time(&pool, time::base()).await;
        let id = seed(&pool, job).await;
        let (a, b) = tokio::join!(
            files::claim_recovery(&pool, id, job, 30),
            files::claim_recovery(&pool, id, job, 30)
        );
        assert_ne!(a.unwrap(), b.unwrap());
        time::set_time(
            &pool,
            time::base() + Duration::seconds(30) - Duration::microseconds(1),
        )
        .await;
        assert!(candidates(&pool, job).await.is_empty());
        assert!(!files::claim_recovery(&pool, id, job, 30).await.unwrap());
        time::set_time(&pool, time::base() + Duration::seconds(30)).await;
        assert_eq!(candidates(&pool, job).await, [id]);
        let other = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(pool.connect_options().as_ref().clone())
            .await
            .unwrap();
        assert!(files::claim_recovery(&other, id, job, 30).await.unwrap());
        other.close().await;
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn observation_rechecks_lease_state_and_upload_ownership_after_lock_wait(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    for change in ["lease", "file", "upload"] {
        let id = seed(&pool, Job::Observe).await;
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
            files::claim_recovery(&other, id, Job::Observe, 30)
                .await
                .unwrap()
        });
        lock_wait::wait_for_blocker(&pool, blocker).await;
        let sql = match change {
            "lease" => "UPDATE leases SET expires_at=grove_time.wall_now() WHERE file_id=$1",
            "file" => "UPDATE files SET state='reclaimed' WHERE id=$1",
            _ => "INSERT INTO uploads(file_id,protocol,key,multipart) VALUES($1,'s3','key',false)",
        };
        sqlx::query(sql)
            .bind(id)
            .execute(&mut *owner)
            .await
            .unwrap();
        owner.commit().await.unwrap();
        assert!(!waiting.await.unwrap());
        let unchanged: bool =
            sqlx::query_scalar("SELECT recovery_after=created_at FROM files WHERE id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(unchanged);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn retry_delay_uses_wall_time_after_lock_wait(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    let id = seed(&pool, Job::Observe).await;
    assert!(
        files::claim_recovery(&pool, id, Job::Observe, 30)
            .await
            .unwrap()
    );
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
        files::claim_recovery(&other, id, Job::Observe, 30)
            .await
            .unwrap()
    });
    lock_wait::wait_for_blocker(&pool, blocker).await;
    time::set_time(&pool, time::base() + Duration::seconds(30)).await;
    owner.commit().await.unwrap();
    assert!(waiting.await.unwrap());
    let due: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT recovery_after FROM files WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(due, time::base() + Duration::seconds(60));
}

#[sqlx::test(migrations = "./migrations")]
async fn new_cleanup_phases_do_not_inherit_observation_delay(pool: PgPool) {
    time::install_clock(&pool).await;
    lifecycle::wire(&pool, 1000).await;
    for job in [Job::Reclaim, Job::Purge] {
        let id = seed(&pool, Job::Observe).await;
        assert!(
            files::claim_recovery(&pool, id, Job::Observe, 30)
                .await
                .unwrap()
        );
        match job {
            Job::Reclaim => {
                sqlx::query("UPDATE leases SET expires_at=grove_time.transaction_now()-interval '1 second' WHERE file_id=$1")
                    .bind(id).execute(&pool).await.unwrap();
                let candidate = files::expired_pending(&pool, 20).await.unwrap().remove(0);
                assert!(files::finalize_reclaim(&pool, &candidate).await.unwrap());
            }
            Job::Purge => {
                assert!(files::finalize_commit(&pool, id, "etag").await.unwrap());
                files::mark_deleted(&pool, "c", id).await.unwrap();
            }
            Job::Observe => panic!("expected a cleanup fixture"),
        }
        assert_eq!(candidates(&pool, job).await, [id]);
        assert!(files::claim_recovery(&pool, id, job, 30).await.unwrap());
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn purge_finalization_rejects_stale_state_or_location(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    let mut candidate = files::SweepCandidate {
        file_id: file.file_id,
        storage_id: file.storage.id,
        object_key: file.object_key,
        upload_id: None,
        write_lease_id: None,
        multipart: false,
    };
    assert!(!files::finalize_purge(&pool, &candidate).await.unwrap());
    assert!(
        files::finalize_commit(&pool, file.file_id, "etag")
            .await
            .unwrap()
    );
    assert!(!files::finalize_purge(&pool, &candidate).await.unwrap());
    files::mark_deleted(&pool, "c", file.file_id).await.unwrap();
    candidate.object_key.push_str("-stale");
    assert!(!files::finalize_purge(&pool, &candidate).await.unwrap());
    let current = files::purgeable(&pool, 20).await.unwrap().remove(0);
    assert!(files::finalize_purge(&pool, &current).await.unwrap());
    assert!(
        !files::claim_recovery(&pool, file.file_id, Job::Purge, 30)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = false)]
async fn upgrade_preserves_file_states_locations_and_leases(pool: PgPool) {
    {
        let mut connection = pool.acquire().await.unwrap();
        connection.ensure_migrations_table().await.unwrap();
        for migration in sqlx::migrate!("./migrations")
            .iter()
            .filter(|m| m.version <= 7)
        {
            connection.apply(migration).await.unwrap();
        }
    }
    lifecycle::wire(&pool, 1000).await;
    // Pre-upgrade fixtures cannot use the new transition queries.
    for state in ["pending", "active", "reclaimed", "deleted"] {
        let file = lifecycle::create_ok(&pool, 10).await;
        sqlx::query("UPDATE files SET state=$2,committed_at=CASE WHEN $2 IN ('active','deleted') THEN now() END,deleted_at=CASE WHEN $2='deleted' THEN now() END WHERE id=$1")
            .bind(file.file_id).bind(state).execute(&pool).await.unwrap();
    }
    let before: String =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(f) ORDER BY id)::text FROM files f")
            .fetch_one(&pool)
            .await
            .unwrap();
    let locations: String =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(l) ORDER BY file_id)::text FROM locations l")
            .fetch_one(&pool)
            .await
            .unwrap();
    let leases: String =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(le) ORDER BY id)::text FROM leases le")
            .fetch_one(&pool)
            .await
            .unwrap();
    grove_db::migrate(&pool).await.unwrap();
    let after: String = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(f)-'recovery_after' ORDER BY id)::text FROM files f",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, before);
    let after: String =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(l) ORDER BY file_id)::text FROM locations l")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, locations);
    let after: String =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(le) ORDER BY id)::text FROM leases le")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, leases);
    let due: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM files WHERE recovery_after<=grove_time.transaction_now()",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(due, 4);
}
