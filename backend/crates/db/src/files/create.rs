//! create의 pending 파일 기록 — 선언 해석 → 기록. 전부 한 트랜잭션.
//!
//! Registered capacity is observational, not a write quota. Usage is aggregated
//! from object metadata; this transaction performs no provider network I/O.

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::registry::{STORAGE_COLUMNS, StorageRow};

/// create 요청의 선언. 업로드 대상 storage는 클라이언트가 소유한 것으로
/// 해석된다 — 선언에 들어오지 않는다.
pub struct CreateSpec<'a> {
    pub client_id: &'a str,
    pub declared_size: i64,
    pub content_type: Option<&'a str>,
    pub declared_md5: Option<&'a str>,
    pub lease_ttl_secs: i64,
    /// multipart면 Some — create 시점 설정값이 업로드별로 동결된다 (spec 02).
    pub part_size: Option<i64>,
}

/// create가 예약을 마친 결과. URL 발급(presign 또는 중계 secret)은
/// 호출자가 storage 종류에 따라 한다.
pub struct CreatedFile {
    pub file_id: Uuid,
    pub lease_id: Uuid,
    pub object_key: String,
    pub storage: StorageRow,
}

pub enum CreateOutcome {
    Created(Box<CreatedFile>),
    /// 클라이언트가 등록부에 없어 소유 storage를 해석할 수 없다.
    NoClient,
}

/// 선언 해석 → pending 파일 기록. 저장소 공유 락은 동시 create끼리 호환되고
/// 저장소 주소 변경과는 직렬화된다. 예약 전체는 한 트랜잭션이다.
pub async fn create(pool: &PgPool, spec: CreateSpec<'_>) -> Result<CreateOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let outcome = create_in_tx(&mut tx, spec).await?;
    if matches!(&outcome, CreateOutcome::Created(_)) {
        tx.commit().await?;
    }
    Ok(outcome)
}

/// pending 파일·location·write lease·이력을 호출자 트랜잭션에 기록한다.
/// S3 표면은 이 헬퍼 뒤에 논리키 세션을 붙여 create 전체를 원자화한다.
pub(crate) async fn create_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    spec: CreateSpec<'_>,
) -> Result<CreateOutcome, sqlx::Error> {
    // storages와 clients가 id 컬럼을 겹치므로 storage 컬럼은 s. 접두로 뽑는다.
    let storage_cols = STORAGE_COLUMNS
        .split(", ")
        .map(|c| format!("s.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let storage: Option<StorageRow> = sqlx::query_as(&format!(
        "SELECT {storage_cols} FROM storages s \
         JOIN clients c ON c.storage_id = s.id \
         WHERE c.id = $1 FOR SHARE OF s"
    ))
    .bind(spec.client_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(storage) = storage else {
        return Ok(CreateOutcome::NoClient);
    };

    let (file_id, now): (Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "INSERT INTO files (client_id, declared_size, content_type, declared_md5, \
         part_size) VALUES ($1, $2, $3, $4, $5) RETURNING id, created_at",
    )
    .bind(spec.client_id)
    .bind(spec.declared_size)
    .bind(spec.content_type)
    .bind(spec.declared_md5)
    .bind(spec.part_size)
    .fetch_one(&mut **tx)
    .await?;

    // 키는 규칙으로 조합해 저장한다 (spec 00 물리 배치). 읽기·삭제는 저장된
    // 키만 따르므로, 규칙이 바뀌어도 기존 객체는 계속 동작한다 (ADR 001).
    let object_key = object_key(spec.client_id, file_id, spec.content_type, now);
    sqlx::query("INSERT INTO locations (file_id, storage_id, object_key) VALUES ($1, $2, $3)")
        .bind(file_id)
        .bind(&storage.id)
        .bind(&object_key)
        .execute(&mut **tx)
        .await?;

    let lease_id: Uuid = sqlx::query_scalar(
        "INSERT INTO leases (file_id, kind, expires_at) \
         VALUES ($1, 'write', grove_time.transaction_now() + $2 * interval '1 second') RETURNING id",
    )
    .bind(file_id)
    .bind(spec.lease_ttl_secs)
    .fetch_one(&mut **tx)
    .await?;

    // 대여 이력 — 발급과 같은 트랜잭션이라 lease와 항상 짝이다 (관찰용,
    // leases가 GC된 뒤에도 남는 durable 로그).
    sqlx::query(
        "INSERT INTO lease_history (file_id, storage_id, client_id, kind, size) \
         VALUES ($1, $2, $3, 'write', $4)",
    )
    .bind(file_id)
    .bind(&storage.id)
    .bind(spec.client_id)
    .bind(spec.declared_size)
    .execute(&mut **tx)
    .await?;

    Ok(CreateOutcome::Created(Box::new(CreatedFile {
        file_id,
        lease_id,
        object_key,
        storage,
    })))
}

/// New S3 keys preserve `fg/{client}/{yyyy}/{mm}/{file_id}[.ext]`.
/// Existing objects are always addressed through their stored locations.
fn object_key(
    client_id: &str,
    file_id: Uuid,
    content_type: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let date = now.format("%Y/%m");
    let name = match ext_for(content_type) {
        Some(ext) => format!("{file_id}.{ext}"),
        None => file_id.to_string(),
    };
    format!("fg/{client_id}/{date}/{name}")
}

/// 확장자 허용목록 — content_type 문자열을 자르지 않는다 (spec 00: 경로
/// 오염 차단). 모르는 타입은 확장자 없음. 선언의 반영일 뿐 검증이 아니다.
fn ext_for(content_type: Option<&str>) -> Option<&'static str> {
    Some(match content_type? {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "text/markdown" => "md",
        "application/json" => "json",
        "application/zip" => "zip",
        "video/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        _ => return None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod key_tests {
    use super::*;

    #[test]
    fn s3_key_preserves_the_existing_prefix_and_extension() {
        let id = Uuid::parse_str("0198a3f2-1111-4222-8333-4444555566ab").unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-05T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let s3 = object_key("notegate", id, Some("application/pdf"), now);
        assert!(s3.starts_with("fg/notegate/"));
        assert!(s3.ends_with(&format!("/{id}.pdf")));
        assert_eq!(s3.matches('/').count(), 4); // fg/client/yyyy/mm/name

        let plain = object_key("notegate", id, None, now);
        assert!(plain.ends_with(&format!("/{id}")));
        assert_eq!(plain.matches('/').count(), 4);
    }

    #[test]
    fn ext_comes_only_from_the_allowlist() {
        assert_eq!(ext_for(Some("image/png")), Some("png"));
        assert_eq!(ext_for(Some("application/octet-stream")), None);
        assert_eq!(ext_for(Some("x/../escape")), None);
        assert_eq!(ext_for(None), None);
    }
}
