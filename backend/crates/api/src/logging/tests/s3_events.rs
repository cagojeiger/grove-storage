use super::*;
use grove_s3_protocol::signing::{canonicalize_query, sha256_hex, sign};
use tracing::instrument::WithSubscriber;

const DATE: &str = "20261017";
const AMZ_DATE: &str = "20261017T010000Z";

fn signed(method: &str, path: &str, query: &str, key: &str, secret: &str) -> Request<Body> {
    let scope = format!("{DATE}/auto/s3/aws4_request");
    let canonical = format!(
        "{method}\n{path}\n{}\nhost:grove.test\nx-amz-date:{AMZ_DATE}\n\nhost;x-amz-date\nUNSIGNED-PAYLOAD",
        canonicalize_query(query)
    );
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{AMZ_DATE}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let signature = sign(secret, DATE, "auto", &to_sign);
    let uri = if query.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{query}")
    };
    Request::builder().method(method).uri(uri)
        .header("host", "grove.test")
        .header("x-amz-date", AMZ_DATE)
        .header("x-amz-content-sha256", "UNSIGNED-PAYLOAD")
        .header("authorization", format!("AWS4-HMAC-SHA256 Credential={key}/{scope}, SignedHeaders=host;x-amz-date, Signature={signature}"))
        .body(Body::empty()).unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn signed_dispatch_records_all_eight_operations_and_early_rejections(pool: grove_db::PgPool) {
    let mut state = crate::routes::tests::test_state();
    state.pool = pool.clone();
    state.clock = Arc::new(grove_core::time::FixedClock(
        chrono::DateTime::parse_from_rfc3339("2026-10-17T01:00:00Z")
            .unwrap()
            .into(),
    ));
    sqlx::raw_sql("INSERT INTO storages(id,endpoint,public_endpoint,region,bucket,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes)
        VALUES('vendor','https://storage.invalid','https://storage.invalid','auto','objects','key',decode('01','hex'),decode(repeat('00',12),'hex'),'test-v1',100);
        INSERT INTO clients(id,storage_id) VALUES('app','vendor');")
        .execute(&pool).await.unwrap();
    let prepared =
        grove_management_service::resources::PreparedCredential::new(&state.crypto).unwrap();
    let encrypted = prepared.encrypted();
    grove_db::s3_registry::insert_credential(
        &pool,
        encrypted.access_key_id,
        "app",
        encrypted.ciphertext,
        encrypted.nonce,
        encrypted.enc_key_id,
    )
    .await
    .unwrap();
    let credential = prepared.into_output();
    let app = crate::routes::app(state, &[]);
    let capture = Capture::default();
    let observer = subscriber(LogFormat::Json, "info", capture.clone());
    async {
        let cases = [
            ("PUT", "", "put_object", 411, "MissingContentLength"),
            ("GET", "", "get_object", 404, "NoSuchKey"),
            ("HEAD", "", "head_object", 404, "NoSuchKey"),
            ("DELETE", "", "delete_object", 204, ""),
            (
                "POST",
                "uploads",
                "create_multipart_upload",
                400,
                "InvalidArgument",
            ),
            (
                "PUT",
                "uploadId=private-id&partNumber=1",
                "upload_part",
                404,
                "NoSuchUpload",
            ),
            (
                "POST",
                "uploadId=private-id",
                "complete_multipart_upload",
                404,
                "NoSuchUpload",
            ),
            (
                "DELETE",
                "uploadId=private-id",
                "abort_multipart_upload",
                404,
                "NoSuchUpload",
            ),
        ];
        for (method, query, operation, status, code) in cases {
            let before = capture.records().len();
            let mut request = signed(
                method,
                "/app/private-key",
                query,
                &credential.access_key_id,
                &credential.secret_key,
            );
            request
                .headers_mut()
                .insert("content-type", "x".repeat(256).parse().unwrap());
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), status, "{operation}");
            to_bytes(response.into_body(), 4096).await.unwrap();
            let rows: Vec<_> = capture
                .records()
                .into_iter()
                .skip(before)
                .filter(|row| row["event_name"] == "grove.http.request_finished")
                .collect();
            assert_eq!(rows.len(), 1, "{operation}");
            assert_eq!(rows[0]["attributes"]["grove.operation"], operation);
            assert_eq!(rows[0]["attributes"]["grove.stage"], "execution");
            assert_eq!(
                rows[0]["attributes"]["grove.outcome"],
                if status == 204 { "success" } else { "rejected" }
            );
            if !code.is_empty() {
                assert_eq!(rows[0]["attributes"]["grove.s3_error_code"], code);
            }

            let before = capture.records().len();
            let mut request = signed(
                method,
                "/app/private-key",
                query,
                &credential.access_key_id,
                &credential.secret_key,
            );
            request.headers_mut().insert(
                "if-modified-since",
                "Wed, 01 Jan 2020 00:00:00 GMT".parse().unwrap(),
            );
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), 501);
            to_bytes(response.into_body(), 4096).await.unwrap();
            let rows: Vec<_> = capture
                .records()
                .into_iter()
                .skip(before)
                .filter(|row| row["event_name"] == "grove.http.request_finished")
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["attributes"]["grove.operation"], operation);
            assert_eq!(rows[0]["attributes"]["grove.stage"], "validation");
            assert_eq!(
                rows[0]["attributes"]["grove.s3_error_code"],
                "NotImplemented"
            );
            assert_eq!(rows[0]["attributes"]["grove.outcome"], "rejected");
        }
        for (path, query, stage, status, code) in [
            (
                "/other/private-key",
                "",
                "authorization",
                404,
                "NoSuchBucket",
            ),
            (
                "/app/private-key",
                "tagging",
                "routing",
                501,
                "NotImplemented",
            ),
            (
                "/app/private-key",
                "uploadId=private-id&partNumber=0",
                "routing",
                400,
                "InvalidArgument",
            ),
        ] {
            let before = capture.records().len();
            let response = app
                .clone()
                .oneshot(signed(
                    "PUT",
                    path,
                    query,
                    &credential.access_key_id,
                    &credential.secret_key,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            to_bytes(response.into_body(), 4096).await.unwrap();
            let rows: Vec<_> = capture
                .records()
                .into_iter()
                .skip(before)
                .filter(|row| row["event_name"] == "grove.http.request_finished")
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["attributes"]["grove.operation"], "unknown");
            assert_eq!(rows[0]["attributes"]["grove.stage"], stage);
            assert_eq!(rows[0]["attributes"]["grove.s3_error_code"], code);
        }
    }
    .with_subscriber(observer)
    .await;
    let output = format!("{:?}", capture.records());
    for forbidden in [
        "private-key",
        "private-id",
        "storage.invalid",
        &credential.access_key_id,
        &credential.secret_key,
    ] {
        assert!(!output.contains(forbidden), "{forbidden}");
    }
}
