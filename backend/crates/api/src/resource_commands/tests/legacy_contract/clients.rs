use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn client_creation_and_idempotent_deletion_interoperate_in_both_directions(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    for legacy_create in [true, false] {
        let input = json!({"id":"managed","storage_id":"vendor"});
        let created = if legacy_create {
            payload(
                api.legacy("POST", "/clients", input.clone()).await,
                StatusCode::CREATED,
            )
            .await
        } else {
            payload(
                api.command("client.create", input.clone()).await,
                StatusCode::OK,
            )
            .await["result"]
                .clone()
        };
        assert_eq!(created, input);
        assert_eq!(
            api.same_read("/clients/managed", "client.show", json!({"id":"managed"}))
                .await,
            input
        );
        assert_eq!(
            api.same_read("/clients", "client.list", json!({})).await,
            json!(["app", "managed"])
        );
        for _ in 0..2 {
            if legacy_create {
                let result = payload(
                    api.command("client.delete", json!({"id":"managed"})).await,
                    StatusCode::OK,
                )
                .await;
                assert_eq!(
                    result["result"],
                    json!({"resource":"client", "id":"managed"})
                );
            } else {
                deleted(api.legacy("DELETE", "/clients/managed", Value::Null).await).await;
            }
        }
        assert!(
            !filegate_db::registry::client_exists(&pool, "managed")
                .await
                .unwrap()
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn service_keys_keep_ownership_and_cascade_only_with_their_client(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    let key_hash = filegate_core::client_key_hash("fixture-native-key");
    for legacy_write in [true, false] {
        payload(
            api.command(
                "client.create",
                json!({"id":"managed","storage_id":"vendor"}),
            )
            .await,
            StatusCode::OK,
        )
        .await;
        if legacy_write {
            payload(
                api.legacy(
                    "POST",
                    "/clients/managed/keys",
                    json!({"key_hash":key_hash}),
                )
                .await,
                StatusCode::CREATED,
            )
            .await;
        } else {
            payload(
                api.command(
                    "client-key.register",
                    json!({"client_id":"managed","key_hash":key_hash}),
                )
                .await,
                StatusCode::OK,
            )
            .await;
        }
        let issued = if legacy_write {
            payload(
                api.legacy("POST", "/clients/managed/s3-credentials", json!({}))
                    .await,
                StatusCode::CREATED,
            )
            .await
        } else {
            payload(
                api.command("credential.create", json!({"client_id":"managed"}))
                    .await,
                StatusCode::OK,
            )
            .await["result"]
                .clone()
        };
        let access = issued["access_key_id"].as_str().unwrap();
        let secret = issued["secret_key"].as_str().unwrap();
        assert!(!secret.is_empty());
        let saved = filegate_db::s3_registry::get_credential(&pool, access)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.client_id, "managed");
        let crypto = crate::routes::tests::test_state().crypto;
        let encrypted = filegate_core::EncryptedSecret {
            ciphertext: saved.secret_ciphertext,
            nonce: saved.secret_nonce,
        };
        let restored = crypto
            .decrypt(&saved.enc_key_id, access, &encrypted)
            .unwrap();
        assert_eq!(
            filegate_core::ExposeSecret::expose_secret(&restored),
            secret
        );
        assert!(
            crypto
                .decrypt(&saved.enc_key_id, "different-key", &encrypted)
                .is_err()
        );
        assert_eq!(
            api.same_read(
                "/clients/managed/keys",
                "client-key.list",
                json!({"client_id":"managed"})
            )
            .await,
            json!([key_hash])
        );
        assert_eq!(
            api.same_read(
                "/clients/managed/s3-credentials",
                "credential.list",
                json!({"client_id":"managed"})
            )
            .await,
            json!([access])
        );

        // A valid key ID/hash under a different client must not revoke the owner key.
        deleted(
            api.legacy(
                "DELETE",
                &format!("/clients/app/s3-credentials/{access}"),
                Value::Null,
            )
            .await,
        )
        .await;
        payload(
            api.command(
                "credential.delete",
                json!({"client_id":"app","access_key_id":access}),
            )
            .await,
            StatusCode::OK,
        )
        .await;
        deleted(
            api.legacy(
                "DELETE",
                &format!("/clients/app/keys/{key_hash}"),
                Value::Null,
            )
            .await,
        )
        .await;
        payload(
            api.command(
                "client-key.delete",
                json!({"client_id":"app","key_hash":key_hash}),
            )
            .await,
            StatusCode::OK,
        )
        .await;
        assert_eq!(
            filegate_db::s3_registry::list_credentials(&pool, "managed")
                .await
                .unwrap(),
            [access]
        );
        assert_eq!(
            filegate_db::registry::list_client_keys(&pool, "managed")
                .await
                .unwrap(),
            [key_hash.as_str()]
        );

        if legacy_write {
            payload(
                api.command("client.delete", json!({"id":"managed"})).await,
                StatusCode::OK,
            )
            .await;
        } else {
            deleted(api.legacy("DELETE", "/clients/managed", Value::Null).await).await;
        }
        assert!(
            filegate_db::s3_registry::get_credential(&pool, access)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            filegate_db::registry::client_id_for_key_hash(&pool, &key_hash)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            filegate_db::s3_registry::list_credentials(&pool, "app")
                .await
                .unwrap(),
            ["testaccesskey"]
        );
    }
}
