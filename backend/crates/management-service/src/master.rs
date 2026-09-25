//! Master access is confined to a short setup/recovery session, never a User.
use crate::{Error, logging};
use filegate_db::{
    PgPool,
    management::{self as db, master as store},
};
use grove_management_policy::{
    Action, Actor, AuthMethod, Caller, CredentialState, Surface, authorize,
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

// No Debug/Serialize: even verification material stays out of logs.
pub struct Config {
    generation: i64,
    hash: String,
}
impl Config {
    pub fn new(generation: i64, hash: String) -> Result<Self, Error> {
        if generation < 1
            || hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::InvalidInput);
        }
        Ok(Self { generation, hash })
    }
    pub fn binding(&self) -> store::Binding<'_> {
        store::Binding {
            generation: self.generation,
            token_hash: &self.hash,
        }
    }
    pub(crate) fn matches(&self, hash: Option<&str>) -> bool {
        hash.is_some_and(|value| bool::from(self.hash.as_bytes().ct_eq(value.as_bytes())))
    }
    /// Called at process startup with trusted configuration, not from HTTP.
    pub async fn install(&self, pool: &PgPool) -> Result<bool, Error> {
        store::configure(pool, self.binding(), Uuid::new_v4())
            .await
            .map_err(Error::from)
    }
}

pub struct Login {
    pub request_id: Uuid,
    pub result: Result<store::Session, Error>,
}
pub async fn login(
    pool: &PgPool,
    config: &Config,
    presented_hash: Option<&str>,
    session_hash: &str,
) -> Login {
    let request_id = Uuid::new_v4();
    let started = std::time::Instant::now();
    let result = exchange(pool, config, presented_hash, session_hash, request_id).await;
    let context = result
        .as_ref()
        .ok()
        .map(|s| store::context(request_id, s.id));
    logging::record(
        pool,
        context.as_ref(),
        request_id,
        Surface::Console,
        "master.session.login",
        started.elapsed(),
        &result,
    )
    .await;
    if result.is_ok() {
        logging::security(
            pool,
            context.as_ref(),
            request_id,
            Surface::Console,
            db::telemetry::SecurityReason::Authenticated,
        )
        .await;
    }
    Login { request_id, result }
}

async fn exchange(
    pool: &PgPool,
    config: &Config,
    presented_hash: Option<&str>,
    hash: &str,
    id: Uuid,
) -> Result<store::Session, Error> {
    if !db::telemetry::login_allowed(pool)
        .await
        .map_err(Error::from)?
    {
        return Err(Error::RateLimited);
    }
    let valid = presented_hash
        .is_some_and(|value| bool::from(config.hash.as_bytes().ct_eq(value.as_bytes())));
    if !valid {
        return Err(Error::Unauthenticated);
    }
    store::create_session(pool, config.binding(), hash, id)
        .await
        .map_err(Error::from)
}

pub enum Command<'a> {
    Current,
    Logout,
    Bootstrap {
        name: &'a str,
        key: db::NewCredential<'a>,
    },
    Recover {
        account: Uuid,
        key: db::NewCredential<'a>,
    },
}
impl Command<'_> {
    fn name(&self) -> &'static str {
        match self {
            Self::Current => "master.session.current",
            Self::Logout => "master.session.logout",
            Self::Bootstrap { .. } => "master.bootstrap",
            Self::Recover { .. } => "master.recover",
        }
    }
    fn action(&self) -> Action {
        match self {
            Self::Current | Self::Logout => Action::ManageSetupSession,
            Self::Bootstrap { .. } => Action::BootstrapAdmin,
            Self::Recover { .. } => Action::RecoverAdmin,
        }
    }
}

#[derive(Debug)]
pub enum Output {
    Current {
        session: store::Session,
        initialized: bool,
    },
    LoggedOut,
    Credential(db::Credential),
}
pub struct Execution {
    pub request_id: Uuid,
    pub result: Result<Output, Error>,
}

pub async fn execute(
    pool: &PgPool,
    config: &Config,
    hash: &str,
    command: Command<'_>,
) -> Execution {
    let id = Uuid::new_v4();
    let started = std::time::Instant::now();
    let name = command.name();
    let read = matches!(command, Command::Current);
    let (context, result) = run(pool, config, hash, command, id).await;
    let result = if read && matches!(result, Err(Error::OutcomeUnknown)) {
        Err(Error::Unavailable)
    } else {
        result
    };
    logging::record(
        pool,
        context.as_ref(),
        id,
        Surface::Console,
        name,
        started.elapsed(),
        &result,
    )
    .await;
    Execution {
        request_id: id,
        result,
    }
}

async fn run(
    pool: &PgPool,
    config: &Config,
    hash: &str,
    command: Command<'_>,
    id: Uuid,
) -> (Option<db::AuditContext>, Result<Output, Error>) {
    let mut tx = match db::IdentityTransaction::begin(pool).await {
        Ok(tx) => tx,
        Err(e) => return (None, Err(e.into())),
    };
    let session = match tx.master_session(config.binding(), hash).await {
        Ok(Some(session)) => session,
        Ok(None) => return (None, Err(Error::Unauthenticated)),
        Err(e) => return (None, Err(e.into())),
    };
    let context = store::context(id, session.id);
    let caller = Caller {
        actor: Actor::Master,
        method: AuthMethod::MasterSession,
        credential_state: CredentialState::Active,
    };
    if authorize(caller, Surface::Console, command.action()).is_err() {
        return (Some(context), Err(Error::Forbidden));
    }
    let result = dispatch(tx, &context, session, command)
        .await
        .map_err(Error::from);
    (Some(context), result)
}

async fn dispatch(
    mut tx: db::IdentityTransaction<'_>,
    context: &db::AuditContext,
    session: store::Session,
    command: Command<'_>,
) -> Result<Output, db::Error> {
    if matches!(command, Command::Current) {
        let initialized = tx.management_initialized().await?;
        tx.finish().await?;
        return Ok(Output::Current {
            session,
            initialized,
        });
    }
    // Consuming the session and changing credentials share one commit. Retrying
    // a successful setup/recovery cannot silently replace the delivered key.
    tx.consume_master(context, session.id).await?;
    match command {
        Command::Bootstrap { name, key } => tx
            .bootstrap_admin(context, name, &key)
            .await
            .map(Output::Credential),
        Command::Recover { account, key } => tx
            .recover_admin(context, account, &key)
            .await
            .map(Output::Credential),
        Command::Logout => {
            tx.finish().await?;
            Ok(Output::LoggedOut)
        }
        Command::Current => Err(db::Error::InvalidInput),
    }
}
