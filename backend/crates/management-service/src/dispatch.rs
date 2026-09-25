use crate::{Command, Output};
use filegate_db::management::{
    AuditContext, Error, IdentityTransaction, ResolvedIdentity, history::HistoryScope,
};
use grove_management_policy::Scope;
use uuid::Uuid;

pub(super) async fn run(
    mut tx: IdentityTransaction<'_>,
    context: &AuditContext,
    identity: ResolvedIdentity,
    scope: Scope,
    command: Command<'_>,
) -> Result<Output, Error> {
    let user = match &identity {
        ResolvedIdentity::User(identity) => Some(identity.account_id),
        ResolvedIdentity::Root(_) => None,
    };
    let output = match command {
        Command::CurrentSession => match identity {
            ResolvedIdentity::User(identity) => Output::Identity(identity),
            ResolvedIdentity::Root(session) => Output::RootSession(session),
        },
        Command::Logout => {
            let ResolvedIdentity::User(identity) = identity else {
                let ResolvedIdentity::Root(session) = identity else {
                    return Err(Error::InvalidInput);
                };
                return tx
                    .revoke_root_session(context, session.id)
                    .await
                    .map(Output::Changed);
            };
            let id = identity.session_id.ok_or(Error::InvalidInput)?;
            return tx
                .revoke_session(context, identity.account_id, id)
                .await
                .map(Output::Changed);
        }
        Command::CreateAccount(account) => {
            return tx
                .create_account(context, account)
                .await
                .map(Output::Account);
        }
        Command::ChangeAccount { id, change } => {
            return tx
                .change_account(context, id, change)
                .await
                .map(Output::Changed);
        }
        Command::IssueCredential { account, key } => {
            return tx
                .issue_credential(context, account, &key)
                .await
                .map(Output::Credential);
        }
        Command::RevokeCredential(id) => {
            return tx.revoke_credential(context, id).await.map(Output::Changed);
        }
        Command::RevokeOwnSession(id) => {
            let Some(user) = user else {
                return tx
                    .revoke_root_session(context, id)
                    .await
                    .map(Output::Changed);
            };
            return tx
                .revoke_session(context, user, id)
                .await
                .map(Output::Changed);
        }
        Command::Accounts(page) => Output::Accounts(tx.accounts(page).await?),
        Command::Credentials { account, page } => {
            Output::Credentials(tx.credentials(account, page).await?)
        }
        Command::OwnSessions(page) => Output::Sessions(match user {
            Some(user) => tx.sessions(user, page).await?,
            None => tx.root_sessions(page).await?,
        }),
        Command::Audit(page) => {
            Output::Audit(tx.audit_history(history_scope(scope, user)?, page).await?)
        }
        Command::Invocations(page) => Output::Invocations(
            tx.invocation_history(history_scope(scope, user)?, page)
                .await?,
        ),
        Command::Security(page) => Output::Security(tx.security_history(page).await?),
    };
    tx.finish().await?;
    Ok(output)
}

fn history_scope(scope: Scope, user: Option<Uuid>) -> Result<HistoryScope, Error> {
    match scope {
        Scope::Installation => Ok(HistoryScope::Installation),
        Scope::SelfOnly => Ok(HistoryScope::User(user.ok_or(Error::InvalidInput)?)),
        _ => Err(Error::InvalidInput),
    }
}
