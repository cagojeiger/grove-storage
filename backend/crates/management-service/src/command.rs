use filegate_db::management::{AccountChange, NewAccount, NewCredential, queries::Page};
use grove_management_policy::Action;
use uuid::Uuid;

/// Internal console operations, deliberately separate from the CLI/MCP catalog.
pub enum Command<'a> {
    CreateAccount(NewAccount<'a>),
    ChangeAccount {
        id: Uuid,
        change: AccountChange,
    },
    IssueCredential {
        account: Uuid,
        key: NewCredential<'a>,
    },
    RevokeCredential(Uuid),
    RevokeOwnSession(Uuid),
    CurrentSession,
    Logout,
    Accounts(Page<Uuid>),
    Credentials {
        account: Uuid,
        page: Page<Uuid>,
    },
    OwnSessions(Page<Uuid>),
    Audit(Page<i64>),
    Invocations(Page<i64>),
    Security(Page<i64>),
}

impl Command<'_> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::CreateAccount(_) => "identity.account.create",
            Self::ChangeAccount {
                change: AccountChange::Role(_),
                ..
            } => "identity.account.role",
            Self::ChangeAccount {
                change: AccountChange::Active(_),
                ..
            } => "identity.account.active",
            Self::ChangeAccount {
                change: AccountChange::Delete,
                ..
            } => "identity.account.delete",
            Self::IssueCredential { .. } => "identity.credential.issue",
            Self::RevokeCredential(_) => "identity.credential.revoke",
            Self::RevokeOwnSession(_) => "identity.session.revoke",
            Self::CurrentSession => "identity.session.current",
            Self::Logout => "identity.session.logout",
            Self::Accounts(_) => "identity.account.list",
            Self::Credentials { .. } => "identity.credential.list",
            Self::OwnSessions(_) => "identity.session.list",
            Self::Audit(_) => "history.audit.list",
            Self::Invocations(_) => "history.invocation.list",
            Self::Security(_) => "history.security.list",
        }
    }
    pub fn action(&self) -> Action {
        match self {
            Self::CreateAccount(_)
            | Self::ChangeAccount { .. }
            | Self::IssueCredential { .. }
            | Self::RevokeCredential(_) => Action::ManageIdentities,
            Self::RevokeOwnSession(_) | Self::Logout => Action::RevokeOwnSessions,
            Self::Accounts(_) | Self::Credentials { .. } => Action::ReadIdentities,
            Self::OwnSessions(_) | Self::CurrentSession => Action::ReadOwnSessions,
            Self::Audit(_) => Action::ReadAuditHistory,
            Self::Invocations(_) => Action::ReadInvocationHistory,
            Self::Security(_) => Action::ReadSecurityEvents,
        }
    }
    pub fn is_mutation(&self) -> bool {
        matches!(
            self.action(),
            Action::ManageIdentities | Action::RevokeOwnSessions
        )
    }
}
