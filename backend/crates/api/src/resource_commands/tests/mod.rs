#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod boundaries;
mod legacy_contract;
mod reads;
mod storage_failures;
mod storage_s3;
mod storages;
mod write_failures;
mod writes;

use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request as HttpRequest,
};
use filegate_db::{PgPool, management as db};
use grove_management_policy::Role;
use serde_json::{Value, json};
use tower::ServiceExt;

const PATH: &str = "/api/admin/commands/v1";
pub(crate) async fn owner(pool: &PgPool) -> String {
    let token = format!("gsm_{}", filegate_core::generate_url_secret());
    let context = db::AuditContext {
        actor: db::AuditActor::Master { session_id: None },
        request_id: Uuid::new_v4(),
        surface: Surface::Console,
    };
    db::bootstrap(
        pool,
        &context,
        "Owner",
        &db::NewCredential {
            label: "test",
            token_prefix: "gsm_test",
            token_hash: &secrets::token_hash(&token),
            expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        },
    )
    .await
    .unwrap();
    token
}
async fn request(
    pool: &PgPool,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Value,
) -> Response {
    let mut state = crate::routes::tests::test_state();
    state.pool = pool.clone();
    state.console_origin = Some("https://console.test".into());
    let mut builder = HttpRequest::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (key, value) in headers {
        builder = builder.header(*key, *value);
    }
    crate::routes::app(state, &[])
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn call(pool: &PgPool, token: &str, command: &str, input: Value) -> Response {
    request(
        pool,
        "POST",
        PATH,
        &[("authorization", &format!("Bearer {token}"))],
        json!({"protocol":1,"command":command,"input":input}),
    )
    .await
}
async fn json_body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
pub(crate) async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('local','fs','/fixture',100);
        INSERT INTO storages(id,kind,endpoint,public_endpoint,region,bucket,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes)
        VALUES('vendor','s3','https://s3.test','https://s3.test','region','bucket','public-access','private-ciphertext',decode(repeat('ab',12),'hex'),'private-key-id',0);
        INSERT INTO clients(id,storage_id) VALUES('app','local');
        INSERT INTO client_keys(client_id,key_hash) VALUES('app','sha256:'||repeat('a',64));
        INSERT INTO s3_credentials(access_key_id,client_id,secret_key_ciphertext,secret_key_nonce,enc_key_id)
        VALUES('testaccesskey','app','private-service-cipher',decode(repeat('ab',12),'hex'),'private-key-id');
        INSERT INTO usage_snapshot(day,storage_id,client_id,active_bytes,active_files) VALUES(current_date,'local','app',120,1);
        WITH f AS (INSERT INTO files(client_id,state,declared_size,committed_at,deleted_at)
          VALUES('app','active',120,now(),NULL),('app','pending',10,NULL,NULL),('app','deleted',20,now(),now()) RETURNING id)
        INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'local',id::text FROM f;")
        .execute(pool).await.unwrap();
}
