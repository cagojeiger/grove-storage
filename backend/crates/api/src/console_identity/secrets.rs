use filegate_core::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

pub(crate) const TOKEN_PREFIX: &str = "gsm_";
pub(super) const SESSION_PREFIX: &str = "gss_";
pub(super) const MASTER_PREFIX: &str = "gsmt_";
pub(super) const MASTER_SESSION_PREFIX: &str = "gsms_";

pub(crate) fn valid(raw: &str, prefix: &str) -> bool {
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

pub(crate) fn token_hash(raw: &str) -> String {
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

// The same one-time issuance material serves master recovery and Admin issuance.
pub(super) struct IssuedToken {
    raw: SecretString,
    hash: String,
    prefix: String,
}
impl IssuedToken {
    pub fn new() -> Self {
        let raw = SecretString::from(format!(
            "{TOKEN_PREFIX}{}",
            filegate_core::generate_url_secret()
        ));
        Self {
            hash: token_hash(raw.expose_secret()),
            prefix: raw.expose_secret().chars().take(12).collect(),
            raw,
        }
    }
    pub fn expose(&self) -> &str {
        self.raw.expose_secret()
    }
    pub fn credential<'a>(
        &'a self,
        label: &'a str,
        days: u16,
    ) -> filegate_db::management::NewCredential<'a> {
        filegate_db::management::NewCredential {
            label,
            token_prefix: &self.prefix,
            token_hash: &self.hash,
            expires_at: chrono::Utc::now() + chrono::Duration::days(i64::from(days)),
        }
    }
}
