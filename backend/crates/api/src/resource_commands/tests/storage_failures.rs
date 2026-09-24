use super::storages::Root;
use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn storage_http_audit_failure_rolls_back_every_mutation(pool: PgPool) {
    let token = owner(&pool).await;
    let root = Root::new();
    assert_eq!(
        call(&pool, &token, "storage.create", root.input("local", 100))
            .await
            .status(),
        StatusCode::OK
    );
    sqlx::raw_sql("CREATE FUNCTION management.reject_storage_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-detail'; END $$;
        CREATE TRIGGER reject_storage_audit BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_storage_audit();")
        .execute(&pool).await.unwrap();
    for (name, input) in [
        ("storage.create", root.input("new", 1)),
        ("storage.replace", root.input("local", 200)),
        ("storage.delete", json!({"id":"local"})),
    ] {
        let response = call(&pool, &token, name, input).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = json_body(response).await;
        assert_eq!(
            body["error"],
            json!({"code":"unavailable","outcome":"not_applied"})
        );
        assert!(body.get("result").is_none());
        assert!(!body.to_string().contains("private-detail"));
        let rows = filegate_db::registry::list_storages(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].capacity_bytes, 100);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn storage_http_commit_failure_is_unknown_without_retry(pool: PgPool) {
    let token = owner(&pool).await;
    let root = Root::new();
    assert_eq!(
        call(&pool, &token, "storage.create", root.input("local", 100))
            .await
            .status(),
        StatusCode::OK
    );
    sqlx::raw_sql("CREATE SEQUENCE public.storage_attempts;
        CREATE FUNCTION public.reject_storage_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('public.storage_attempts'); RAISE EXCEPTION 'private-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT OR UPDATE OR DELETE ON storages
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.reject_storage_commit();")
        .execute(&pool).await.unwrap();
    for (name, input) in [
        ("storage.create", root.input("new", 1)),
        ("storage.replace", root.input("local", 200)),
        ("storage.delete", json!({"id":"local"})),
    ] {
        let response = call(&pool, &token, name, input).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(!response.headers().contains_key(header::RETRY_AFTER));
        let body = json_body(response).await;
        assert_eq!(
            body["error"],
            json!({"code":"unavailable","outcome":"unknown"})
        );
        assert!(body.get("result").is_none());
        assert!(!body.to_string().contains("private-detail"));
    }
    let attempts: i64 = sqlx::query_scalar("SELECT last_value FROM public.storage_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 3);
    let rows = filegate_db::registry::list_storages(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].capacity_bytes, 100);
}
