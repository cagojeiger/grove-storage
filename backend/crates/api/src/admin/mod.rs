//! 운영자 API의 리소스 경로 배선. 인증은 admin_auth가 담당한다.
//! 리소스 규칙은 하위 모듈, 상태 코드 번역은 error가 담당한다.

mod clients;
mod storages;
mod usage;

pub use storages::{check_registered, verify_registered};

use axum::Router;
use axum::routing::get;

use crate::routes::AppState;

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/usage", get(usage::report))
        .route("/usage/clients", get(usage::by_client))
        .route("/usage/history", get(usage::history))
        .route("/storages", get(storages::list).post(storages::create))
        .route(
            "/storages/{id}",
            get(storages::get)
                .put(storages::update)
                .delete(storages::delete),
        )
        .route("/clients", get(clients::list).post(clients::create))
        .route("/clients/{id}", get(clients::get).delete(clients::delete))
        .route(
            "/clients/{id}/keys",
            get(clients::key_list).post(clients::key_create),
        )
        .route(
            "/clients/{id}/keys/{key_hash}",
            get(clients::key_get).delete(clients::key_delete),
        )
        .route(
            "/clients/{id}/s3-credentials",
            get(clients::s3_credential_list).post(clients::s3_credential_create),
        )
        .route(
            "/clients/{id}/s3-credentials/{access_key_id}",
            axum::routing::delete(clients::s3_credential_delete),
        )
}
