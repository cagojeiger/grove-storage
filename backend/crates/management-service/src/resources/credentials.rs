use grove_core::{Crypto, EncryptedSecret, ExposeSecret, SecretString};
use grove_db::management::EncryptedServiceCredential;
use grove_management_command::model::IssuedCredential;

/// Shared key material preparation; callers own authorization, persistence and audit.
/// No Debug/Serialize implementation: plaintext is exposed only for the issue reply.
pub struct PreparedCredential {
    access_key_id: String,
    secret: SecretString,
    encrypted: EncryptedSecret,
    enc_key_id: String,
}

impl PreparedCredential {
    pub fn new(crypto: &Crypto) -> Result<Self, grove_core::Error> {
        let access_key_id = grove_core::generate_access_key_id();
        let secret = SecretString::from(grove_core::generate_url_secret());
        let encrypted = crypto.encrypt(&access_key_id, &secret)?;
        Ok(Self {
            access_key_id,
            secret,
            encrypted,
            enc_key_id: crypto.active_key_id().to_owned(),
        })
    }

    pub fn encrypted(&self) -> EncryptedServiceCredential<'_> {
        EncryptedServiceCredential {
            access_key_id: &self.access_key_id,
            ciphertext: &self.encrypted.ciphertext,
            nonce: &self.encrypted.nonce,
            enc_key_id: &self.enc_key_id,
        }
    }

    /// The caller delivers this response only after its database commit succeeds.
    pub fn into_output(self) -> IssuedCredential {
        IssuedCredential {
            access_key_id: self.access_key_id,
            secret_key: self.secret.expose_secret().to_owned(),
        }
    }
}
