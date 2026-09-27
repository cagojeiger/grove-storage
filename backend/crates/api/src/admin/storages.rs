//! Legacy storage REST handlers with shared resource response projection.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use filegate_db::registry::{self, StorageRow};
use grove_management_command::model::Storage;
use serde::Deserialize;

use crate::error::{ApiError, conflict, internal, not_found};
use crate::routes::AppState;
use crate::storage_registration::{Submission, verified_row};

#[derive(Deserialize)]
pub(super) struct StorageCreateBody {
    id: String,
    #[serde(flatten)]
    spec: Submission,
}

fn output(row: StorageRow) -> Result<Storage, ApiError> {
    grove_management_service::resources::storage_output(row)
        .map_err(|_| internal("invalid storage kind"))
}

pub(super) async fn create(
    State(state): State<AppState>,
    Json(body): Json<StorageCreateBody>,
) -> Result<Response, ApiError> {
    let relay_base_ready = state.public_url.is_some();
    let row = verified_row(&state.crypto, relay_base_ready, &body.id, body.spec).await?;
    registry::insert_storage(&state.pool, &row).await?;
    tracing::info!(event = "storage.registered", storage = %row.id, kind = %row.kind);
    Ok((StatusCode::CREATED, Json(output(row)?)).into_response())
}

pub(super) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Submission>,
) -> Result<Response, ApiError> {
    // 없는 행의 갱신은 네트워크 검증 전에 404로 끝낸다.
    if registry::get_storage(&state.pool, &id).await?.is_none() {
        return Err(not_found("storage not found"));
    }
    let relay_base_ready = state.public_url.is_some();
    let row = verified_row(&state.crypto, relay_base_ready, &id, body).await?;
    match registry::update_storage(&state.pool, &row).await? {
        registry::UpdateStorageOutcome::Updated => {}
        registry::UpdateStorageOutcome::NotFound => return Err(not_found("storage not found")),
        registry::UpdateStorageOutcome::LocationInUse => {
            return Err(conflict(
                "storage address cannot change while file locations remain",
            ));
        }
    }
    tracing::info!(event = "storage.updated", storage = %row.id);
    Ok(Json(output(row)?).into_response())
}

pub(super) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let row = registry::get_storage(&state.pool, &id)
        .await?
        .ok_or_else(|| not_found("storage not found"))?;
    Ok(Json(output(row)?).into_response())
}

pub(super) async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = registry::list_storages(&state.pool).await?;
    Ok(Json(
        rows.into_iter()
            .map(output)
            .collect::<Result<Vec<_>, _>>()?,
    )
    .into_response())
}

pub(super) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    registry::delete_storage(&state.pool, &id)
        .await
        .map_err(ApiError::on_delete)?;
    tracing::info!(event = "storage.deleted", storage = %id);
    Ok(StatusCode::NO_CONTENT.into_response())
}
