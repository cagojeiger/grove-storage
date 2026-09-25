use super::master::{router, setup, sign_in};
use super::*;
use grove_management_service::master::Config;
use std::sync::Arc;

#[sqlx::test(migrations = "../db/migrations")]
async fn root_console_is_audited_and_cannot_be_an_editable_user_or_bearer(pool: PgPool) {
    account(&pool, Role::Admin).await;
    let (app, token) = setup(&pool).await;
    let response = login(app.clone(), &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    let root_cookie = cookie(&response);
    assert!(root_cookie.contains("gsrs_"));
    let current = json(current(app.clone(), &root_cookie).await).await;
    assert_eq!(current["principal"], "root");
    assert_eq!(current["role"], "root");
    let response = request(
        app.clone(),
        "GET",
        "/api/admin/identity/v1/root",
        &[("cookie", &root_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["protected"], true);
    let response = master::change(
        app.clone(),
        "/api/admin/identity/v1/accounts",
        &root_cookie,
        serde_json::json!({"kind":"user","display_name":"Created by Root","role":"reader"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let event: (String, Option<Uuid>, Option<Uuid>) = sqlx::query_as("SELECT actor_kind,actor_id,session_id FROM management.audit_events WHERE action='account.create' ORDER BY id DESC LIMIT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(event.0, "master"); // Stable persisted name for the config-owned principal.
    assert_eq!(event.1, None);
    assert!(event.2.is_some());
    for role in ["root", "master"] {
        assert_eq!(
            master::change(
                app.clone(),
                "/api/admin/identity/v1/accounts",
                &root_cookie,
                serde_json::json!({"kind":"user","display_name":"forged","role":role})
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        request(
            app.clone(),
            "DELETE",
            "/api/admin/identity/v1/accounts/root",
            &[
                ("cookie", &root_cookie),
                ("origin", ORIGIN),
                ("x-grove-csrf", "1")
            ],
            String::new()
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let body = serde_json::json!({"protocol":1,"command":"client.list","input":{}});
    assert_eq!(
        master::change(
            app.clone(),
            "/api/admin/console-commands/v1",
            &root_cookie,
            body.clone()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/admin/commands/v1",
            &[
                ("authorization", &format!("Bearer {token}")),
                ("content-type", "application/json")
            ],
            body.to_string()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        logout(app.clone(), &root_cookie).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        super::current(app, &root_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn root_rotation_fences_sessions_and_setup_sessions_never_gain_console_access(pool: PgPool) {
    let (old_app, old_token) = setup(&pool).await;
    let setup_cookie = cookie(&sign_in(old_app.clone(), &old_token).await);
    assert_eq!(
        current(old_app.clone(), &setup_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let root_cookie = cookie(&login(old_app.clone(), &old_token).await);
    let new_token = format!("gsrt_{}", filegate_core::generate_url_secret());
    let config = Arc::new(Config::new(2, secrets::master_hash(&new_token)).unwrap());
    assert!(config.install(&pool).await.unwrap());
    let new_app = router(&pool, config);
    assert_eq!(
        current(new_app.clone(), &root_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        current(old_app.clone(), &root_cookie).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        login(old_app, &old_token).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        login(new_app.clone(), &old_token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(login(new_app, &new_token).await.status(), StatusCode::OK);
}

#[test]
fn root_configuration_is_explicit_and_rejects_ambiguous_aliases() {
    let token = format!("gsrt_{}", "a".repeat(64));
    assert!(
        super::super::master_config::load(&|key| match key {
            "GROVE_ROOT_TOKEN" => Some(token.clone()),
            "GROVE_ROOT_GENERATION" => Some("1".into()),
            _ => None,
        })
        .unwrap()
        .is_some()
    );
    assert!(
        super::super::master_config::load(&|key| match key {
            "GROVE_ROOT_TOKEN" | "FILEGATE_MASTER_TOKEN" => Some(token.clone()),
            "GROVE_ROOT_GENERATION" => Some("1".into()),
            _ => None,
        })
        .is_err()
    );
}
