//! client(서비스 신원)와 그 소유물인 키 해시의 등록.
//!
//! 키는 해시로만 도착한다 (spec 01: raw는 서버에 도달하지 않는다).
//! 검증은 전부 DB가 한다 — 슬러그·해시 형식은 CHECK, 참조는 FK.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use filegate_db::registry;
use grove_management_command::model::{Client as ClientOut, ClientKey as ClientKeyOut};
use grove_management_service::resources::PreparedCredential;
use serde::Deserialize;

use crate::error::{ApiError, bad_request, not_found};
use crate::routes::AppState;

#[derive(Deserialize)]
pub(super) struct ClientCreateBody {
    id: String,
    /// 클라이언트가 소유하는 기반 storage — 업로드가 향하는 곳.
    storage_id: String,
}

#[derive(Deserialize)]
pub(super) struct ClientKeyCreateBody {
    key_hash: String,
}

pub(super) async fn create(
    State(state): State<AppState>,
    Json(body): Json<ClientCreateBody>,
) -> Result<Response, ApiError> {
    if crate::routes::RESERVED_TOP_LEVEL.contains(&body.id.as_str()) {
        return Err(bad_request(
            "id is reserved and cannot be used as a client/bucket name",
        ));
    }
    // 없는 storage를 가리키는 건 잘못된 입력이다 (FK도 거부하지만 400으로).
    if registry::get_storage(&state.pool, &body.storage_id)
        .await?
        .is_none()
    {
        return Err(bad_request(
            "storage_id does not reference a registered storage",
        ));
    }
    registry::insert_client(&state.pool, &body.id, &body.storage_id).await?;
    tracing::info!(event = "client.registered", client = %body.id, storage = %body.storage_id);
    Ok((
        StatusCode::CREATED,
        Json(ClientOut {
            id: body.id,
            storage_id: body.storage_id,
        }),
    )
        .into_response())
}

pub(super) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let storage_id = registry::client_storage(&state.pool, &id)
        .await?
        .ok_or_else(|| not_found("client not found"))?;
    Ok(Json(ClientOut { id, storage_id }).into_response())
}

pub(super) async fn list(State(state): State<AppState>) -> Result<Response, ApiError> {
    let ids = registry::list_clients(&state.pool).await?;
    Ok(Json(ids).into_response())
}

pub(super) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    registry::delete_client(&state.pool, &id)
        .await
        .map_err(ApiError::on_delete)?;
    tracing::info!(event = "client.deleted", client = %id);
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub(super) async fn key_create(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
    Json(body): Json<ClientKeyCreateBody>,
) -> Result<Response, ApiError> {
    registry::insert_client_key(&state.pool, &client_id, &body.key_hash).await?;
    tracing::info!(event = "client_key.registered", client = %client_id);
    Ok((
        StatusCode::CREATED,
        Json(ClientKeyOut {
            client_id,
            key_hash: body.key_hash,
        }),
    )
        .into_response())
}

pub(super) async fn key_get(
    State(state): State<AppState>,
    Path((client_id, key_hash)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    if !registry::client_key_exists(&state.pool, &client_id, &key_hash).await? {
        return Err(not_found("client key not found"));
    }
    Ok(Json(ClientKeyOut {
        client_id,
        key_hash,
    })
    .into_response())
}

pub(super) async fn key_list(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Response, ApiError> {
    if !registry::client_exists(&state.pool, &client_id).await? {
        return Err(not_found("client not found"));
    }
    let hashes = registry::list_client_keys(&state.pool, &client_id).await?;
    Ok(Json(hashes).into_response())
}

pub(super) async fn key_delete(
    State(state): State<AppState>,
    Path((client_id, key_hash)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    registry::delete_client_key(&state.pool, &client_id, &key_hash)
        .await
        .map_err(ApiError::on_delete)?;
    tracing::info!(event = "client_key.deleted", client = %client_id);
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---- S3 표면 자격증명 (spec 03) ----

pub(super) async fn s3_credential_create(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Response, ApiError> {
    let prepared = PreparedCredential::new(&state.crypto)?;
    let key = prepared.encrypted();
    filegate_db::s3_registry::insert_credential(
        &state.pool,
        key.access_key_id,
        &client_id,
        key.ciphertext,
        key.nonce,
        key.enc_key_id,
    )
    .await?;
    tracing::info!(event = "s3_credential.registered", client = %client_id, access_key = %key.access_key_id);
    Ok((StatusCode::CREATED, Json(prepared.into_output())).into_response())
}

pub(super) async fn s3_credential_list(
    State(state): State<AppState>,
    Path(client_id): Path<String>,
) -> Result<Response, ApiError> {
    if !registry::client_exists(&state.pool, &client_id).await? {
        return Err(not_found("client not found"));
    }
    let ids = filegate_db::s3_registry::list_credentials(&state.pool, &client_id).await?;
    Ok(Json(ids).into_response())
}

pub(super) async fn s3_credential_delete(
    State(state): State<AppState>,
    Path((client_id, access_key_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    filegate_db::s3_registry::delete_credential(&state.pool, &client_id, &access_key_id)
        .await
        .map_err(ApiError::on_delete)?;
    tracing::info!(event = "s3_credential.deleted", client = %client_id, access_key = %access_key_id);
    Ok(StatusCode::NO_CONTENT.into_response())
}
