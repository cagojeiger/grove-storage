//! Characterize both public contracts before consolidating their implementations.
mod clients;
mod guards;
mod identity;
mod storages;
mod usage;

use super::*;

struct Pair<'a> {
    pool: &'a PgPool,
    operator: String,
    user: String,
}

impl<'a> Pair<'a> {
    async fn new(pool: &'a PgPool) -> Self {
        let user = owner(pool).await;
        let operator = format!("fgop_{}", filegate_core::generate_url_secret());
        filegate_db::admin_auth::issue(
            pool,
            filegate_db::admin_auth::IssueMode::Initialize,
            "contract-test",
            &crate::admin_auth::hash("admin-token", &operator),
        )
        .await
        .unwrap()
        .unwrap();
        Self {
            pool,
            operator,
            user,
        }
    }

    async fn legacy(&self, method: &str, path: &str, body: Value) -> Response {
        request(
            self.pool,
            method,
            &format!("/api/admin/v1{path}"),
            &[("authorization", &format!("Bearer {}", self.operator))],
            body,
        )
        .await
    }

    async fn command(&self, name: &str, input: Value) -> Response {
        call(self.pool, &self.user, name, input).await
    }

    async fn same_read(&self, path: &str, name: &str, input: Value) -> Value {
        let old = payload(self.legacy("GET", path, Value::Null).await, StatusCode::OK).await;
        let new = payload(self.command(name, input).await, StatusCode::OK).await;
        assert_eq!(old, new["result"], "{name}");
        old
    }
}

async fn payload(response: Response, status: StatusCode) -> Value {
    let actual = response.status();
    let body = json_body(response).await;
    assert_eq!(actual, status, "{body}");
    body
}

async fn deleted(response: Response) {
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        to_bytes(response.into_body(), 1024)
            .await
            .unwrap()
            .is_empty()
    );
}
