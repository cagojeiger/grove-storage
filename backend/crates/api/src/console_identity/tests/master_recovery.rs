use super::master::*;
use super::*;

async fn data_snapshot(pool: &PgPool) -> Vec<Vec<String>> {
    let mut result = Vec::new();
    for table in [
        "storages",
        "clients",
        "client_keys",
        "s3_credentials",
        "files",
        "locations",
    ] {
        result.push(
            sqlx::query_scalar(&format!(
                "SELECT row_to_json(t)::text FROM {table} t ORDER BY row_to_json(t)::text"
            ))
            .fetch_all(pool)
            .await
            .unwrap(),
        );
    }
    result
}

#[sqlx::test(migrations = "../db/migrations")]
async fn targeted_recovery_replaces_only_selected_admin_credentials(pool: PgPool) {
    let (user, _, old_token) = account(&pool, Role::Admin).await;
    let (reader, _, reader_token) = account(&pool, Role::Reader).await;
    let (_, _, other_token) = account(&pool, Role::Admin).await;
    let (router, master_token) = setup(&pool).await;
    let old_cookie = cookie(&login(router.clone(), &old_token).await);
    sqlx::raw_sql("INSERT INTO storages(id,kind,endpoint,public_endpoint,region,bucket,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes)
        VALUES('existing','s3','https://provider.test','https://provider.test','test','bucket','provider-key',decode('deadbeef','hex'),decode(repeat('00',12),'hex'),'v1',1000);
        INSERT INTO clients(id,storage_id) VALUES('existing','existing');
        INSERT INTO client_keys(key_hash,client_id) VALUES('sha256:'||repeat('a',64),'existing');
        INSERT INTO s3_credentials(access_key_id,client_id,secret_key_ciphertext,secret_key_nonce,enc_key_id)
        VALUES('existingkey','existing',decode('abcd','hex'),decode(repeat('00',12),'hex'),'v1');
        INSERT INTO files(id,client_id,declared_size) VALUES('00000000-0000-0000-0000-000000000001','existing',1);
        INSERT INTO locations(file_id,storage_id,object_key) VALUES('00000000-0000-0000-0000-000000000001','existing','object');").execute(&pool).await.unwrap();
    let before = data_snapshot(&pool).await;
    let master_cookie = cookie(&sign_in(router.clone(), &master_token).await);
    assert_eq!(
        change(
            router.clone(),
            RECOVER,
            &master_cookie,
            serde_json::json!({"user_id":user,"confirm":false})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    for target in [reader, Uuid::new_v4()] {
        assert_eq!(
            change(
                router.clone(),
                RECOVER,
                &master_cookie,
                serde_json::json!({"user_id":target,"confirm":true})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        current(router.clone(), &old_cookie).await.status(),
        StatusCode::OK
    );
    let response = change(
        router.clone(),
        RECOVER,
        &master_cookie,
        serde_json::json!({"user_id":user,"confirm":true}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let id: Uuid = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let body = json(response).await;
    assert_eq!(body["user_id"], user.to_string());
    assert_eq!(
        current(router.clone(), &old_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(router.clone(), &old_token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for token in [body["token"].as_str().unwrap(), &reader_token, &other_token] {
        assert_eq!(login(router.clone(), token).await.status(), StatusCode::OK);
    }
    assert_eq!(
        change(
            router.clone(),
            RECOVER,
            &master_cookie,
            serde_json::json!({"user_id":user,"confirm":true})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(data_snapshot(&pool).await, before);
    let actor: String = sqlx::query_scalar("SELECT actor_kind FROM management.audit_events WHERE request_id=$1 AND action='user.recover'").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(actor, "master");
}
