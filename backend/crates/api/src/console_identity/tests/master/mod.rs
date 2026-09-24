use super::*;
use grove_management_service::master::Config;
use std::sync::Arc;
pub(super) const MASTER_PATH: &str = "/api/admin/identity/v1/master/session";
pub(super) const BOOTSTRAP: &str = "/api/admin/identity/v1/master/bootstrap";
pub(super) const RECOVER: &str = "/api/admin/identity/v1/master/recover";

pub(super) fn router(pool: &PgPool, config: Arc<Config>) -> Router {
    let mut state = crate::routes::tests::test_state();
    state.pool = pool.clone();
    state.console_origin = Some(ORIGIN.into());
    state.master = Some(config);
    crate::routes::app(state, &[])
}
pub(super) async fn setup(pool: &PgPool) -> (Router, String) {
    let token = format!("gsmt_{}", filegate_core::generate_url_secret());
    let config = Arc::new(Config::new(1, secrets::master_hash(&token)).unwrap());
    assert!(config.install(pool).await.unwrap());
    (router(pool, config), token)
}
pub(super) async fn change(
    router: Router,
    path: &str,
    cookie: &str,
    body: serde_json::Value,
) -> Response {
    request(
        router,
        "POST",
        path,
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
            ("cookie", cookie),
        ],
        body.to_string(),
    )
    .await
}
pub(super) async fn sign_in(router: Router, token: &str) -> Response {
    change(router, MASTER_PATH, "", serde_json::json!({"token":token})).await
}

mod browser;
mod configuration;
mod flows;
