use super::*;
use filegate_core::{EncryptedSecret, ExposeSecret};

#[sqlx::test(migrations = "../db/migrations")]
async fn all_six_mutations_work_with_same_results_for_cli_and_mcp(pool: PgPool) {
    owner(&pool).await;
    let operator = operator(&pool, Role::Operator, 2).await;
    seed(&pool).await;
    for (surface, id) in [(Surface::Cli, "cli-client"), (Surface::Mcp, "mcp-client")] {
        let before = audit_count(&pool).await;
        let execution = resources::execute(
            &pool,
            &crypto(),
            Proof::Token(&operator.token),
            surface,
            Command::ClientCreate(input::ClientCreateInput {
                id: id.into(),
                storage_id: "local".into(),
            }),
        )
        .await;
        assert!(
            matches!(execution.result.unwrap(), Output::ClientCreate(row) if row.id == id && row.storage_id == "local")
        );
        let hash = format!("sha256:{}", "c".repeat(64));
        assert!(
            resources::execute(
                &pool,
                &crypto(),
                Proof::Token(&operator.token),
                surface,
                Command::ClientKeyRegister(input::ClientKeyInput {
                    client_id: id.into(),
                    key_hash: hash.clone()
                })
            )
            .await
            .result
            .is_ok()
        );
        let issued = resources::execute(
            &pool,
            &crypto(),
            Proof::Token(&operator.token),
            surface,
            Command::CredentialCreate(input::ClientInput {
                client_id: id.into(),
            }),
        )
        .await;
        let output = issued.result.unwrap();
        assert!(matches!(output, Output::CredentialCreate(_)));
        if let Output::CredentialCreate(key) = output {
            let saved = filegate_db::s3_registry::get_credential(&pool, &key.access_key_id)
                .await
                .unwrap()
                .unwrap();
            let plaintext = crypto()
                .decrypt(
                    &saved.enc_key_id,
                    &key.access_key_id,
                    &EncryptedSecret {
                        ciphertext: saved.secret_ciphertext,
                        nonce: saved.secret_nonce,
                    },
                )
                .unwrap();
            assert_eq!(plaintext.expose_secret(), &key.secret_key);
            for table in ["audit_events", "command_invocations", "security_events"] {
                let rows: Vec<String> = sqlx::query_scalar(&format!(
                    "SELECT row_to_json(t)::text FROM management.{table} t"
                ))
                .fetch_all(&pool)
                .await
                .unwrap();
                assert!(rows.iter().all(|r| !r.contains(&key.secret_key)
                    && !r.contains(&hash)
                    && !r.contains(&operator.token)));
            }
            assert!(
                resources::execute(
                    &pool,
                    &crypto(),
                    Proof::Token(&operator.token),
                    surface,
                    Command::CredentialDelete(input::CredentialDeleteInput {
                        client_id: id.into(),
                        access_key_id: key.access_key_id
                    })
                )
                .await
                .result
                .is_ok()
            );
        }
        assert!(
            resources::execute(
                &pool,
                &crypto(),
                Proof::Token(&operator.token),
                surface,
                Command::ClientKeyDelete(input::ClientKeyInput {
                    client_id: id.into(),
                    key_hash: hash
                })
            )
            .await
            .result
            .is_ok()
        );
        assert!(
            resources::execute(
                &pool,
                &crypto(),
                Proof::Token(&operator.token),
                surface,
                Command::ClientDelete(input::ResourceInput { id: id.into() })
            )
            .await
            .result
            .is_ok()
        );
        assert_eq!(audit_count(&pool).await, before + 6);
        let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM management.audit_events WHERE request_id=$1),(SELECT count(*) FROM management.command_invocations WHERE request_id=$1)")
            .bind(issued.request_id).fetch_one(&pool).await.unwrap();
        assert_eq!(counts, (1, 1));
    }
    assert_eq!(resource_counts(&pool).await, (1, 1, 1));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn deletion_respects_client_scope_and_skips_duplicate_audit(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    for command in [
        Command::ClientKeyDelete(input::ClientKeyInput {
            client_id: "other".into(),
            key_hash: native_hash(),
        }),
        Command::CredentialDelete(input::CredentialDeleteInput {
            client_id: "other".into(),
            access_key_id: "existingkey".into(),
        }),
    ] {
        assert!(execute(&pool, &admin.token, command).await.result.is_ok());
    }
    assert_eq!(resource_counts(&pool).await, (1, 1, 1));
    assert_eq!(audit_count(&pool).await, before);
    for _ in 0..2 {
        assert!(
            execute(
                &pool,
                &admin.token,
                Command::ClientDelete(input::ResourceInput { id: "app".into() })
            )
            .await
            .result
            .is_ok()
        );
    }
    assert_eq!(resource_counts(&pool).await, (0, 0, 0));
    assert_eq!(audit_count(&pool).await, before + 1);
}
