use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{
    AccountChange, AuditContext, Credential, Error, Identity, NewAccount, NewCredential, accounts,
    credentials, identity, lock, sessions,
};

/// Hashes produced by the trusted transport; intentionally not Debug/Serialize.
#[derive(Clone, Copy)]
pub enum Proof<'a> {
    Session(&'a str),
    Token(&'a str),
}

/// Keeps identity changes fenced from authentication through the final commit.
/// The service owns policy checks; this type owns the transaction lifetime.
pub struct IdentityTransaction<'a> {
    pub(super) inner: Transaction<'a, Postgres>,
}

impl<'a> IdentityTransaction<'a> {
    pub async fn begin(pool: &'a PgPool) -> Result<Self, Error> {
        Ok(Self {
            inner: lock(pool).await?,
        })
    }

    pub async fn resolve(&mut self, proof: Proof<'_>) -> Result<Option<Identity>, Error> {
        match proof {
            Proof::Session(hash) => identity::session(&mut self.inner, hash).await,
            Proof::Token(hash) => identity::token(&mut self.inner, hash).await,
        }
    }

    pub async fn finish(self) -> Result<(), Error> {
        self.inner.commit().await.map_err(|_| Error::CommitUnknown)
    }

    pub async fn create_account(
        self,
        context: &AuditContext,
        account: NewAccount<'_>,
    ) -> Result<Uuid, Error> {
        accounts::create_in(self.inner, context, account).await
    }
    pub async fn change_account(
        self,
        context: &AuditContext,
        id: Uuid,
        change: AccountChange,
    ) -> Result<bool, Error> {
        accounts::change_in(self.inner, context, id, change).await
    }
    pub async fn issue_credential(
        self,
        context: &AuditContext,
        account: Uuid,
        key: &NewCredential<'_>,
    ) -> Result<Credential, Error> {
        credentials::issue_in(self.inner, context, account, key).await
    }
    pub async fn revoke_credential(self, context: &AuditContext, id: Uuid) -> Result<bool, Error> {
        credentials::revoke_in(self.inner, context, id).await
    }
    pub async fn revoke_session(
        self,
        context: &AuditContext,
        user: Uuid,
        session: Uuid,
    ) -> Result<bool, Error> {
        sessions::revoke_in(self.inner, context, user, session).await
    }
}
