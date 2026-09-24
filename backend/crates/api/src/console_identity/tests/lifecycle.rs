use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn login_current_logout_and_secret_free_correlated_history(pool: PgPool) {
    let (user, credential, token) = account(&pool, Role::Reader).await;
    let response = login(app(&pool), &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    let request_id: Uuid = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let set = response.headers()[header::SET_COOKIE].to_str().unwrap();
    for flag in [
        "Secure",
        "HttpOnly",
        "SameSite=Strict",
        "Path=/",
        "Max-Age=",
    ] {
        assert!(set.contains(flag));
    }
    assert!(!set.contains("Domain="));
    let cookie = cookie(&response);
    let raw = cookie.split_once('=').unwrap().1;
    let body = json(response).await;
    let session: Uuid = body["session_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(body["user_id"], user.to_string());
    assert_eq!(body["credential_id"], credential.to_string());
    assert!(!body.to_string().contains(raw));
    let row: (String, bool, bool) = sqlx::query_as("SELECT session_hash,expires_at<=clock_timestamp()+interval '8 hours',expires_at>clock_timestamp()+interval '7 hours' FROM management.sessions WHERE id=$1")
        .bind(session).fetch_one(&pool).await.unwrap();
    assert_eq!(row, (secrets::session_hash(raw), true, true));
    let used: bool = sqlx::query_scalar(
        "SELECT last_used_at IS NOT NULL FROM management.credentials WHERE id=$1",
    )
    .bind(credential)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(used);
    for table in ["audit_events", "command_invocations", "security_events"] {
        let rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT row_to_json(t)::text FROM management.{table} t WHERE request_id=$1"
        ))
        .bind(request_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "{table}");
        assert!(rows[0].contains(&session.to_string()));
        if table == "security_events" {
            assert!(rows[0].contains("authentication_succeeded"));
        }
        for secret in [
            &token,
            raw,
            &secrets::token_hash(&token),
            &secrets::session_hash(raw),
        ] {
            assert!(!rows[0].contains(secret));
        }
    }
    let response = current(app(&pool), &cookie).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["role"], "reader");
    db::change_account(
        &pool,
        &context(),
        user,
        db::AccountChange::Role(Role::Writer),
    )
    .await
    .unwrap();
    assert_eq!(
        json(current(app(&pool), &cookie).await).await["role"],
        "writer"
    );
    let other = login(app(&pool), &token).await;
    let other_cookie = super::cookie(&other);
    assert_ne!(cookie, other_cookie);
    let response = logout(app(&pool), &cookie).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        current(app(&pool), &cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        current(app(&pool), &other_cookie).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        logout(app(&pool), &cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.audit_events WHERE action='session.revoke' AND resource_id=$1")
        .bind(session.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn invalid_credentials_and_expiry_never_become_console_users(pool: PgPool) {
    let (user, _, _) = account(&pool, Role::Writer).await;
    for token in [
        format!("gsm_{}", filegate_core::generate_url_secret()),
        format!("fgop_{}", filegate_core::generate_url_secret()),
        String::new(),
    ] {
        let response = login(app(&pool), &token).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(json(response).await["error"], "unauthenticated");
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let (id, token) = credential(&pool, user).await;
    sqlx::query("UPDATE management.credentials SET expires_at=clock_timestamp()+interval '30 minutes' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    let response = login(app(&pool), &token).await;
    let cookie = cookie(&response);
    let bounded: bool = sqlx::query_scalar("SELECT s.expires_at=c.expires_at FROM management.sessions s JOIN management.credentials c ON c.id=s.credential_id WHERE c.id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert!(bounded);
    sqlx::query("UPDATE management.credentials SET created_at=clock_timestamp()-interval '2 days',expires_at=clock_timestamp()-interval '1 day' WHERE id=$1")
        .bind(id).execute(&pool).await.unwrap();
    assert_eq!(
        current(app(&pool), &cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(app(&pool), &token).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn source_revocation_account_disabling_and_session_expiry_take_effect(pool: PgPool) {
    for change in ["revoke", "disable", "expire-session"] {
        let (user, id, token) = account(&pool, Role::Reader).await;
        let response = login(app(&pool), &token).await;
        let cookie = cookie(&response);
        match change {
            "revoke" => {
                db::revoke_credential(&pool, &context(), id).await.unwrap();
            }
            "disable" => {
                db::change_account(&pool, &context(), user, db::AccountChange::Active(false))
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("UPDATE management.sessions SET created_at=clock_timestamp()-interval '2 days',expires_at=clock_timestamp()-interval '1 day' WHERE credential_id=$1")
                .bind(id).execute(&pool).await.unwrap();
            }
        }
        assert_eq!(
            current(app(&pool), &cookie).await.status(),
            StatusCode::UNAUTHORIZED,
            "{change}"
        );
        let expected = if change == "expire-session" {
            StatusCode::OK
        } else {
            StatusCode::UNAUTHORIZED
        };
        assert_eq!(
            login(app(&pool), &token).await.status(),
            expected,
            "{change}"
        );
    }
}
