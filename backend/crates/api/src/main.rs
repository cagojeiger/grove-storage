//! Grove Storage 진입점: env 설정 → PostgreSQL(+마이그레이션) → storage 재검증
//! → HTTP + reconciler → graceful shutdown.

mod accounts;
mod commands;
mod console_web;
mod cors;
mod error;
mod lease;
mod lease_relay;
mod local_accounts;
mod log_retention;
mod logging;
mod mcp;
mod native;
mod openapi;
mod reconciler;
mod routes;
mod s3;
mod shutdown;
mod spool;
mod status;
mod storage_access;
mod storage_registration;
#[cfg(test)]
#[path = "../../db/tests/support/management.rs"]
mod test_management;

use std::io;
use std::sync::Arc;

use grove_core::{ExposeSecret, LogFormat};
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    match std::env::args().nth(1).as_deref() {
        None | Some("serve") => {
            serve().await?;
            Ok(std::process::ExitCode::SUCCESS)
        }
        Some("status") => status::run().await,
        Some("account") => local_accounts::run().await,
        Some("openapi") => {
            println!("{}", serde_json::to_string_pretty(&openapi::documents())?);
            Ok(std::process::ExitCode::SUCCESS)
        }
        Some("--help") | Some("-h") | Some("help") => {
            print_usage();
            Ok(std::process::ExitCode::SUCCESS)
        }
        Some(other) => {
            eprintln!("grove-storage: unknown command '{other}'");
            print_usage();
            Ok(std::process::ExitCode::from(2))
        }
    }
}

fn print_usage() {
    eprintln!(
        "Grove Storage — S3 gateway\n\n\
         USAGE:\n    \
         grove-storage [serve]   Start the server (default)\n    \
         grove-storage status    Inspect local database and storage access\n    \
         grove-storage account   Initialize or recover a local password account\n    \
         grove-storage openapi   Export API contracts (no database required)\n\n\
         Remote resource management and CLI updates: gscli --help\n\
         Server configuration: GROVE_* environment variables"
    );
}

/// 서버 기동: env 설정 → PostgreSQL(+마이그레이션) → storage 재검증
/// → HTTP + object reconciler + management maintenance → graceful shutdown.
async fn serve() -> anyhow::Result<()> {
    let config = grove_core::Config::load()?;
    let console_origin = console_web::parse_origin(std::env::var("GROVE_CONSOLE_ORIGIN").ok())?;
    if let Some(origin) = &console_origin {
        console_web::validate_object_host(origin, config.server.public_url.as_deref())?;
    }
    let console_web = match (&console_origin, std::env::var_os("GROVE_CONSOLE_DIST_DIR")) {
        (Some(_), Some(directory)) => Some(console_web::ConsoleWeb::load(directory.into()).await?),
        _ => None,
    };
    init_tracing(config.server.log_format);
    grove_infra::temp_spool::prepare_root(&spool::spool_root()).await?;

    // 암호기 조립이 부팅 첫머리다 — 루트 길이·중복 key_id 오설정을 여기서 잡는다.
    let crypto = Arc::new(config.security.crypto()?);

    // 시그널 핸들러는 부팅 초기에 설치한다. 설치가 실패하면 graceful
    // shutdown이 불가능한 프로세스가 되므로 부팅 자체를 중단한다.
    let mut signals = ShutdownSignals::install()?;

    let pool = grove_db::connect(
        config.database.url.expose_secret(),
        config.database.max_connections,
    )
    .await?;
    grove_db::migrate(&pool).await?;
    anyhow::ensure!(
        grove_db::management::passwords::initialized(&pool)
            .await
            .map_err(|_| anyhow::anyhow!("management initialization check failed"))?,
        "administrator not initialized; run grove-storage account init"
    );
    info!(
        event = "db.connected",
        max_connections = config.database.max_connections
    );

    // 등록된 storage 접근 재검증 — 실패하면 부팅 중단 (ADR 001).
    storage_registration::verify_registered(&pool, &crypto).await?;

    let maintenance_pool = grove_db::connect(
        config.database.url.expose_secret(),
        log_retention::DB_CONNECTIONS,
    )
    .await?;

    let listener = tokio::net::TcpListener::bind(config.server.bind_addr).await?;
    info!(event = "server.listening", addr = %config.server.bind_addr);

    let shutdown = CancellationToken::new();
    // 요청 경로와 reconciler가 같은 캐시를 공유한다 — 같은 storage의 웜 풀.
    let clock: Arc<dyn grove_core::time::Clock> = Arc::new(grove_core::time::SystemClock);
    let s3_clients = Arc::new(grove_infra::S3ClientCache::new(clock.clone()));
    let mut workers = tokio::task::JoinSet::new();
    let tick = std::time::Duration::from_secs(config.server.reconciler_interval_secs);
    workers.spawn(reconciler::run(
        pool.clone(),
        crypto.clone(),
        s3_clients.clone(),
        tick,
        shutdown.clone(),
        clock.clone(),
    ));
    workers.spawn(log_retention::run(
        maintenance_pool.clone(),
        config.server.management_log_retention,
        tick,
        shutdown.clone(),
    ));

    let state = routes::AppState {
        clock,
        pool: pool.clone(),
        crypto,
        public_url: config.server.public_url.clone(),
        console_origin,
        console_web,
        multipart_threshold: config.server.multipart_threshold_bytes,
        part_size: config.server.part_size_bytes,
        s3_clients,
        single_upload_claims: std::sync::Arc::new(tokio::sync::Semaphore::new(
            lease_relay::SINGLE_UPLOAD_CLAIM_LIMIT,
        )),
        spool_budget: Arc::new(spool::SpoolBudget::new(config.server.spool_budget_bytes)),
    };

    let http_shutdown = shutdown.clone().cancelled_owned();
    let app = routes::app(state, &config.server.s3_cors_allowed_origins);
    let server = async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(http_shutdown)
            .await
    };
    tokio::pin!(server);

    // 서버가 스스로 끝나거나(에러), 종료 시그널이 오거나.
    let server_result: Option<io::Result<()>> = tokio::select! {
        result = &mut server => Some(result),
        () = signals.wait() => None,
    };

    info!(event = "server.shutting_down");
    shutdown.cancel();

    shutdown::drain(
        async {
            match server_result {
                Some(result) => result,
                None => server.await,
            }
        },
        workers,
        || async {
            tokio::join!(pool.close(), maintenance_pool.close());
        },
        shutdown::GRACE_PERIOD,
    )
    .await?;
    info!(event = "shutdown.complete");
    Ok(())
}

/// SIGINT(Ctrl-C)와 SIGTERM(컨테이너 종료)을 함께 기다린다.
struct ShutdownSignals {
    #[cfg(unix)]
    sigterm: tokio::signal::unix::Signal,
}

impl ShutdownSignals {
    fn install() -> io::Result<Self> {
        #[cfg(unix)]
        {
            let sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            Ok(Self { sigterm })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    async fn wait(&mut self) {
        #[cfg(unix)]
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = self.sigterm.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

fn init_tracing(format: LogFormat) {
    logging::init(format);
}
