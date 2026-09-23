use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use grove_s3_protocol::integrity::{verify_base64, verify_hex};

use super::{header_str, xml::xml_error};
use crate::spool::Measured;

#[allow(clippy::result_large_err)]
pub(super) fn verify_read_condition(
    headers: &HeaderMap,
    etag: Option<&str>,
) -> Result<(), Response> {
    let values = headers.get_all("if-match");
    if values.iter().next().is_none() {
        return Ok(());
    }
    let joined = values
        .iter()
        .map(|v| v.to_str())
        .collect::<Result<Vec<_>, _>>();
    if !joined
        .is_ok_and(|values| grove_s3_protocol::integrity::if_match_matches(&values.join(","), etag))
    {
        return Err(xml_error(
            StatusCode::PRECONDITION_FAILED,
            "PreconditionFailed",
            "the object etag does not match",
        ));
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
pub(super) fn verify(headers: &HeaderMap, measured: &Measured) -> Result<(), Response> {
    for name in headers.keys() {
        if name.as_str().starts_with("x-amz-checksum-")
            && !matches!(
                name.as_str(),
                "x-amz-checksum-crc32" | "x-amz-checksum-sha256"
            )
        {
            return Err(xml_error(
                StatusCode::NOT_IMPLEMENTED,
                "NotImplemented",
                "this checksum option is not supported",
            ));
        }
    }
    if let Some(algorithm) = header_str(headers, "x-amz-sdk-checksum-algorithm") {
        let required = match algorithm {
            "CRC32" => "x-amz-checksum-crc32",
            "SHA256" => "x-amz-checksum-sha256",
            _ => {
                return Err(xml_error(
                    StatusCode::NOT_IMPLEMENTED,
                    "NotImplemented",
                    "this checksum algorithm is not supported",
                ));
            }
        };
        if !headers.contains_key(required) {
            return Err(xml_error(
                StatusCode::BAD_REQUEST,
                "BadDigest",
                "checksum algorithm requires a matching digest",
            ));
        }
    }
    if let Some(expected) = header_str(headers, "x-amz-content-sha256")
        && expected != "UNSIGNED-PAYLOAD"
        && !measured
            .sha256_hex
            .as_deref()
            .is_some_and(|hash| hash.eq_ignore_ascii_case(expected))
    {
        return Err(xml_error(
            StatusCode::BAD_REQUEST,
            "XAmzContentSHA256Mismatch",
            "the signed payload hash does not match the body",
        ));
    }
    for name in [
        "content-md5",
        "x-amz-checksum-crc32",
        "x-amz-checksum-sha256",
    ] {
        if headers.get_all(name).iter().count() > 1 {
            return Err(xml_error(
                StatusCode::BAD_REQUEST,
                "InvalidDigest",
                "duplicate checksum header",
            ));
        }
        let Some(value) = headers.get(name) else {
            continue;
        };
        let expected = value.to_str().map_err(|_| {
            xml_error(
                StatusCode::BAD_REQUEST,
                "InvalidDigest",
                "invalid checksum header",
            )
        })?;
        let result = match name {
            "content-md5" => verify_hex(expected, &measured.md5_hex),
            "x-amz-checksum-sha256" => {
                verify_hex(expected, measured.sha256_hex.as_deref().unwrap_or_default())
            }
            _ => verify_base64(expected, &measured.crc32.unwrap_or_default().to_be_bytes()),
        };
        result.map_err(|error| {
            xml_error(
                StatusCode::BAD_REQUEST,
                error.code(),
                "invalid or mismatched request checksum",
            )
        })?;
    }
    Ok(())
}
