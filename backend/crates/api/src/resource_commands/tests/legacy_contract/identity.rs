use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn issued_credentials_cannot_cross_authentication_boundaries_to_mutate_resources(
    pool: PgPool,
) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    let input = json!({"id":"blocked","storage_id":"vendor"});
    let old = request(
        &pool,
        "POST",
        "/api/admin/v1/clients",
        &[("authorization", &format!("Bearer {}", api.user))],
        input.clone(),
    )
    .await;
    payload(old, StatusCode::UNAUTHORIZED).await;
    let new = call(&pool, &api.operator, "client.create", input).await;
    let error = payload(new, StatusCode::UNAUTHORIZED).await;
    assert_eq!(
        error["error"],
        json!({"code":"unauthorized","outcome":"not_applied"})
    );
    assert_eq!(
        api.same_read("/clients", "client.list", json!({})).await,
        json!(["app"])
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT
        (SELECT count(*) FROM admin_audit_events WHERE action='POST' AND target='/clients'),
        (SELECT count(*) FROM management.audit_events WHERE action='client.create')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn shared_resource_operations_keep_each_audit_actor_and_table_boundary(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    let operator = filegate_db::admin_auth::authenticate(
        &pool,
        &crate::admin_auth::hash("admin-token", &api.operator),
    )
    .await
    .unwrap()
    .unwrap();
    let user = db::authenticate(&pool, &secrets::token_hash(&api.user))
        .await
        .unwrap()
        .unwrap();
    payload(
        api.legacy(
            "POST",
            "/clients",
            json!({"id":"legacy-client","storage_id":"vendor"}),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    let response = payload(
        api.command(
            "client.create",
            json!({"id":"command-client","storage_id":"vendor"}),
        )
        .await,
        StatusCode::OK,
    )
    .await;
    let request_id: Uuid = response["request_id"].as_str().unwrap().parse().unwrap();

    let legacy: Vec<(Uuid, String, String, i32)> = sqlx::query_as(
        "SELECT credential_id,actor,target,status FROM admin_audit_events WHERE action='POST'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(legacy, [(operator, "admin".into(), "/clients".into(), 201)]);
    let managed: Vec<(Uuid,Uuid,Uuid,String,String)> = sqlx::query_as(
        "SELECT actor_id,credential_id,request_id,surface,resource_id FROM management.audit_events WHERE action='client.create'",
    ).fetch_all(&pool).await.unwrap();
    assert_eq!(
        managed,
        [(
            user.account_id,
            user.credential_id.unwrap(),
            request_id,
            "resource_api".into(),
            "command-client".into()
        )]
    );
    for table in [
        "admin_audit_events",
        "management.audit_events",
        "management.command_invocations",
    ] {
        let records: Vec<String> =
            sqlx::query_scalar(&format!("SELECT row_to_json(t)::text FROM {table} t"))
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(
            records
                .iter()
                .all(|record| !record.contains(&api.operator) && !record.contains(&api.user))
        );
    }
}
