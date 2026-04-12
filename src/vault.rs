use magic_crypt::{new_magic_crypt, MagicCryptTrait};
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
            EncryptBits::B256 => "AES-256",
        }
    }
}

pub struct DecryptResult {
    pub content: String,
    pub migration_needed: bool,
}

pub struct Vault;

impl Vault {
    const V2_HEADER: &'static str = "VAULT:v2:";
    const SALT_LEN: usize = 16;
    const NONCE_LEN: usize = 12;

    pub fn make_legacy_header(bits: EncryptBits) -> String {
        let b: u16 = match bits {
            EncryptBits::B64  => 64,
            EncryptBits::B128 => 128,
            EncryptBits::B192 => 192,
            EncryptBits::B256 => 256,
        };
        format!("VAULT:{}:\n", b)
    }

    pub fn encrypt(content: &str, password: &str, _bits: EncryptBits) -> String {
        if password.is_empty() {
            return content.to_string();
        }

        // --- SECURE V2 IMPLEMENTATION (AES-256-GCM + Argon2id) ---
        let mut salt = [0u8; Self::SALT_LEN];
        let mut nonce_bytes = [0u8; Self::NONCE_LEN];
        thread_rng().fill_bytes(&mut salt);
        thread_rng().fill_bytes(&mut nonce_bytes);

        // Derive 256-bit key using Argon2id
        let mut key = [0u8; 32];
        let argon2 = Argon2::default();
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

    pub fn decrypt(raw: &str, password: &str) -> Result<DecryptResult, String> {
        if raw.starts_with(Self::V2_HEADER) {
            // --- SECURE V2 DECRYPTION ---
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
            Argon2::default().hash_password_into(password.as_bytes(), salt, &mut key)
                .map_err(|_| "KDF derivation failed".to_string())?;

            let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
            let nonce = Nonce::from_slice(nonce_bytes);

            let plaintext_bytes = cipher.decrypt(nonce, ciphertext)
                .map_err(|_| "[WRONG PASSWORD or TAMPERED DATA - Decryption failed]".to_string())?;

            let content = String::from_utf8(plaintext_bytes)
                .map_err(|_| "Invalid UTF-8 content".to_string())?;

            Ok(DecryptResult { content, migration_needed: false })

        } else if raw.starts_with("VAULT:") {
            // --- LEGACY V1 DECRYPTION (MIGRATION MODE) ---
            let rest = &raw[6..];
            if let Some(colon_pos) = rest.find(':') {
                let bits_str = &rest[..colon_pos];
                let payload = &rest[colon_pos + 2..].trim();
                let bits = bits_str.parse::<u16>().map_err(|_| "Invalid legacy header")?;

                if password.is_empty() {
                    let label = match bits { 64 => "DES-64", 128 => "AES-128", 192 => "AES-192", _ => "AES-256" };
                    return Err(format!("[LOCKED - {} legacy encrypted]\n\nThis note uses an insecure format. Type :vault to unlock and migrate.", label));
                }

                let result = match bits {
                    64  => { let mc = new_magic_crypt!(password, 64);  mc.decrypt_base64_to_string(payload) }
                    128 => { let mc = new_magic_crypt!(password, 128); mc.decrypt_base64_to_string(payload) }
                    192 => { let mc = new_magic_crypt!(password, 192); mc.decrypt_base64_to_string(payload) }
                    _   => { let mc = new_magic_crypt!(password, 256); mc.decrypt_base64_to_string(payload) }
                };

                match result {
                    Ok(content) => Ok(DecryptResult { content, migration_needed: true }),
                    Err(_) => Err("[WRONG PASSWORD - Legacy decryption failed]".to_string()),
                }
            } else {
                Err("Incomplete legacy header".to_string())
            }
        } else {
            // Plaintext
            Ok(DecryptResult { content: raw.to_string(), migration_needed: false })
        }
    }
}
