use super::*;
mod account_details;
mod account_names;
mod account_search;
mod accounts;
mod authorization;
mod credentials;
mod failures;
mod history;
const BASE: &str = "/api/admin/identity/v1";

async fn send(
    pool: &PgPool,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> Response {
    request(
        app(pool),
        method,
        &format!("{BASE}{path}"),
        &[
            ("cookie", cookie),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        body.to_string(),
    )
    .await
}
async fn actor(pool: &PgPool, role: Role) -> (Uuid, String) {
    let (id, _, token) = account(pool, role).await;
    (id, cookie(&login(app(pool), &token).await))
}
async fn create(pool: &PgPool, cookie: &str, body: serde_json::Value) -> Uuid {
    let response = send(pool, cookie, "POST", "/accounts", body).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await["account_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}
async fn issue(pool: &PgPool, cookie: &str, id: Uuid) -> serde_json::Value {
    let response = send(
        pool,
        cookie,
        "POST",
        &format!("/accounts/{id}/credentials"),
        serde_json::json!({"label":"automation","expires_in_days":1}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await
}
async fn get(pool: &PgPool, cookie: &str, path: &str) -> serde_json::Value {
    let response = send(pool, cookie, "GET", path, serde_json::Value::Null).await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}
