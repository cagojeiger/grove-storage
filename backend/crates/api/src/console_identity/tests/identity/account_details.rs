use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn account_lookup_is_independent_of_pages_and_preserves_deleted_state(pool: PgPool) {
    let (caller, cookie) = actor(&pool, Role::Admin).await;
    create(
        &pool,
        &cookie,
        serde_json::json!({"kind":"user","display_name":"Extra","role":"reader"}),
    )
    .await;
    let first = get(&pool, &cookie, "/accounts?limit=1").await;
    let all = get(&pool, &cookie, "/accounts?limit=100").await;
    let expected = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] != first["items"][0]["id"] && row["id"] != caller.to_string())
        .unwrap();
    let target = expected["id"].as_str().unwrap();
    let path = format!("/accounts/{target}");
    let row = get(&pool, &cookie, &path).await;
    assert_eq!(&row, expected);
    assert_eq!(row.as_object().unwrap().len(), 8);
    assert!(row["username"].is_null());
    assert_eq!(row["password_ready"], false);
    let id: Uuid = target.parse().unwrap();
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations WHERE operation='identity.account.get'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(invocations, 1);
    db::change_account(&pool, &context(), id, db::AccountChange::Delete)
        .await
        .unwrap();
    let row = get(&pool, &cookie, &path).await;
    assert_eq!(row["id"], target);
    assert_eq!(row["is_active"], false);
    assert!(!row["deleted_at"].is_null());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn account_lookup_rejects_missing_and_invalid_ids(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    for (id, expected) in [
        (Uuid::new_v4().to_string(), StatusCode::NOT_FOUND),
        ("invalid".into(), StatusCode::BAD_REQUEST),
    ] {
        let response = send(
            &pool,
            &cookie,
            "GET",
            &format!("/accounts/{id}"),
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(response.status(), expected);
    }
}
