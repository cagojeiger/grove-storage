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
    RootSession {
        hash: &'a str,
        binding: super::master::Binding<'a>,
    },
}

pub enum ResolvedIdentity {
    User(Identity),
    Root(super::master::Session),
}
impl ResolvedIdentity {
    pub fn caller(&self) -> grove_management_policy::Caller {
        use grove_management_policy::{Actor, AuthMethod, Caller, CredentialState};
        match self {
            Self::User(identity) => identity.caller,
            Self::Root(_) => Caller {
                actor: Actor::Root,
                method: AuthMethod::RootSession,
                credential_state: CredentialState::Active,
            },
        }
    }
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

    pub async fn resolve(&mut self, proof: Proof<'_>) -> Result<Option<ResolvedIdentity>, Error> {
        match proof {
            Proof::Session(hash) => Ok(identity::session(&mut self.inner, hash)
                .await?
                .map(ResolvedIdentity::User)),
            Proof::Token(hash) => Ok(identity::token(&mut self.inner, hash)
                .await?
                .map(ResolvedIdentity::User)),
            Proof::RootSession { hash, binding } => Ok(self
                .root_session(binding, hash)
                .await?
                .map(ResolvedIdentity::Root)),
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
