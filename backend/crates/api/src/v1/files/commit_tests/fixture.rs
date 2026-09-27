use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use filegate_db::{
    PgPool,
    files::{self, CreateOutcome, CreateSpec},
    registry::{self, StorageRow},
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

pub(super) const ETAG: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(super) struct Fixture {
    pub(super) state: crate::routes::AppState,
    pub(super) file: Uuid,
}

impl Fixture {
    pub(super) async fn new(pool: PgPool, relay: bool, endpoint: &str) -> Self {
        let mut state = crate::routes::tests::test_state();
        state.pool = pool;
        let secret = state
            .crypto
            .encrypt("home", &"provider-secret".to_owned().into())
            .unwrap();
        registry::insert_storage(
            &state.pool,
            &StorageRow {
                id: "home".into(),
                kind: "s3".into(),
                force_relay: relay,
                root_path: None,
                endpoint: Some(endpoint.into()),
                public_endpoint: Some("http://public.invalid".into()),
                region: Some("local".into()),
                bucket: Some("objects".into()),
                force_path_style: true,
                access_key: Some("access".into()),
                secret_key_ciphertext: Some(secret.ciphertext),
                secret_key_nonce: Some(secret.nonce),
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
            registry::insert_client_key(
                &state.pool,
                client,
                &filegate_core::client_key_hash(client),
            )
            .await
            .unwrap();
        }
        let CreateOutcome::Created(created) = files::create(
            &state.pool,
            CreateSpec {
                client_id: "app",
                declared_size: 7,
                declared_md5: Some(ETAG),
                content_type: None,
                lease_ttl_secs: 900,
                part_size: None,
            },
        )
        .await
        .unwrap() else {
            panic!("expected a file reservation");
        };
        Self {
            state,
            file: created.file_id,
        }
    }

    pub(super) async fn commit(&self, client: &str) -> (StatusCode, Value) {
        let response = crate::routes::app(self.state.clone(), &[])
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/files/{}/commit", self.file))
                    .header("authorization", format!("Bearer {client}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 8192).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    pub(super) async fn recorded(&self, size: i64, etag: &str) {
        sqlx::query(
            "UPDATE leases SET uploaded_size=$2,uploaded_md5=$3 WHERE file_id=$1 AND kind='write'",
        )
        .bind(self.file)
        .bind(size)
        .bind(etag)
        .execute(&self.state.pool)
        .await
        .unwrap();
    }

    pub(super) async fn assert_pending(&self) {
        let states: (String, String) = sqlx::query_as(
            "SELECT f.state,le.state FROM files f JOIN leases le ON le.file_id=f.id WHERE f.id=$1 AND le.kind='write'"
        ).bind(self.file).fetch_one(&self.state.pool).await.unwrap();
        assert_eq!(states, ("pending".into(), "issued".into()));
    }
}
