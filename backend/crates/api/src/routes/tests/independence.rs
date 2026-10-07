//! The registry is the contract between management and file operations.
use super::*;
use grove_db::{PgPool, registry, s3_registry};
use grove_s3_protocol::signing::{sha256_hex, sign};
use uuid::Uuid;

const ACCESS_KEY: &str = "fgak0123456789abcdef";
const SECRET: &str = "client-s3-secret";

async fn fixture(pool: PgPool) -> (AppState, Uuid) {
    let mut state = test_state();
    state.pool = pool;
    state.console_origin = Some("https://console.test".into());
    let encrypted = state
        .crypto
        .encrypt("home", &"provider-secret".into())
        .unwrap();
    registry::insert_storage(
        &state.pool,
        &registry::StorageRow {
            id: "home".into(),
            kind: "s3".into(),
            force_relay: false,
            endpoint: Some("https://provider.invalid".into()),
            public_endpoint: Some("https://provider.invalid".into()),
            region: Some("local".into()),
            bucket: Some("objects".into()),
            force_path_style: true,
            access_key: Some("provider-key".into()),
            secret_key_ciphertext: Some(encrypted.ciphertext),
            secret_key_nonce: Some(encrypted.nonce),
            enc_key_id: Some(state.crypto.active_key_id().into()),
            capacity_bytes: 0,
        },
    )
    .await
    .unwrap();
    for client in ["app", "other"] {
        registry::insert_client(&state.pool, client, "home")
            .await
            .unwrap();
        registry::insert_client_key(&state.pool, client, &grove_core::client_key_hash(client))
            .await
            .unwrap();
    }
    let encrypted = state.crypto.encrypt(ACCESS_KEY, &SECRET.into()).unwrap();
    s3_registry::insert_credential(
        &state.pool,
        ACCESS_KEY,
        "app",
        &encrypted.ciphertext,
        &encrypted.nonce,
        state.crypto.active_key_id(),
    )
    .await
    .unwrap();
    let file = Uuid::new_v4();
    sqlx::query("INSERT INTO files(id,client_id,state,declared_size,etag,committed_at) VALUES($1,'app','active',7,'etag',now())")
        .bind(file).execute(&state.pool).await.unwrap();
    sqlx::query("INSERT INTO locations(file_id,storage_id,object_key) VALUES($1,'home','object')")
        .bind(file)
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO s3_object_keys(client_id,key,file_id) VALUES('app','object',$1)")
        .bind(file)
        .execute(&state.pool)
        .await
        .unwrap();
    // Each sqlx test owns a disposable database. Fail any accidental account lookup.
    sqlx::query("DROP SCHEMA management CASCADE")
        .execute(&state.pool)
        .await
        .unwrap();
    (state, file)
}

async fn native(
    state: &AppState,
    method: &str,
    path: &str,
    key: &str,
    body: &str,
) -> axum::response::Response {
    app(state.clone(), &[])
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {key}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn s3(state: &AppState, method: &str, path: &str) -> axum::response::Response {
    let now = chrono::Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let time = now.format("%Y%m%dT%H%M%SZ").to_string();
    let scope = format!("{date}/local/s3/aws4_request");
    let canonical = format!(
        "{method}\n{path}\n\nhost:grove.test\nx-amz-date:{time}\n\nhost;x-amz-date\nUNSIGNED-PAYLOAD"
    );
    let signature = sign(
        SECRET,
        &date,
        "local",
        &format!(
            "AWS4-HMAC-SHA256\n{time}\n{scope}\n{}",
            sha256_hex(canonical.as_bytes())
        ),
    );
    app(state.clone(), &[]).oneshot(Request::builder()
        .method(method).uri(path).header("host", "grove.test")
        .header("x-amz-date", time).header("x-amz-content-sha256", "UNSIGNED-PAYLOAD")
        .header("authorization", format!("AWS4-HMAC-SHA256 Credential={ACCESS_KEY}/{scope}, SignedHeaders=host;x-amz-date, Signature={signature}"))
        .body(Body::empty()).unwrap()).await.unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn native_file_operations_need_registry_not_management(pool: PgPool) {
    let (state, file) = fixture(pool).await;
    let path = format!("/api/v1/files/{file}");
    let response = native(&state, "GET", &path, "app", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("active"));
    assert_eq!(
        native(&state, "GET", &path, "other", "").await.status(),
        StatusCode::NOT_FOUND
    );

    let created = native(
        &state,
        "POST",
        "/api/v1/files",
        "app",
        r#"{"declared_size":7,"declared_md5":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let output: serde_json::Value = serde_json::from_str(&body_text(created).await).unwrap();
    assert!(
        output
            .get("put_url")
            .unwrap()
            .as_str()
            .unwrap()
            .starts_with("https://provider.invalid/")
    );
    let read = native(&state, "POST", &format!("{path}/read"), "app", "{}").await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        native(&state, "DELETE", &path, "app", "").await.status(),
        StatusCode::OK
    );
    let deleted = native(&state, "GET", &path, "app", "").await;
    assert!(body_text(deleted).await.contains("deleted"));

    registry::delete_client_key(&state.pool, "app", &grove_core::client_key_hash("app"))
        .await
        .unwrap();
    assert_eq!(
        native(&state, "GET", &path, "app", "").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn s3_file_operations_need_registry_not_management(pool: PgPool) {
    let (state, _) = fixture(pool).await;
    let response = s3(&state, "HEAD", "/app/object").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-length"], "7");
    assert_eq!(
        s3(&state, "HEAD", "/other/object").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        s3(&state, "DELETE", "/app/object").await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        s3(&state, "HEAD", "/app/object").await.status(),
        StatusCode::NOT_FOUND
    );

    s3_registry::delete_credential(&state.pool, "app", ACCESS_KEY)
        .await
        .unwrap();
    assert_eq!(
        s3(&state, "HEAD", "/app/object").await.status(),
        StatusCode::FORBIDDEN
    );
}
