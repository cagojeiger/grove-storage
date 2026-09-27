use super::super::storage_s3::Provider;
use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn empty_storage_deletion_is_idempotent_in_both_directions(pool: PgPool) {
    let api = Pair::new(&pool).await;
    let provider = Provider::start(false).await;
    for legacy_delete in [true, false] {
        let input = provider.input_for("vendor", 100);
        if legacy_delete {
            payload(api.command("storage.create", input).await, StatusCode::OK).await;
        } else {
            let mut body = input["spec"].clone();
            body["id"] = json!("vendor");
            payload(
                api.legacy("POST", "/storages", body).await,
                StatusCode::CREATED,
            )
            .await;
        }
        for _ in 0..2 {
            if legacy_delete {
                deleted(api.legacy("DELETE", "/storages/vendor", Value::Null).await).await;
            } else {
                let result = payload(
                    api.command("storage.delete", json!({"id":"vendor"})).await,
                    StatusCode::OK,
                )
                .await;
                assert_eq!(
                    result["result"],
                    json!({"resource":"storage","id":"vendor"})
                );
            }
        }
        assert_eq!(
            api.same_read("/storages", "storage.list", json!({})).await,
            json!([])
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn storage_replacement_preserves_metadata_and_blocks_address_changes_on_both_paths(
    pool: PgPool,
) {
    let api = Pair::new(&pool).await;
    let provider = Provider::start(false).await;
    payload(
        api.command("storage.create", provider.input_for("vendor", 100))
            .await,
        StatusCode::OK,
    )
    .await;
    payload(
        api.command(
            "storage.metadata.replace",
            json!({"id":"vendor","metadata":{"env":"home"}}),
        )
        .await,
        StatusCode::OK,
    )
    .await;
    sqlx::raw_sql("INSERT INTO clients(id,storage_id) VALUES('app','vendor');
        WITH f AS (INSERT INTO files(client_id,state,declared_size) VALUES('app','pending',0) RETURNING id)
        INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'vendor',id::text FROM f;")
        .execute(&pool).await.unwrap();
    for legacy in [true, false] {
        let capacity = if legacy { 200 } else { 300 };
        let mut input = provider.input_for("vendor", capacity);
        input["spec"]["secret_key"] = json!("rotated-provider-secret");
        let updated = if legacy {
            payload(
                api.legacy("PUT", "/storages/vendor", input["spec"].clone())
                    .await,
                StatusCode::OK,
            )
            .await
        } else {
            payload(
                api.command("storage.replace", input.clone()).await,
                StatusCode::OK,
            )
            .await["result"]
                .clone()
        };
        assert_eq!(updated["capacity_bytes"], capacity);
        assert!(!updated.to_string().contains("rotated-provider-secret"));
        assert_eq!(
            api.same_read("/storages/vendor", "storage.show", json!({"id":"vendor"}))
                .await,
            updated
        );
        let metadata: Value = sqlx::query_scalar("SELECT metadata FROM storages WHERE id='vendor'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(metadata, json!({"env":"home"}));
        let before = filegate_db::registry::get_storage(&pool, "vendor")
            .await
            .unwrap();
        input["spec"]["bucket"] = json!("other-bucket");
        let rejected = if legacy {
            api.legacy("PUT", "/storages/vendor", input["spec"].clone())
                .await
        } else {
            api.command("storage.replace", input).await
        };
        payload(rejected, StatusCode::CONFLICT).await;
        assert_eq!(
            filegate_db::registry::get_storage(&pool, "vendor")
                .await
                .unwrap(),
            before
        );
    }
}
