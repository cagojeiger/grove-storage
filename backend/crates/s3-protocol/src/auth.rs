//! Pure validation for the supported SigV4 credential and header contracts.

#[derive(Debug, PartialEq, Eq)]
pub struct CredentialScope<'a> {
    pub access_key: &'a str,
    pub date: &'a str,
    pub region: &'a str,
}

pub fn credential_scope(value: &str) -> Result<CredentialScope<'_>, &'static str> {
    let fields: Vec<_> = value.split('/').collect();
    let [access_key, date, region, "s3", "aws4_request"] = fields.as_slice() else {
        return Err("credential scope must be <key>/<date>/<region>/s3/aws4_request");
    };
    if access_key.is_empty()
        || region.is_empty()
        || date.len() != 8
        || !date.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("invalid credential scope");
    }
    Ok(CredentialScope {
        access_key,
        date,
        region,
    })
}

pub fn signed_headers(value: &str) -> Result<Vec<String>, &'static str> {
    let names: Vec<_> = value.split(';').collect();
    if !names.contains(&"host")
        || names
            .windows(2)
            .any(|pair| matches!(pair, [a, b] if a >= b))
        || names.iter().any(|name| {
            name.is_empty()
                || !name.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b"!#$%&'*+-.^_`|~".contains(&b)
                })
        })
    {
        return Err("SignedHeaders must contain host and be lowercase, sorted and unique");
    }
    Ok(names.into_iter().map(str::to_owned).collect())
}

pub fn presigned_expiry(value: &str) -> Result<i64, &'static str> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid X-Amz-Expires");
    }
    value
        .parse::<i64>()
        .ok()
        .filter(|n| (1..=604800).contains(n))
        .ok_or("X-Amz-Expires must be between 1 and 604800 seconds")
}

pub fn valid_payload_hash(value: &str) -> bool {
    value == "UNSIGNED-PAYLOAD"
        || (value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Compare the actual bytes to the hash that was authenticated, not to a new
/// interpretation of request headers after authentication.
pub fn payload_matches(expected: &str, body: &[u8]) -> bool {
    expected == "UNSIGNED-PAYLOAD"
        || super::signing::sha256_hex(body).eq_ignore_ascii_case(expected)
}

pub fn canonical_header_value(value: &str) -> String {
    value.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}
