#![allow(dead_code, clippy::unwrap_used)]
#[path = "../../../db/tests/support/management.rs"]
mod fixtures;
use filegate_db::{PgPool, management as db};
pub use fixtures::*;
use grove_management_policy::Role;
use uuid::Uuid;

#[allow(clippy::panic)]
pub async fn unexpected_storage_probe(
    _: grove_management_command::input::StorageInput,
) -> Result<filegate_db::registry::StorageRow, grove_management_service::Error> {
    panic!("this command must not probe a storage")
}

pub struct Login {
    pub account: Uuid,
    pub credential: Uuid,
    pub token: String,
    pub session: String,
    pub session_id: Uuid,
}
pub async fn owner(pool: &PgPool) -> Login {
    let (id, credential) = bootstrap(pool).await;
    login(pool, id, credential.id, 1).await
}
pub async fn user_login(pool: &PgPool, role: Role, seed: u64) -> Login {
    let id = user(pool, role).await;
    let credential = db::issue_credential(pool, &context(), id, &key(&hash(seed)))
        .await
        .unwrap();
    login(pool, id, credential.id, seed).await
}
async fn login(pool: &PgPool, id: Uuid, credential: Uuid, seed: u64) -> Login {
    let token = hash(seed);
    let session = hash(100_000 + seed);
    let row = db::create_session(pool, Uuid::new_v4(), &token, &session)
        .await
        .unwrap()
        .unwrap();
    Login {
        account: id,
        credential,
        token,
        session,
        session_id: row.id,
    }
}

pub async fn wait_for_identity_lock(pool: &PgPool) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event='advisory')")
                .fetch_one(pool).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.unwrap();
}
