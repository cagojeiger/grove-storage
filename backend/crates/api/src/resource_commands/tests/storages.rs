use super::*;

pub(super) struct Root(pub std::path::PathBuf);
impl Root {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("grove-storage-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn input(&self, id: &str, capacity: i64) -> Value {
        json!({"id":id,"spec":{"kind":"fs","root_path":self.0,"capacity_bytes":capacity}})
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn storage_http_lifecycle_matches_legacy_reads_and_preserves_references(pool: PgPool) {
    let token = owner(&pool).await;
    let root = super::storage_s3::Provider::start(false).await;
    let response = call(
        &pool,
        &token,
        "storage.create",
        root.input_for("local", 100),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let created = json_body(response).await;
    let legacy = request(
        &pool,
        "GET",
        "/api/admin/v1/storages/local",
        &[("authorization", "Bearer test-operator-token")],
        json!({}),
    )
    .await;
    assert_eq!(json_body(legacy).await, created["result"]);
    filegate_db::registry::insert_client(&pool, "app", "local")
        .await
        .unwrap();
    let response = call(&pool, &token, "storage.delete", json!({"id":"local"})).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    filegate_db::registry::delete_client(&pool, "app")
        .await
        .unwrap();
    let response = call(
        &pool,
        &token,
        "storage.replace",
        root.input_for("local", 200),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["result"]["capacity_bytes"], 200);
    for _ in 0..2 {
        let response = call(&pool, &token, "storage.delete", json!({"id":"local"})).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.audit_events WHERE action LIKE 'storage.%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 3);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn invalid_storage_fields_and_probe_failure_are_sanitized(pool: PgPool) {
    let token = owner(&pool).await;
    let root = Root::new();
    let mut specs = vec![
        json!({"kind":"fs","capacity_bytes":1}),
        json!({"kind":"fs","root_path":root.0.join("absent-private-path"),"capacity_bytes":1}),
        json!({"kind":"s3","endpoint":"ftp://private-host","region":"r","bucket":"b","access_key":"a","secret_key":"private-secret","capacity_bytes":1}),
        json!({"kind":"s3","endpoint":"http://localhost","capacity_bytes":1}),
    ];
    for (field, value) in [
        ("force_path_style", json!(true)),
        ("force_relay", json!(true)),
        ("secret_key", json!("private-secret")),
    ] {
        let mut spec = root.input("local", 1)["spec"].clone();
        spec[field] = value;
        specs.push(spec);
    }
    for spec in specs {
        let response = call(
            &pool,
            &token,
            "storage.create",
            json!({"id":"local","spec":spec}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = json_body(response).await;
        assert_eq!(
            body["error"],
            json!({"code":"invalid_input","outcome":"not_applied"})
        );
        assert!(!body.to_string().contains("private-"));
    }
    let mut state = crate::routes::tests::test_state();
    state.public_url = None;
    let input = serde_json::from_value(
        json!({"id":"relay","spec":{"kind":"s3","force_relay":true,"capacity_bytes":1}}),
    )
    .unwrap();
    assert!(matches!(
        crate::storage_registration::verify_command(
            &state.crypto,
            state.public_url.is_some(),
            input
        )
        .await,
        Err(grove_management_service::Error::InvalidInput)
    ));
    assert!(
        filegate_db::registry::list_storages(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn legacy_storage_writes_share_registration_with_commands(pool: PgPool) {
    let token = owner(&pool).await;
    let root = super::storage_s3::Provider::start(false).await;
    let mut body = root.input_for("legacy", 100)["spec"].clone();
    body["id"] = json!("legacy");
    let response = request(
        &pool,
        "POST",
        "/api/admin/v1/storages",
        &[("authorization", "Bearer test-operator-token")],
        body.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    let read = json_body(call(&pool, &token, "storage.show", json!({"id":"legacy"})).await).await;
    assert_eq!(read["result"], created);

    body["capacity_bytes"] = json!(200);
    let response = request(
        &pool,
        "PUT",
        "/api/admin/v1/storages/legacy",
        &[("authorization", "Bearer test-operator-token")],
        body,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["capacity_bytes"], 200);
    assert_eq!(
        call(&pool, &token, "storage.delete", json!({"id":"legacy"}))
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn filesystem_create_and_replace_are_rejected_without_mutation(pool: PgPool) {
    let token = owner(&pool).await;
    let provider = super::storage_s3::Provider::start(false).await;
    assert_eq!(
        call(
            &pool,
            &token,
            "storage.create",
            provider.input_for("retained", 100)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let before = filegate_db::registry::get_storage(&pool, "retained")
        .await
        .unwrap()
        .unwrap();
    let spec = json!({"kind":"fs","root_path":"/never-probed","capacity_bytes":1});
    for (method, path, command, id) in [
        (
            "POST",
            "/api/admin/v1/storages",
            "storage.create",
            "blocked",
        ),
        (
            "PUT",
            "/api/admin/v1/storages/retained",
            "storage.replace",
            "retained",
        ),
    ] {
        let mut body = spec.clone();
        body["id"] = json!(id);
        assert_eq!(
            request(
                &pool,
                method,
                path,
                &[("authorization", "Bearer test-operator-token")],
                body
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(&pool, &token, command, json!({"id":id,"spec":spec}))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let after = filegate_db::registry::get_storage(&pool, "retained")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.kind, before.kind);
    assert_eq!(after.endpoint, before.endpoint);
    assert_eq!(after.capacity_bytes, before.capacity_bytes);
    assert_eq!(
        filegate_db::registry::list_storages(&pool)
            .await
            .unwrap()
            .len(),
        1
    );
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.audit_events WHERE action LIKE 'storage.%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);
}
