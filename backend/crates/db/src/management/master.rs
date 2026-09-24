//! Configuration fencing and short, single-use setup/recovery sessions.
use super::{
    AuditActor, AuditContext, Credential, Error, IdentityTransaction, NewCredential, accounts,
    audit, credentials, lock,
};
use chrono::{DateTime, Utc};
use grove_management_policy::Surface;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct Binding<'a> {
    pub generation: i64,
    pub token_hash: &'a str,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// Trusted process configuration only. Higher generations fence older replicas;
/// HTTP requests never install or advance configuration.
pub async fn configure(
    pool: &PgPool,
    binding: Binding<'_>,
    request_id: Uuid,
) -> Result<bool, Error> {
    let mut tx = lock(pool).await?;
    let old: Option<(i64, String)> = sqlx::query_as(
        "SELECT generation,token_hash FROM management.master_configuration WHERE id=1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((generation, hash)) = old
        && generation >= binding.generation
    {
        return Ok(generation == binding.generation && hash == binding.token_hash);
    }
    sqlx::query("INSERT INTO management.master_configuration(id,generation,token_hash) VALUES(1,$1,$2)
        ON CONFLICT(id) DO UPDATE SET generation=EXCLUDED.generation,token_hash=EXCLUDED.token_hash")
        .bind(binding.generation).bind(binding.token_hash).execute(&mut *tx).await?;
    sqlx::query("UPDATE management.sessions SET revoked_at=clock_timestamp() WHERE auth_method='master' AND revoked_at IS NULL")
        .execute(&mut *tx).await?;
    sqlx::query("INSERT INTO management.audit_events(actor_kind,request_id,surface,action,resource_type,resource_id,metadata)
        VALUES('system',$1,'console','master.configuration.activate','master_configuration','1',jsonb_build_object('generation',$2::bigint))")
        .bind(request_id).bind(binding.generation).execute(&mut *tx).await?;
    tx.commit().await.map_err(|_| Error::CommitUnknown)?;
    Ok(true)
}

pub async fn initialized(pool: &PgPool) -> Result<bool, Error> {
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts)")
            .fetch_one(pool)
            .await?,
    )
}

pub async fn create_session(
    pool: &PgPool,
    binding: Binding<'_>,
    hash: &str,
    request_id: Uuid,
) -> Result<Session, Error> {
    let mut tx = IdentityTransaction::begin(pool).await?;
    tx.master_binding(binding).await?;
    sqlx::query("DELETE FROM management.sessions WHERE auth_method='master' AND (expires_at<=clock_timestamp() OR revoked_at IS NOT NULL)")
        .execute(&mut *tx.inner).await?;
    let evicted: Vec<Uuid> = sqlx::query_scalar("UPDATE management.sessions SET revoked_at=clock_timestamp() WHERE id IN
        (SELECT id FROM management.sessions WHERE auth_method='master' AND revoked_at IS NULL ORDER BY created_at DESC,id DESC OFFSET 7) RETURNING id")
        .fetch_all(&mut *tx.inner).await?;
    let session: Session = sqlx::query_as(
        "INSERT INTO management.sessions(id,session_hash,auth_method,master_generation,created_at,expires_at)
        VALUES($1,$2,'master',$3,statement_timestamp(),statement_timestamp()+interval '10 minutes') RETURNING id,expires_at",
    )
    .bind(Uuid::new_v4())
    .bind(hash)
    .bind(binding.generation.to_string())
    .fetch_one(&mut *tx.inner)
    .await?;
    let context = context(request_id, session.id);
    for id in evicted {
        audit::record(
            &mut tx.inner,
            &context,
            "master.session.evict",
            "session",
            id,
        )
        .await?;
    }
    audit::record(
        &mut tx.inner,
        &context,
        "master.session.create",
        "session",
        session.id,
    )
    .await?;
    tx.finish().await?;
    Ok(session)
}

pub fn context(request_id: Uuid, session_id: Uuid) -> AuditContext {
    AuditContext {
        actor: AuditActor::Master {
            session_id: Some(session_id),
        },
        request_id,
        surface: Surface::Console,
    }
}

impl IdentityTransaction<'_> {
    async fn master_binding(&mut self, binding: Binding<'_>) -> Result<(), Error> {
        let matches: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.master_configuration WHERE id=1 AND generation=$1 AND token_hash=$2)")
            .bind(binding.generation).bind(binding.token_hash).fetch_one(&mut *self.inner).await?;
        if !matches {
            return Err(Error::MasterConfigurationMismatch);
        }
        Ok(())
    }

    pub async fn master_session(
        &mut self,
        binding: Binding<'_>,
        hash: &str,
    ) -> Result<Option<Session>, Error> {
        self.master_binding(binding).await?;
        Ok(sqlx::query_as("SELECT id,expires_at FROM management.sessions WHERE session_hash=$1 AND auth_method='master'
            AND master_generation=$2 AND revoked_at IS NULL AND expires_at>clock_timestamp()")
            .bind(hash).bind(binding.generation.to_string()).fetch_optional(&mut *self.inner).await?)
    }

    pub async fn management_initialized(&mut self) -> Result<bool, Error> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.accounts)")
                .fetch_one(&mut *self.inner)
                .await?,
        )
    }

    pub async fn consume_master(
        &mut self,
        context: &AuditContext,
        session: Uuid,
    ) -> Result<(), Error> {
        let changed = sqlx::query("UPDATE management.sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND auth_method='master' AND revoked_at IS NULL")
            .bind(session).execute(&mut *self.inner).await?.rows_affected();
        if changed != 1 {
            return Err(Error::NotFound);
        }
        audit::record(
            &mut self.inner,
            context,
            "master.session.revoke",
            "session",
            session,
        )
        .await?;
        Ok(())
    }

    pub async fn bootstrap_admin(
        self,
        context: &AuditContext,
        name: &str,
        key: &NewCredential<'_>,
    ) -> Result<Credential, Error> {
        accounts::bootstrap_in(self.inner, context, name, key)
            .await
            .map(|(_, key)| key)
    }

    pub async fn recover_admin(
        self,
        context: &AuditContext,
        account: Uuid,
        key: &NewCredential<'_>,
    ) -> Result<Credential, Error> {
        credentials::recover_in(self.inner, context, account, key).await
    }
}
