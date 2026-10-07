//! 핸들러 에러 → HTTP 응답 번역 (운영자 API와 클라이언트 API 공용).

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use grove_db::registry::{self, WriteOp, WriteViolation};

pub(crate) enum ApiError {
    /// 명시적 상태와 메시지 (400/401/404).
    Status(StatusCode, String),
    /// DB 쓰기 거부 — 분류는 IntoResponse에서 (중복 409, 참조 없음 404,
    /// 사용 중 409, CHECK 위반 400).
    Db(grove_db::DbError, WriteOp),
    /// 내부 실패 — 상세는 로그로, 응답은 일반 문구.
    Internal(grove_core::Error),
    /// 저장소 호출 실패 — 502, 상세는 로그로.
    Storage(anyhow::Error),
}

pub(crate) fn bad_request(message: &str) -> ApiError {
    ApiError::Status(StatusCode::BAD_REQUEST, message.to_owned())
}

pub(crate) fn not_found(message: &str) -> ApiError {
    ApiError::Status(StatusCode::NOT_FOUND, message.to_owned())
}

pub(crate) fn unauthorized(message: &str) -> ApiError {
    ApiError::Status(StatusCode::UNAUTHORIZED, message.to_owned())
}

pub(crate) fn conflict(message: &str) -> ApiError {
    ApiError::Status(StatusCode::CONFLICT, message.to_owned())
}

pub(crate) fn status(code: StatusCode, message: &str) -> ApiError {
    ApiError::Status(code, message.to_owned())
}

pub(crate) fn internal(detail: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(grove_core::Error::internal(detail))
}

impl From<grove_db::DbError> for ApiError {
    fn from(error: grove_db::DbError) -> Self {
        Self::Db(error, WriteOp::Insert)
    }
}

impl From<grove_core::Error> for ApiError {
    fn from(error: grove_core::Error) -> Self {
        Self::Internal(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Status(status, message) => payload(status, &message),
            Self::Db(error, op) => match registry::write_violation(&error, op) {
                Some(WriteViolation::Duplicate) => payload(StatusCode::CONFLICT, "already exists"),
                Some(WriteViolation::MissingRef(constraint)) => {
                    // 없는 부모를 가리키는 쓰기 — 어느 노드가 없는지 제약 이름이 말해준다.
                    let target = if constraint.contains("storage") {
                        "storage not found"
                    } else if constraint.contains("client") {
                        "client not found"
                    } else {
                        "referenced registration not found"
                    };
                    payload(StatusCode::NOT_FOUND, target)
                }
                Some(WriteViolation::InUse) => payload(
                    StatusCode::CONFLICT,
                    "still referenced — delete dependent clients/files first",
                ),
                Some(WriteViolation::Invalid) => payload(
                    StatusCode::BAD_REQUEST,
                    "invalid field (id slug, capacity_bytes >= 0, key hash format)",
                ),
                None => {
                    tracing::error!(event = "api.db_error", %error);
                    payload(StatusCode::INTERNAL_SERVER_ERROR, "database error")
                }
            },
            Self::Internal(error) => {
                tracing::error!(event = "api.internal", %error);
                payload(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
            Self::Storage(error) => {
                tracing::error!(event = "api.storage_error", %error);
                payload(StatusCode::BAD_GATEWAY, "storage unavailable")
            }
        }
    }
}

fn payload(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}
