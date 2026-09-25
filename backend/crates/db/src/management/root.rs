//! Config-owned Root has no mutable account row or issued User credentials.
use super::{
    Error, IdentityTransaction, audit,
    master::{Binding, Session},
};
use uuid::Uuid;

impl IdentityTransaction<'_> {
    pub async fn root_sessions(
        &mut self,
        page: super::queries::Page<Uuid>,
    ) -> Result<Vec<super::queries::SessionSummary>, Error> {
        Ok(sqlx::query_as("SELECT id,NULL::uuid AS credential_id,created_at,expires_at,revoked_at FROM management.root_sessions WHERE ($1::uuid IS NULL OR id<$1) ORDER BY id DESC LIMIT $2")
            .bind(page.before).bind(page.limit).fetch_all(&mut *self.inner).await?)
    }
    pub async fn root_session(
        &mut self,
        binding: Binding<'_>,
        hash: &str,
    ) -> Result<Option<Session>, Error> {
        self.master_binding(binding).await?;
        Ok(sqlx::query_as("SELECT id,expires_at FROM management.root_sessions WHERE session_hash=$1 AND generation=$2 AND revoked_at IS NULL AND expires_at>clock_timestamp()")
            .bind(hash).bind(binding.generation).fetch_optional(&mut *self.inner).await?)
    }

    pub async fn create_root_session(
        mut self,
        binding: Binding<'_>,
        hash: &str,
        request_id: Uuid,
    ) -> Result<Session, Error> {
        self.master_binding(binding).await?;
        sqlx::query("DELETE FROM management.root_sessions WHERE expires_at<=clock_timestamp() OR revoked_at IS NOT NULL")
            .execute(&mut *self.inner).await?;
        let evicted: Vec<Uuid> = sqlx::query_scalar("UPDATE management.root_sessions SET revoked_at=clock_timestamp() WHERE id IN (SELECT id FROM management.root_sessions WHERE revoked_at IS NULL ORDER BY created_at DESC,id DESC OFFSET 7) RETURNING id")
            .fetch_all(&mut *self.inner).await?;
        let session: Session = sqlx::query_as("INSERT INTO management.root_sessions(id,session_hash,generation,expires_at) VALUES($1,$2,$3,clock_timestamp()+interval '30 minutes') RETURNING id,expires_at")
            .bind(Uuid::new_v4()).bind(hash).bind(binding.generation).fetch_one(&mut *self.inner).await?;
        let context = super::master::context(request_id, session.id);
        for id in evicted {
            audit::record(
                &mut self.inner,
                &context,
                "root.session.evict",
                "session",
                id,
            )
            .await?;
        }
        audit::record(
            &mut self.inner,
            &context,
            "root.session.create",
            "session",
            session.id,
        )
        .await?;
        self.finish().await?;
        Ok(session)
    }

    pub async fn revoke_root_session(
        mut self,
        context: &super::AuditContext,
        id: Uuid,
    ) -> Result<bool, Error> {
        let changed = sqlx::query("UPDATE management.root_sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND revoked_at IS NULL")
            .bind(id).execute(&mut *self.inner).await?.rows_affected() == 1;
        if changed {
            audit::record(
                &mut self.inner,
                context,
                "root.session.revoke",
                "session",
                id,
            )
            .await?;
        }
        self.finish().await?;
        Ok(changed)
    }
}
