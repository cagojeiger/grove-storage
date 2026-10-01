use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn account_search_crosses_pages_and_cursors_round_trip(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    for i in 0..53 {
        create(&pool, &cookie, serde_json::json!({"kind":"user","display_name":format!("Fixture {i}"),"role":"reader"})).await;
    }
    let all = get(&pool, &cookie, "/accounts?q=Fixture&limit=100").await;
    assert_eq!(all["items"].as_array().unwrap().len(), 53);
    let first = get(&pool, &cookie, "/accounts?q=fixture&limit=50").await;
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert!(first["previous_after"].is_null());
    let next = get(
        &pool,
        &cookie,
        &format!(
            "/accounts?q=fixture&limit=50&before={}",
            first["next_before"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(next["items"].as_array().unwrap().len(), 3);
    assert!(next["next_before"].is_null());
    let previous = get(
        &pool,
        &cookie,
        &format!(
            "/accounts?q=fixture&limit=50&after={}",
            next["previous_after"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(first, previous);
    let combined: Vec<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(next["items"].as_array().unwrap())
        .cloned()
        .collect();
    assert_eq!(serde_json::json!(combined), all["items"]);
    let exact = get(&pool, &cookie, "/accounts?q=fixture&limit=53").await;
    assert!(exact["next_before"].is_null());

    let small = get(&pool, &cookie, "/accounts?q=fixture&limit=20").await;
    let middle = get(
        &pool,
        &cookie,
        &format!(
            "/accounts?q=fixture&limit=20&before={}",
            small["next_before"].as_str().unwrap()
        ),
    )
    .await;
    let last = get(
        &pool,
        &cookie,
        &format!(
            "/accounts?q=fixture&limit=20&before={}",
            middle["next_before"].as_str().unwrap()
        ),
    )
    .await;
    let back = get(
        &pool,
        &cookie,
        &format!(
            "/accounts?q=fixture&limit=20&after={}",
            last["previous_after"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(back, middle);
    let start = get(
        &pool,
        &cookie,
        &format!(
            "/accounts?q=fixture&limit=20&after={}",
            back["previous_after"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(start, small);

    let hidden = all["items"][52]["id"].as_str().unwrap();
    sqlx::query("UPDATE management.accounts SET display_name='Unloaded Needle' WHERE id=$1")
        .bind(hidden.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let result = get(
        &pool,
        &cookie,
        "/accounts?q=nEeDlE&role=reader&status=active",
    )
    .await;
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["id"], hidden);
    let by_id = get(&pool, &cookie, &format!("/accounts?q={hidden}")).await;
    assert_eq!(by_id["items"][0]["id"], hidden);
    let empty = get(&pool, &cookie, "/accounts?q=absent").await;
    assert_eq!(empty["items"], serde_json::json!([]));
    assert_eq!(empty["initialized"], true);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn account_filters_distinguish_disabled_and_deleted_and_search_is_literal(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    let mut ids = Vec::new();
    for name in [
        "Filter active",
        "Filter disabled",
        "Filter deleted",
        "Filter 100%_'quote",
    ] {
        ids.push(
            create(
                &pool,
                &cookie,
                serde_json::json!({"kind":"user","display_name":name,"role":"writer"}),
            )
            .await,
        );
    }
    db::change_account(&pool, &context(), ids[1], db::AccountChange::Active(false))
        .await
        .unwrap();
    db::change_account(&pool, &context(), ids[2], db::AccountChange::Delete)
        .await
        .unwrap();
    for (status, expected) in [
        ("all", 4),
        ("current", 3),
        ("active", 2),
        ("disabled", 1),
        ("deleted", 1),
    ] {
        let result = get(
            &pool,
            &cookie,
            &format!("/accounts?q=Filter&role=writer&status={status}"),
        )
        .await;
        assert_eq!(
            result["items"].as_array().unwrap().len(),
            expected,
            "{status}"
        );
    }
    for q in ["%25", "_", "%27", "100%25_%27quote"] {
        let result = get(&pool, &cookie, &format!("/accounts?q={q}")).await;
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
        assert_eq!(result["items"][0]["id"], ids[3].to_string());
    }
    let result = get(&pool, &cookie, "/accounts?q=Filter&role=reader").await;
    assert_eq!(result["items"], serde_json::json!([]));
    let legacy = get(&pool, &cookie, "/accounts").await;
    assert!(
        legacy["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == ids[2].to_string())
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn account_filters_reject_invalid_input(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    let id = Uuid::new_v4();
    for query in [
        "status=unknown".into(),
        "role=root".into(),
        "limit=0".into(),
        "limit=101".into(),
        "after=invalid".into(),
        "q=a&q=b".into(),
        "owner=forged".into(),
        format!("before={id}&after={id}"),
        format!("q={}", "a".repeat(81)),
    ] {
        let response = send(
            &pool,
            &cookie,
            "GET",
            &format!("/accounts?{query}"),
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
    }
}
