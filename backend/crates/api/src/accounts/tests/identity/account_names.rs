use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn rename_validates_input_and_preserves_account_id(pool: PgPool) {
    let (id, cookie) = actor(&pool, Role::Admin).await;
    let path = format!("/accounts/{id}");
    for body in [
        serde_json::json!({"operation":"name","display_name":" "}),
        serde_json::json!({"operation":"name","display_name":"x".repeat(81)}),
        serde_json::json!({"operation":"name","display_name":"valid","role":"reader"}),
    ] {
        assert_eq!(
            send(&pool, &cookie, "PATCH", &path, body).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    for changed in [true, false] {
        let response = send(
            &pool,
            &cookie,
            "PATCH",
            &path,
            serde_json::json!({"operation":"name","display_name":" Renamed "}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["changed"], changed);
    }
    let account = get(&pool, &cookie, &path).await;
    assert_eq!(account["id"], id.to_string());
    assert_eq!(account["display_name"], "Renamed");
    assert_eq!(account["role"], "admin");
    for target in ["root".into(), Uuid::new_v4().to_string()] {
        let expected = if target == "root" {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::NOT_FOUND
        };
        assert_eq!(
            send(
                &pool,
                &cookie,
                "PATCH",
                &format!("/accounts/{target}"),
                serde_json::json!({"operation":"name","display_name":"Lost"})
            )
            .await
            .status(),
            expected
        );
    }
}
