use super::*;
use filegate_core::{EncryptedSecret, ExposeSecret};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(super) struct Provider {
    endpoint: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Provider {
    pub(super) async fn start(deny_list: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let app = axum::Router::new().fallback(move |request: axum::extract::Request| {
            let count = count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                assert!(request.headers().contains_key(header::AUTHORIZATION));
                if request.method() == "HEAD" { return StatusCode::OK.into_response(); }
                assert_eq!(request.method(), "GET");
                assert!(request.uri().query().unwrap().contains("uploads"));
                if deny_list { return (StatusCode::FORBIDDEN,"private-provider-detail").into_response(); }
                (StatusCode::OK,[(header::CONTENT_TYPE,"application/xml")],
                    "<ListMultipartUploadsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><Bucket>bucket</Bucket><IsTruncated>false</IsTruncated></ListMultipartUploadsResult>")
                    .into_response()
            }
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            endpoint,
            calls,
            task,
        }
    }
    pub(super) fn input_for(&self, id: &str, capacity: i64) -> Value {
        let mut value = self.input("fixture-provider-secret");
        value["id"] = json!(id);
        value["spec"]["capacity_bytes"] = json!(capacity);
        value
    }
    fn input(&self, secret: &str) -> Value {
        json!({"id":"vendor","spec":{"kind":"s3","endpoint":self.endpoint,"region":"us-east-1","bucket":"bucket",
            "force_path_style":true,"access_key":"provider-access","secret_key":secret,"capacity_bytes":100}})
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn saved_connection_tests_are_read_only_and_redact_provider_failures(pool: PgPool) {
    let token = owner(&pool).await;
    let provider = Provider::start(false).await;
    assert_eq!(
        call(
            &pool,
            &token,
            "storage.create",
            provider.input("private-provider-secret")
        )
        .await
        .status(),
        StatusCode::OK
    );
    let before = filegate_db::registry::get_storage(&pool, "vendor")
        .await
        .unwrap()
        .unwrap();
    let response = call(&pool, &token, "storage.test", json!({"id":"vendor"})).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await["result"],
        json!({"id":"vendor", "state":"ok"})
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        filegate_db::registry::get_storage(&pool, "vendor")
            .await
            .unwrap()
            .unwrap(),
        before
    );

    let denied = Provider::start(true).await;
    sqlx::query("UPDATE storages SET endpoint=$1 WHERE id='vendor'")
        .bind(&denied.endpoint)
        .execute(&pool)
        .await
        .unwrap();
    let response = call(&pool, &token, "storage.test", json!({"id":"vendor"})).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(response).await;
    assert_eq!(
        body["error"],
        json!({"code":"unavailable", "outcome":"not_applied"})
    );
    assert!(!body.to_string().contains("private-"));
    assert!(!body.to_string().contains(&denied.endpoint));
    assert_eq!(denied.calls.load(Ordering::SeqCst), 2);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn provider_probes_encrypt_and_rotate_without_leaking_secrets(pool: PgPool) {
    let token = owner(&pool).await;
    let provider = Provider::start(false).await;
    let state = crate::routes::tests::test_state();
    for (index, name, secret) in [
        (0, "storage.create", "first-private-provider-secret"),
        (1, "storage.replace", "second-private-provider-secret"),
    ] {
        let response = call(&pool, &token, name, provider.input(secret)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert!(!body.to_string().contains(secret));
        assert!(body["result"].get("secret_key_ciphertext").is_none());
        let saved = filegate_db::registry::get_storage(&pool, "vendor")
            .await
            .unwrap()
            .unwrap();
        let plain = state
            .crypto
            .decrypt(
                saved.enc_key_id.as_deref().unwrap(),
                "vendor",
                &EncryptedSecret {
                    ciphertext: saved.secret_key_ciphertext.unwrap(),
                    nonce: saved.secret_key_nonce.unwrap(),
                },
            )
            .unwrap();
        assert_eq!(plain.expose_secret(), secret);
        if index == 0 {
            sqlx::raw_sql("INSERT INTO clients(id,storage_id) VALUES('app','vendor');
                WITH f AS (INSERT INTO files(client_id,state,declared_size) VALUES('app','pending',0) RETURNING id)
                INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'vendor',id::text FROM f;")
                .execute(&pool).await.unwrap();
        }
        let logs: Vec<String> =
            sqlx::query_scalar("SELECT row_to_json(t)::text FROM management.audit_events t")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(logs.iter().all(|s| !s.contains(secret)
            && !s.contains(&provider.endpoint)
            && !s.contains("provider-access")));
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn missing_provider_recovery_permission_rejects_registration(pool: PgPool) {
    let token = owner(&pool).await;
    let provider = Provider::start(true).await;
    let response = call(
        &pool,
        &token,
        "storage.create",
        provider.input("private-provider-secret"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    assert_eq!(
        body["error"],
        json!({"code":"invalid_input","outcome":"not_applied"})
    );
    assert!(!body.to_string().contains("private-"));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert!(
        filegate_db::registry::list_storages(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}
