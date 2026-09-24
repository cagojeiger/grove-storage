use sha2::{Digest, Sha256};

pub(super) const TOKEN_PREFIX: &str = "gsm_";
pub(super) const SESSION_PREFIX: &str = "gss_";
pub(super) const MASTER_PREFIX: &str = "gsmt_";
pub(super) const MASTER_SESSION_PREFIX: &str = "gsms_";

pub(super) fn valid(raw: &str, prefix: &str) -> bool {
    raw.strip_prefix(prefix).is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn hash(domain: &str, raw: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash.update(raw.as_bytes());
    hex::encode(hash.finalize())
}

pub(super) fn token_hash(raw: &str) -> String {
    hash("grove-management-token-v1", raw)
}
pub(super) fn session_hash(raw: &str) -> String {
    hash("grove-management-session-v1", raw)
}

pub(super) fn master_hash(raw: &str) -> String {
    hash("grove-master-token-v1", raw)
}
pub(super) fn master_session_hash(raw: &str) -> String {
    hash("grove-master-session-v1", raw)
}
