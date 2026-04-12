use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{
    Argon2,
};
use rand::{RngCore, thread_rng};
use base64::{Engine as _, engine::general_purpose};

#[derive(Clone, Copy, PartialEq)]
pub enum EncryptBits {
    B64,
    B128,
    B192,
    B256,
}

impl EncryptBits {
    pub fn label(&self) -> &'static str {
        match self {
            EncryptBits::B64  => "DES-64",
            EncryptBits::B128 => "AES-128",
            EncryptBits::B192 => "AES-192",
            EncryptBits::B256 => "AES-256-GCM",
        }
    }
}

pub struct Vault;

impl Vault {
    const V2_HEADER: &'static str = "VAULT:v2:";
    const SALT_LEN: usize = 16;
    const NONCE_LEN: usize = 12;

    // Explicit Argon2id parameters (High security)
    const ARGON2_MEM: u32 = 65536; // 64 MB
    const ARGON2_ITER: u32 = 3;
    const ARGON2_PARA: u32 = 4;

    fn get_argon2() -> Argon2<'static> {
        use argon2::{Algorithm, Params, Version};
        let params = Params::new(Self::ARGON2_MEM, Self::ARGON2_ITER, Self::ARGON2_PARA, Some(32)).unwrap();
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
    }

    pub fn is_encrypted(raw: &str) -> bool {
        raw.starts_with(Self::V2_HEADER) || raw.starts_with("VAULT:")
    }

    pub fn encrypt(content: &str, password: &str, _bits: EncryptBits) -> String {
        if password.is_empty() {
            return content.to_string();
        }

        let mut salt = [0u8; Self::SALT_LEN];
        let mut nonce_bytes = [0u8; Self::NONCE_LEN];
        thread_rng().fill_bytes(&mut salt);
        thread_rng().fill_bytes(&mut nonce_bytes);

        let mut key = [0u8; 32];
        let argon2 = Self::get_argon2();
        if let Err(_) = argon2.hash_password_into(password.as_bytes(), &salt, &mut key) {
            return "[ENCRYPTION ERROR - KDF failed]".to_string();
        }

        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        match cipher.encrypt(nonce, content.as_bytes()) {
            Ok(ciphertext) => {
                let mut combined = Vec::with_capacity(Self::SALT_LEN + Self::NONCE_LEN + ciphertext.len());
                combined.extend_from_slice(&salt);
                combined.extend_from_slice(&nonce_bytes);
                combined.extend_from_slice(&ciphertext);
                
                let encoded = general_purpose::STANDARD.encode(combined);
                format!("{}{}\n", Self::V2_HEADER, encoded)
            }
            Err(_) => "[ENCRYPTION ERROR - AES-GCM failed]".to_string(),
        }
    }

    pub fn decrypt(raw: &str, password: &str) -> Result<String, String> {
        if raw.starts_with(Self::V2_HEADER) {
            if password.is_empty() {
                return Err("[LOCKED - AES-256-GCM encrypted]\n\nType :vault to unlock this note.".to_string());
            }

            let payload_str = &raw[Self::V2_HEADER.len()..].trim();
            let combined = general_purpose::STANDARD.decode(payload_str)
                .map_err(|_| "Invalid vault encoding".to_string())?;

            if combined.len() < Self::SALT_LEN + Self::NONCE_LEN {
                return Err("Vault payload too short".to_string());
            }

            let salt = &combined[..Self::SALT_LEN];
            let nonce_bytes = &combined[Self::SALT_LEN..Self::SALT_LEN + Self::NONCE_LEN];
            let ciphertext = &combined[Self::SALT_LEN + Self::NONCE_LEN..];

            let mut key = [0u8; 32];
            Self::get_argon2().hash_password_into(password.as_bytes(), salt, &mut key)
                .map_err(|_| "KDF derivation failed".to_string())?;

            let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
            let nonce = Nonce::from_slice(nonce_bytes);

            let plaintext_bytes = cipher.decrypt(nonce, ciphertext)
                .map_err(|_| "[WRONG PASSWORD or TAMPERED DATA - Decryption failed]".to_string())?;

            String::from_utf8(plaintext_bytes)
                .map_err(|_| "Invalid UTF-8 content".to_string())

        } else if raw.starts_with("VAULT:") {
            Err("[INSECURE LEGACY FORMAT - Cannot decrypt]\n\nThis note uses a deprecated format. Please restore it using an older version of the app.".into())
        } else {
            // Plaintext
            Ok(raw.to_string())
        }
    }
}
