use magic_crypt::{new_magic_crypt, MagicCryptTrait};

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

pub struct Vault;

impl Vault {
    pub fn make_header(bits: EncryptBits) -> String {
        let b: u16 = match bits {
            EncryptBits::B64  => 64,
            EncryptBits::B128 => 128,
            EncryptBits::B192 => 192,
            EncryptBits::B256 => 256,
        };
        format!("VAULT:{}:\n", b)
    }

    pub fn parse_header(raw: &str) -> Option<(u16, &str)> {
        if raw.starts_with("VAULT:") {
            let rest = &raw[6..];
            if let Some(colon_pos) = rest.find(':') {
                let bits_str = &rest[..colon_pos];
                let payload = &rest[colon_pos + 2..];
                if let Ok(bits) = bits_str.parse::<u16>() {
                    return Some((bits, payload));
                }
            }
        }
        None
    }

    pub fn encrypt(content: &str, password: &str, bits: EncryptBits) -> String {
        if password.is_empty() {
            return content.to_string();
        }
        let header = Self::make_header(bits);
        let encrypted = match bits {
            EncryptBits::B64  => { let mc = new_magic_crypt!(password, 64);  mc.encrypt_str_to_base64(content) }
            EncryptBits::B128 => { let mc = new_magic_crypt!(password, 128); mc.encrypt_str_to_base64(content) }
            EncryptBits::B192 => { let mc = new_magic_crypt!(password, 192); mc.encrypt_str_to_base64(content) }
            EncryptBits::B256 => { let mc = new_magic_crypt!(password, 256); mc.encrypt_str_to_base64(content) }
        };
        format!("{}{}", header, encrypted)
    }

    pub fn decrypt(raw: &str, password: &str) -> Result<String, String> {
        match Self::parse_header(raw) {
            Some((bits, payload)) => {
                if password.is_empty() {
                    let label = match bits {
                        64 => "DES-64",
                        128 => "AES-128",
                        192 => "AES-192",
                        _ => "AES-256"
                    };
                    return Err(format!("[LOCKED - {} encrypted]\n\nType :vault to unlock this note.", label));
                }
                let result = match bits {
                    64  => { let mc = new_magic_crypt!(password, 64);  mc.decrypt_base64_to_string(payload) }
                    128 => { let mc = new_magic_crypt!(password, 128); mc.decrypt_base64_to_string(payload) }
                    192 => { let mc = new_magic_crypt!(password, 192); mc.decrypt_base64_to_string(payload) }
                    _   => { let mc = new_magic_crypt!(password, 256); mc.decrypt_base64_to_string(payload) }
                };
                result.map_err(|_| "[WRONG PASSWORD - decryption failed]\n\nType :vault to try a different password.".to_string())
            }
            None => Ok(raw.to_string()),
        }
    }
}
