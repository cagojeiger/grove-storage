//! 운영자용 storage·client 점유 조회와 일별 사용량 이력.
//! capacity는 등록 기준선이며, 등록 변경은 운영자 리소스 API가 담당한다.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use filegate_db::usage;
use grove_management_service::resources::{
    client_usage_output, snapshot_output, storage_usage_output,
};
use serde::Deserialize;

use crate::error::{ApiError, internal};
use crate::routes::AppState;

pub(super) async fn report(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = usage::by_storage(&state.pool).await?;
    let out = rows
        .into_iter()
        .map(storage_usage_output)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| internal("invalid storage usage"))?;
    Ok(Json(out).into_response())
}

pub(super) async fn by_client(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = usage::by_client(&state.pool).await?;
    let out: Vec<_> = rows.into_iter().map(client_usage_output).collect();
    Ok(Json(out).into_response())
}

#[derive(Deserialize)]
pub(super) struct HistoryParams {
    days: Option<i32>,
}

/// 일별 점유 추이 — usage_snapshot 그대로 (조회 시점 파생이 아닌, 매일
/// 박제된 stock 시계열). storage·전체 합계는 소비자가 행 SUM으로 가른다.
pub(super) async fn history(
    State(state): State<AppState>,
    Query(params): Query<HistoryParams>,
) -> Result<Response, ApiError> {
    let days = params.days.unwrap_or(90).clamp(1, 3650);
    let rows = usage::snapshot_history(&state.pool, days).await?;
    let out: Vec<_> = rows.into_iter().map(snapshot_output).collect();
    Ok(Json(out).into_response())
}
