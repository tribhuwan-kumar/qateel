use std::sync::OnceLock;

/// Global thread safe server's ephemeral secret key
pub static SERVER_SECRET_KEY: OnceLock<[u8; 32]> = OnceLock::new();

pub fn get_server_secret() -> &'static [u8; 32] {
    SERVER_SECRET_KEY.get_or_init(|| {
        rand::random::<[u8; 32]>()
    })
}
