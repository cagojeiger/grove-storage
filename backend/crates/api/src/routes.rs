//! HTTP 표면 — 경로 배선과 공통 레이어만 안다.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use grove_db::PgPool;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{Span, info, info_span};

mod management;
mod objects;

/// 컨트롤 API 요청 본문 상한. 바이트는 이 표면을 지나지 않는다 (공리 2).
const CONTROL_BODY_LIMIT: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// 단일 리스너의 최상위 제어 경로 세그먼트다. client id(= S3 버킷, 루트
/// path-style)가 이 중 하나와 같으면 제어 라우트를 가리므로 예약된다
/// (admin::clients가 client id로 거부한다).
pub(crate) const RESERVED_TOP_LEVEL: &[&str] = grove_db::registry::RESERVED_CLIENT_IDS;

#[derive(Clone)]
pub struct AppState {
    pub clock: Arc<dyn grove_core::time::Clock>,
    pub pool: PgPool,
    pub crypto: Arc<grove_core::Crypto>,
    /// 중계 바이트 URL의 공개 베이스 (GROVE_PUBLIC_URL). 중계 storage
    /// 등록·발급이 요구한다 — 없으면 등록이 400으로 거부된다.
    pub public_url: Option<String>,
    pub console_origin: Option<String>,
    pub console_web: Option<crate::console_web::ConsoleWeb>,
    /// 이 선언 크기를 넘으면 create가 multipart를 발급한다 (spec 02).
    pub multipart_threshold: i64,
    /// multipart part 크기 — create 시점 값이 업로드별로 동결된다 (spec 02).
    pub part_size: i64,
    /// storage당 S3 클라이언트 재사용 — 커넥션 풀을 웜 상태로 유지한다.
    pub s3_clients: Arc<grove_infra::S3ClientCache>,
    /// Bound the DB connections held by Native single-upload relay claims.
    pub single_upload_claims: Arc<tokio::sync::Semaphore>,
    /// S3 중계 스풀 동시성 상한 — 공유 임시 볼륨(temp_dir)을 채우는 자원
    /// 고갈(DoS)을 막는다. relay 진입 시 슬롯을 잡는다(spool 모듈).
    pub spool_slots: Arc<tokio::sync::Semaphore>,
}

pub(crate) fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .and_then(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer").then_some(token))
}

pub fn app(state: AppState, s3_cors_allowed_origins: &[String]) -> Router {
    // 표면이 둘이다: 컨트롤(JSON, 본문 상한·타임아웃)과 바이트(/blobs, 스트리밍 —
    // 요청 전체 타임아웃 없음: 크기는 스트림 차단이, 진행 중 연결의 수명은
    // blobs의 청크 유휴 타임아웃이 다스린다. lease 만료는 진입 시에만 검사된다.
    // GET의 저속 수신은 여기서 다스리지 않는다 — 앞단 프록시의 몫).
    // Router::layer는 나중에 추가한 레이어가 바깥이다. 요청 기준 실행 순서가
    // SetRequestId → Trace → (컨트롤만: Timeout → BodyLimit)이다.
    let control = Router::new()
        .route("/", get(root))
        .merge(system_routes())
        .merge(management::routes(state.clone()))
        .merge(crate::console_web::routes(&state))
        .merge(objects::control(state.clone()))
        .layer(RequestBodyLimitLayer::new(CONTROL_BODY_LIMIT))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ));
    // S3 호환 표면은 control 바깥에 둔다 — 루트 path-style `/{bucket}/{key}`라
    // 컨트롤 표면과 한 리스너를 공유하되, control의 본문 상한·타임아웃(스트리밍
    // 업로드를 자른다)은 피한다. api·blobs·probes 이름의 버킷은 예약된다
    // (admin::clients가 client id로 거부한다).
    let boundary = state.console_web.as_ref().and(state.console_origin.clone());
    let app = Router::new()
        .merge(control)
        .merge(objects::streaming(s3_cors_allowed_origins))
        .with_state(state);
    let app = if let Some(origin) = boundary {
        app.layer(axum::middleware::from_fn_with_state(
            origin,
            crate::console_web::host_boundary,
        ))
    } else {
        app
    };
    with_telemetry(app)
}

/// 요청 telemetry — request-id 생성/전파 + trace 스팬. 컨트롤·바이트·S3 표면이
/// 한 리스너에서 공유한다: 모든 표면이 request-id와 request.end를 갖도록.
/// 실행 순서는 SetRequestId → Trace → 핸들러다.
fn with_telemetry(router: Router) -> Router {
    router
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(make_request_span)
                .on_response(log_request_end),
        )
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

fn system_routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
}

/// 시스템 경로 판정 — 성공 프로브의 스팬 로그를 제외한다.
/// 위 system_routes 등록 목록과 같아야 한다.
pub(crate) fn is_system_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz")
}

async fn root() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "grove-storage",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Liveness: 프로세스가 살아 있다. 의존성 검사는 하지 않는다 (k8s livenessProbe).
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Readiness: DB에 닿을 수 있어야 트래픽을 받는다 (k8s readinessProbe).
async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    match grove_db::ping(&state.pool).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ready" })),
        ),
        Err(error) => {
            tracing::error!(event = "ready.failed", %error);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "status": "unavailable" })),
            )
        }
    }
}

/// 프로브·스크레이프는 "health-check" 스팬으로 만들어 성공 시 로그를 뺀다.
fn make_request_span(req: &Request) -> Span {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("");
    let path = req.uri().path();
    if is_system_path(path) {
        info_span!("health-check", method = %req.method(), route)
    } else {
        info_span!("request", method = %req.method(), route)
    }
}

/// 성공한 프로브·스크레이프는 로그를 남기지 않는다. 나머지는 request.end로 기록.
fn log_request_end(response: &axum::response::Response, latency: Duration, span: &Span) {
    let is_probe = span
        .metadata()
        .map(|m| m.name() == "health-check")
        .unwrap_or(false);
    if is_probe && response.status().is_success() {
        return;
    }
    info!(
        event = "request.end",
        status = response.status().as_u16(),
        latency_ms = latency.as_millis() as u64,
    );
}

#[cfg(test)]
pub(crate) mod tests;
