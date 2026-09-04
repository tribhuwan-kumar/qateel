use sha2::Sha256;
use hmac::{Hmac, Mac, KeyInit};
use base64::{engine::general_purpose, Engine as _};

type HmacSha256 = Hmac<Sha256>;

pub fn generate_bound_csrf(user_id: &str, secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap(); // unwrap :D
    mac.update(user_id.as_bytes());
    let result = mac.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(result.into_bytes())
}
