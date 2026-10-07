//! Human passwords only. Random API tokens retain their separate hash contract.
use std::sync::{Arc, OnceLock};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use grove_core::{ExposeSecret, SecretString};
use tokio::sync::Semaphore;
use unicode_normalization::UnicodeNormalization;

const MIN_CHARS: usize = 15;
const MAX_CHARS: usize = 128;
const MAX_INPUT_BYTES: usize = 1024;
const HASH_WORKERS: usize = 4;
const HASH_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidUsername,
    InvalidLength,
    WeakPassword,
    Busy,
    InvalidHash,
    Unavailable,
}

pub fn username(value: &str) -> Result<String, Error> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=64).contains(&value.len())
        || !value.starts_with(|c: char| c.is_ascii_alphanumeric())
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(Error::InvalidUsername);
    }
    Ok(value)
}

fn normalize(value: &str) -> Result<SecretString, Error> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(Error::InvalidLength);
    }
    let normalized = SecretString::from(value.nfc().collect::<String>());
    if !(MIN_CHARS..=MAX_CHARS).contains(&normalized.expose_secret().chars().count()) {
        return Err(Error::InvalidLength);
    }
    Ok(normalized)
}

fn validate_new(value: &str, login: &str) -> Result<SecretString, Error> {
    let value = normalize(value)?;
    let folded = SecretString::from(value.expose_secret().trim().to_lowercase());
    let first = folded.expose_secret().chars().next();
    if folded.expose_secret() == login
        || folded.expose_secret().chars().all(|c| Some(c) == first)
        || [
            "correct horse battery staple",
            "this is my password",
            "this is a password",
            "passwordpassword",
            "password123456789",
            "12345678901234567890",
            "qwertyuiopasdfghjkl",
            "grove storage password",
        ]
        .contains(&folded.expose_secret())
    {
        return Err(Error::WeakPassword);
    }
    Ok(value)
}

fn engine() -> Result<Argon2<'static>, Error> {
    let params = Params::new(19 * 1024, 2, 1, Some(32)).map_err(|_| Error::Unavailable)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn workers() -> &'static Arc<Semaphore> {
    static WORKERS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    WORKERS.get_or_init(|| Arc::new(Semaphore::new(HASH_WORKERS)))
}

async fn permit() -> Result<tokio::sync::OwnedSemaphorePermit, Error> {
    permit_with(workers().clone(), HASH_WAIT).await
}

async fn permit_with(
    workers: Arc<Semaphore>,
    wait: std::time::Duration,
) -> Result<tokio::sync::OwnedSemaphorePermit, Error> {
    tokio::time::timeout(wait, workers.acquire_owned())
        .await
        .map_err(|_| Error::Busy)?
        .map_err(|_| Error::Unavailable)
}

pub async fn hash(login: &str, password: SecretString) -> Result<SecretString, Error> {
    let login = username(login)?;
    let password = validate_new(password.expose_secret(), &login)?;
    let permit = permit().await?;
    tokio::task::spawn_blocking(move || {
        // The permit lives with the CPU task, even if the request is cancelled.
        let _permit = permit;
        engine()?
            .hash_password(password.expose_secret().as_bytes())
            .map(|hash: PasswordHash| SecretString::from(hash.to_string()))
            .map_err(|_| Error::Unavailable)
    })
    .await
    .map_err(|_| Error::Unavailable)?
}

pub async fn verify(password: SecretString, hash: SecretString) -> Result<bool, Error> {
    let password = match normalize(password.expose_secret()) {
        Ok(password) => password,
        Err(Error::InvalidLength) => return Ok(false),
        Err(error) => return Err(error),
    };
    let permit = permit().await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let parsed = PasswordHash::new(hash.expose_secret()).map_err(|_| Error::InvalidHash)?;
        let params = Params::try_from(&parsed).map_err(|_| Error::InvalidHash)?;
        // Bound work even if a stored PHC string is corrupt or from an unsupported import.
        if parsed.algorithm.as_str() != "argon2id"
            || parsed.version != Some(19)
            || params.m_cost() != 19 * 1024
            || params.t_cost() != 2
            || params.p_cost() != 1
            || params.output_len() != Some(32)
            || !params.keyid().is_empty()
            || !params.data().is_empty()
            || parsed.salt.is_none()
        {
            return Err(Error::InvalidHash);
        }
        Ok(engine()?
            .verify_password(password.expose_secret().as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| Error::Unavailable)?
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn hash_queue_times_out_at_the_injected_deadline() {
        let start = tokio::time::Instant::now();
        assert!(matches!(
            permit_with(Arc::new(Semaphore::new(0)), HASH_WAIT).await,
            Err(Error::Busy)
        ));
        assert_eq!(start.elapsed(), HASH_WAIT);
    }

    #[tokio::test(start_paused = true)]
    async fn hash_queue_accepts_capacity_before_the_deadline() {
        let queue = Arc::new(Semaphore::new(0));
        let release = queue.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            release.add_permits(1);
        });
        let start = tokio::time::Instant::now();
        assert!(permit_with(queue, HASH_WAIT).await.is_ok());
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(4));
        task.await.unwrap();
    }

    #[test]
    fn usernames_have_one_canonical_form() {
        assert_eq!(username("  User.Name-01  ").unwrap(), "user.name-01");
        for input in ["a", "_user", "a b", "한글이름", "a/b", "user@host"] {
            assert_eq!(username(input), Err(Error::InvalidUsername));
        }
        assert!(username(&"a".repeat(64)).is_ok());
        assert!(username(&"a".repeat(65)).is_err());
    }

    #[test]
    fn password_policy_allows_passphrases_without_composition_rules() {
        assert!(validate_new("a private phrase for testing", "owner").is_ok());
        assert!(validate_new(" varied spacing stays here ", "owner").is_ok());
        for weak in [
            "aaaaaaaaaaaaaaaa",
            "passwordpassword",
            "correct horse battery staple",
        ] {
            assert!(matches!(
                validate_new(weak, "owner"),
                Err(Error::WeakPassword)
            ));
        }
        assert!(matches!(
            validate_new("qwer1234", "owner"),
            Err(Error::InvalidLength)
        ));
        assert!(matches!(
            validate_new(&"x".repeat(129), "owner"),
            Err(Error::InvalidLength)
        ));
        assert!(matches!(
            validate_new("longaccountname", "longaccountname"),
            Err(Error::WeakPassword)
        ));
        let decomposed = "cafe\u{301} phrase for testing";
        assert_eq!(
            normalize(decomposed).unwrap().expose_secret(),
            "caf\u{e9} phrase for testing"
        );
    }

    #[tokio::test]
    async fn hashes_are_salted_and_verify_normalized_passwords() {
        let raw = "cafe\u{301} phrase for testing";
        let a = hash("owner", raw.into()).await.unwrap();
        let b = hash("owner", raw.into()).await.unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
        assert!(
            a.expose_secret()
                .starts_with("$argon2id$v=19$m=19456,t=2,p=1$")
        );
        assert!(
            verify("caf\u{e9} phrase for testing".into(), a.clone())
                .await
                .unwrap()
        );
        assert!(
            !verify("another private phrase".into(), a.clone())
                .await
                .unwrap()
        );
        assert!(!verify("too short".into(), a.clone()).await.unwrap());
        assert!(!format!("{a:?}").contains(a.expose_secret()));
        assert_eq!(
            verify(raw.into(), "invalid".into()).await,
            Err(Error::InvalidHash)
        );
        let expensive = a.expose_secret().replace("m=19456", "m=4294967295");
        assert_eq!(
            verify(raw.into(), expensive.into()).await,
            Err(Error::InvalidHash)
        );
    }
}
