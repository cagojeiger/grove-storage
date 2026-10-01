use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn invalid_clients_conflicts_and_missing_reads_have_matching_statuses(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    for (id, storage, status) in [
        ("api", "vendor", StatusCode::BAD_REQUEST),
        ("INVALID", "vendor", StatusCode::BAD_REQUEST),
        ("new", "missing", StatusCode::BAD_REQUEST),
        ("app", "vendor", StatusCode::CONFLICT),
    ] {
        let input = json!({"id":id,"storage_id":storage});
        let old = payload(api.legacy("POST", "/clients", input.clone()).await, status).await;
        let new = payload(api.command("client.create", input).await, status).await;
        assert!(old["error"].is_string());
        assert_eq!(new["error"]["outcome"], "not_applied");
    }
    for (path, name, input) in [
        ("/clients/missing", "client.show", json!({"id":"missing"})),
        ("/storages/missing", "storage.show", json!({"id":"missing"})),
        (
            "/clients/missing/keys",
            "client-key.list",
            json!({"client_id":"missing"}),
        ),
        (
            "/clients/missing/s3-credentials",
            "credential.list",
            json!({"client_id":"missing"}),
        ),
    ] {
        payload(
            api.legacy("GET", path, Value::Null).await,
            StatusCode::NOT_FOUND,
        )
        .await;
        let new = payload(api.command(name, input).await, StatusCode::NOT_FOUND).await;
        assert_eq!(
            new["error"],
            json!({"code":"not_found","outcome":"not_applied"})
        );
    }
    assert_eq!(
        filegate_db::registry::list_clients(&pool).await.unwrap(),
        ["app"]
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn missing_key_owner_preserves_existing_rest_and_command_error_contracts(pool: PgPool) {
    let api = Pair::new(&pool).await;
    for (path, name, input) in [
        (
            "/clients/missing/s3-credentials",
            "credential.create",
            json!({"client_id":"missing"}),
        ),
        (
            "/clients/missing/keys",
            "client-key.register",
            json!({"client_id":"missing","key_hash":filegate_core::client_key_hash("fixture")}),
        ),
    ] {
        // REST translates a missing FK parent to 404; commands reject input with 400.
        let old = payload(
            api.legacy("POST", path, input.clone()).await,
            StatusCode::NOT_FOUND,
        )
        .await;
        assert_eq!(old["error"], "client not found");
        let new = payload(api.command(name, input).await, StatusCode::BAD_REQUEST).await;
        assert_eq!(
            new["error"],
            json!({"code":"invalid_input","outcome":"not_applied"})
        );
    }
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM client_keys), (SELECT count(*) FROM s3_credentials)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn retained_files_and_locations_block_deletion_even_without_active_usage(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    sqlx::query("INSERT INTO clients(id,storage_id) VALUES('retained','local')")
        .execute(&pool)
        .await
        .unwrap();
    // The client points at local, so only a physical location can protect vendor.
    for state in ["pending", "active", "deleted", "reclaimed"] {
        let file: Uuid = sqlx::query_scalar("INSERT INTO files(client_id,state,declared_size,committed_at,deleted_at) VALUES('retained',$1,0,now(),now()) RETURNING id")
            .bind(state).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO locations(file_id,storage_id,object_key) VALUES($1,'vendor',$2)")
            .bind(file)
            .bind(file.to_string())
            .execute(&pool)
            .await
            .unwrap();
        for (path, name, id) in [
            ("/clients/retained", "client.delete", "retained"),
            ("/storages/vendor", "storage.delete", "vendor"),
        ] {
            payload(
                api.legacy("DELETE", path, Value::Null).await,
                StatusCode::CONFLICT,
            )
            .await;
            let result = payload(
                api.command(name, json!({"id":id})).await,
                StatusCode::CONFLICT,
            )
            .await;
            assert_eq!(
                result["error"],
                json!({"code":"conflict","outcome":"not_applied"})
            );
        }
        let retained: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM locations WHERE file_id=$1)")
                .bind(file)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(retained, "{state}");
        sqlx::query("DELETE FROM locations WHERE file_id=$1")
            .bind(file)
            .execute(&pool)
            .await
            .unwrap();
        // Purging physical bytes does not remove the file's client ownership.
        payload(
            api.legacy("DELETE", "/clients/retained", Value::Null).await,
            StatusCode::CONFLICT,
        )
        .await;
        payload(
            api.command("client.delete", json!({"id":"retained"})).await,
            StatusCode::CONFLICT,
        )
        .await;
        sqlx::query("DELETE FROM files WHERE id=$1")
            .bind(file)
            .execute(&pool)
            .await
            .unwrap();
    }
    // A registry assignment alone protects an empty storage, too.
    sqlx::query("INSERT INTO clients(id,storage_id) VALUES('empty','vendor')")
        .execute(&pool)
        .await
        .unwrap();
    payload(
        api.legacy("DELETE", "/storages/vendor", Value::Null).await,
        StatusCode::CONFLICT,
    )
    .await;
    payload(
        api.command("storage.delete", json!({"id":"vendor"})).await,
        StatusCode::CONFLICT,
    )
    .await;
    assert!(
        filegate_db::registry::get_storage(&pool, "vendor")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        filegate_db::s3_registry::list_credentials(&pool, "app")
            .await
            .unwrap(),
        ["testaccesskey"]
    );
}
