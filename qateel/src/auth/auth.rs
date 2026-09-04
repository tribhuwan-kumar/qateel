use std::fs;
use argon2::{
    Argon2,
    PasswordHash,
    PasswordVerifier,
    password_hash::{
        SaltString,
        PasswordHasher,
        rand_core::OsRng,
    },
};
use jsonwebtoken::{
    encode, Header,
    EncodingKey,
};
use ulid::Ulid;
use std::path::Path;
use sha2::Sha256;
use hmac::{Hmac, Mac, KeyInit};
use tracing::{info, warn};
use sqlx::{Pool, Sqlite, Row};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::key;
use super::types::{
	Role,
	User,
	Claims,
	AuthError,
	AuthController,
	AdminPassResetGenState,
};

type HmacSha256 = Hmac<Sha256>;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const COOKIE_VAILDITY_DURATION: usize = 24 * 60 * 60 * 15;

impl AuthController {
	async fn hash_password(password: String) -> Result<String, AuthError> {
        tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|h| h.to_string())
                .map_err(|_| AuthError::PasswordHashError)
        })
        .await
        .map_err(|_| AuthError::PasswordHashError)?
    }

    /// Just verify the string duh!! :D
    async fn verify_password(password: String, password_hash: String) -> Result<bool, AuthError> {
		let join_handle = tokio::task::spawn_blocking(move || {
			let parsed_hash = PasswordHash::new(&password_hash)
				.map_err(|_| AuthError::PasswordHashError)?;

			let is_valid = Argon2::default()
				.verify_password(password.as_bytes(), &parsed_hash)
				.is_ok();

			Ok(is_valid)
		});

		match join_handle.await {
            Ok(result) => result,
            Err(_) => Err(AuthError::InvalidCredentials),
        }
    }

    fn now_unix_secs() -> Result<u64, AuthError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|_| AuthError::TimeError)
    }

    /// Pass it directly from main -> state
    fn create_token(
        username: &str,
        uid: String,
        role: Role,
        token_version: i64,
        secret: &[u8],
    ) -> Result<String, AuthError> {
        let expiration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize + COOKIE_VAILDITY_DURATION;

        let claims = Claims {
            role: role,
            user_id: uid,
            exp: expiration,
            sub: username.to_string(),
            token_version: token_version,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret),
        )
        .map_err(|_| AuthError::TokenCreationError)
    }

    async fn update_password(
        pool: &Pool<Sqlite>,
        user_id: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
		if new_password.len() <= MIN_PASSWORD_LENGTH {
			return Err(AuthError::PasswordMinLength);
		}
        let password_hash = Self::hash_password(new_password.to_string()).await?;

        let result = sqlx::query(
            "UPDATE users
            SET
				password_hash = ?,
				token_version = token_version + 1,
				updated_at = CURRENT_TIMESTAMP
            WHERE user_id = ?",
        )
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }

        Ok(())
    }

    pub async fn verify_token_version(
        pool: &Pool<Sqlite>,
        user_id: &str,
        token_version: i64,
    ) -> Result<(), AuthError> {
        let db_version: Option<i64> = sqlx::query_scalar(
            "SELECT token_version
			FROM users
			WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        match db_version {
            Some(current) if current == token_version => Ok(()),
            Some(_) => Err(AuthError::Unauthorized),
            None => Err(AuthError::UserNotFound),
        }
    }

    /// Verification of Credentials and return a jwt string
    pub async fn login(
        pool: &Pool<Sqlite>,
        username: &str,
        password: &str,
        jwt_secret: &str,
    ) -> Result<String, AuthError> {
        let row = sqlx::query(
			"SELECT
				user_id,
				password_hash,
				role,
				token_version
			FROM users
			WHERE username = ?"
		)
		.bind(username)
		.fetch_optional(pool)
		.await?;

        let row = match row {
            Some(r) => r,
            None => return Err(AuthError::UserNotFound),
        };

        let user_id: String = row.get("user_id");
        let role_str: String = row.get("role");
        let role = Role::try_from(role_str.as_str())
            .map_err(|_| AuthError::InvalidRole)?;
        let stored_hash: String = row.get("password_hash");
        let token_version: i64 = row.get("token_version");

        if !Self::verify_password(password.to_string(), stored_hash.to_string()).await? {
            return Err(AuthError::InvalidCredentials);
        }

        let token = Self::create_token(username, user_id, role, token_version, jwt_secret.as_bytes())?;

        info!("User {:?} logged in successfully", username);
        Ok(token)
    }

	pub async fn logout(pool: &Pool<Sqlite>, user_id: &str) -> Result<(), AuthError> {
		let result = sqlx::query(
				"UPDATE users
				SET token_version = token_version + 1
				WHERE user_id = ?",
			)
			.bind(user_id)
			.execute(pool)
			.await?;

        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }

        Ok(())
	}

    /// Create initial admin (only allow if table is empty)
    pub async fn init_admin(
        pool: &Pool<Sqlite>,
        username: &str,
        password: &str,
    ) -> Result<User, AuthError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(pool)
            .await?;

        if count.0 > 0 {
            warn!("Database is initialized, Users already exists!!");
            return Err(AuthError::AdminAlreadyRegistered);
        }

        let ulid = Ulid::new().to_string();
        let password_hash = Self::hash_password(password.to_string()).await?;
        let role = Role::Admin;

        let result = sqlx::query(
            "INSERT INTO users (
                user_id,
                username,
                password_hash,
                role
            ) VALUES (?, ?, ?, ?)"
        )
        .bind(&ulid)
        .bind(username)
        .bind(password_hash)
        .bind(role.to_string())
        .execute(pool)
        .await;


        match result.map_err(AuthError::from) {
            Ok(_) => {
                info!("Initial admin created: {:?}", username);
                Ok(User {
                    role,
                    user_id: ulid,
                    username: username.to_string(),
                })
            }
            Err(AuthError::DbError(sqlx::Error::Database(db_err)))
            if db_err.message().contains("UNIQUE") => {
                Err(AuthError::UserAlreadyExists)
            }
            Err(e) => Err(e),
        }
    }

    /// Create standard user, can be only invoked by admin
    pub async fn create_user(
        pool: &Pool<Sqlite>,
        requester_username: &str,
        username: &str,
        password: &str,
    ) -> Result<User, AuthError> {
        let requester_role = Self::get_user_role(pool, requester_username).await?;

        if requester_role != Role::Admin {
            return Err(AuthError::Unauthorized);
        }

        let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = ?")
            .bind(username)
            .fetch_one(pool)
            .await?;

        if exists.0 > 0 {
            return Err(AuthError::UserAlreadyExists);
        }

        let ulid = Ulid::new().to_string();
        let password_hash = Self::hash_password(password.to_string()).await?;
        /*
          * Only create user with role user,
          * Let the app can have only one admin
        */
        let role = Role::User;

        let result = sqlx::query(
            "INSERT INTO users (
                user_id,
                username,
                password_hash,
                role
            ) VALUES (?, ?, ?, ?)"
        )
        .bind(&ulid)
        .bind(username)
        .bind(password_hash)
        .bind(role.to_string())
        .execute(pool)
        .await;

        match result.map_err(AuthError::from) {
            Ok(_) => {
                info!("User created: {:?}", username.to_string());
                Ok(User {
                    role,
                    user_id: ulid,
                    username: username.to_string(),
                })
            }
            Err(AuthError::DbError(sqlx::Error::Database(db_err)))
            if db_err.message().contains("UNIQUE") => {
                Err(AuthError::UserAlreadyExists)
            }
            Err(e) => Err(e),
        }
    }

	/// Change password via old password, for both user and admin
	pub async fn change_password(
		pool: &Pool<Sqlite>,
		username: &str,
		current_password: &str,
		new_password: &str,
	) -> Result<(), AuthError> {
		let row = sqlx::query("
				SELECT user_id, password_hash
				FROM users
				WHERE username = ?"
			)
			.bind(username)
			.fetch_optional(pool)
			.await?;

		let row = match row {
			Some(r) => r,
			None => return Err(AuthError::UserNotFound),
		};

		let user_id: String = row.get("user_id");
		let stored_hash: String = row.get("password_hash");

		if !Self::verify_password(current_password.to_string(), stored_hash.to_string()).await? {
			return Err(AuthError::InvalidCredentials);
		}

		Self::update_password(pool, &user_id, new_password).await
	}

    /// Called by admin to change any user's password
    pub async fn admin_change_user_password(
        pool: &Pool<Sqlite>,
        requester_username: &str,
        target_username: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let requester_role = Self::get_user_role(pool, requester_username).await?;
        if requester_role != Role::Admin {
            return Err(AuthError::Unauthorized);
        }

        let target_role = Self::get_user_role(pool, target_username).await?;
        if target_role != Role::User {
            return Err(AuthError::InvalidRole);
        }

        let row = sqlx::query("SELECT user_id FROM users WHERE username = ?")
            .bind(target_username)
            .fetch_optional(pool)
            .await?;

        let row = match row {
            Some(r) => r,
            None => return Err(AuthError::UserNotFound),
        };

        let user_id: String = row.get("user_id");
        Self::update_password(pool, &user_id, new_password).await
    }

    /// Generate a reset token file for the admin account by taking username
    pub async fn gen_admin_pass_reset_token(
        pool: &Pool<Sqlite>,
		data_dir: &Path,
		username: &str
	) -> Result<(), AuthError> {
		const ADMIN_RESET_TOKEN_MAX_PER_DAY: u32 = 5;
		const ADMIN_RESET_TOKEN_TTL_SECS: u64 = 20 * 60;

        let requester_role = Self::get_user_role(pool, username).await?;
        if requester_role != Role::Admin {
            return Err(AuthError::InvalidRole);
        }

        let now = Self::now_unix_secs()?;
        let current_day = now / 86_400;				// 24 hours window
        let reset_dir = data_dir.join("auth");
		let token_filename = format!("{}-password-reset-token.txt", username);
        let token_path = reset_dir.join(token_filename);
        let rate_path = reset_dir.join("rate.bin");

        fs::create_dir_all(&reset_dir).map_err(|_| AuthError::ResetTokenFileError)?;

		let mut rate_state = if let Ok(binary_content) = fs::read(&rate_path) {
            postcard::from_bytes::<AdminPassResetGenState>(&binary_content)
                .map_err(|_| AuthError::ResetTokenFileError)?
        } else {
			AdminPassResetGenState {
                day_bucket: current_day,
                count: 0,
            }
        };

        if rate_state.day_bucket != current_day {
            rate_state.day_bucket = current_day;
            rate_state.count = 0;
        }

        if rate_state.count >= ADMIN_RESET_TOKEN_MAX_PER_DAY {
            return Err(AuthError::ResetTokenRateLimited);
        }

        rate_state.count += 1;

        let expires_at = now + ADMIN_RESET_TOKEN_TTL_SECS;
        let secure_token = Self::create_signed_token(username, expires_at)?;

        let rate_binary_content = postcard::to_allocvec(&rate_state)
            .map_err(|_| AuthError::ResetTokenFileError)?;

        fs::write(&token_path, secure_token).map_err(|_| AuthError::ResetTokenFileError)?;
		// The rate limit should be related with `username`
		// Keep it, as it is, since the app will have only one admin
        fs::write(&rate_path, rate_binary_content).map_err(|_| AuthError::ResetTokenFileError)?;

        info!("Username: {:?} password reset token written to {:?}", username, token_path);

        Ok(())
    }

    /// Reset the admin password using the token stored in a local file.
    pub async fn reset_admin_password(
        pool: &Pool<Sqlite>,
        data_dir: &Path,
        reset_token_input: &str,
		username: &str,
        new_password: &str,
    ) -> Result<User, AuthError> {
		let provided_token = reset_token_input.trim();
		if provided_token.is_empty(){
            return Err(AuthError::InvalidResetToken);
		}

		let (trusted_username, expires_at) = Self::extract_verify_token(provided_token)?;
	    if username != trusted_username {
            return Err(AuthError::InvalidResetToken);
        }

        let user_role = Self::get_user_role(pool, username).await?;
        if user_role != Role::Admin {
            return Err(AuthError::InvalidRole);
        }

        let now = Self::now_unix_secs()?;
        let reset_dir = data_dir.join("auth");
		let token_filename = format!("{}-password-reset-token.txt", username);
        let token_path = reset_dir.join(token_filename);

        if now > expires_at {
            let _ = fs::remove_file(&token_path);
            return Err(AuthError::ResetTokenExpired);
        }

        let stored_content = fs::read_to_string(&token_path)
            .map_err(|_| AuthError::ResetTokenFileError)?;

        if provided_token != stored_content {
            return Err(AuthError::InvalidResetToken);
        }

		let row = sqlx::query(
            "SELECT user_id, role
			FROM users
			WHERE username = ?
			LIMIT 1"
        )
        .bind(&username)
        .fetch_optional(pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => return Err(AuthError::AdminNotFound),
        };

        let user_id: String = row.get("user_id");
        let role_str: String = row.get("role");

        Self::update_password(pool, &user_id, new_password).await?;

        let _ = fs::remove_file(&token_path);

        info!("Admin password reset successfully for {:?}", username);
        let role = Role::try_from(role_str.as_str())
            .map_err(|_| AuthError::InvalidRole)?;

        Ok(User {
            role,
            user_id,
            username: username.to_string(),
        })
    }

    /// Called by admin to delete users
    pub async fn delete_user(
        pool: &Pool<Sqlite>,
        requester_username: &str,
        target_username: &str,
    ) -> Result<(), AuthError> {
        let requester_role = Self::get_user_role(pool, requester_username).await?;
        if requester_role != Role::Admin {
            return Err(AuthError::Unauthorized);
        }
        /*
          * Prevent admin from deleting itself
          * Don't delete user if a single admin exists
        */
        if requester_username == target_username {
            return Err(AuthError::CannotDeleteSelf);
        }

        let admin_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(pool)
            .await?;

        let target_is_admin: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE username = ? AND role = 'admin'"
        )
        .bind(target_username)
        .fetch_optional(pool)
        .await?;

        if let Some((count,)) = target_is_admin {
            if count > 0 && admin_count.0 <= 1 {
                return Err(AuthError::CannotDeleteLastAdmin);
            }
        }

		let row = sqlx::query("SELECT user_id, password_hash FROM users WHERE username = ?")
			.bind(target_username)
			.fetch_optional(pool)
			.await?;

		match row {
			Some(r) => r,
			None => return Err(AuthError::UserNotFound),
		};

        sqlx::query("DELETE FROM users WHERE username = ?")
            .bind(target_username)
            .execute(pool)
            .await?;

        Ok(())
    }

	fn create_signed_token(username: &str, expires_at: u64) -> Result<String, AuthError> {
		let payload = format!("{}:{}", username, expires_at);
		let secret_key = key::get_server_secret();

		let mut mac = HmacSha256::new_from_slice(secret_key)
			.map_err(|_| AuthError::TokenCreationError)?;

		mac.update(payload.as_bytes());
		let signature_bytes = mac.finalize().into_bytes();

		let signature_str = URL_SAFE_NO_PAD.encode(signature_bytes);
		Ok(format!("{}:{}", payload, signature_str))
	}

	/// Parses the token, recalculates signature, extracts the username
	fn extract_verify_token(token: &str) -> Result<(String, u64), AuthError> {
		let parts: Vec<&str> = token.split(':').collect();
		if parts.len() != 3 {
			return Err(AuthError::InvalidResetToken);
		}
		let username = parts[0];
		let expires_at_str = parts[1];
		let provided_signature = parts[2];

		let payload = format!("{}:{}", username, expires_at_str);
		let secret_key = key::get_server_secret();

		let mut mac = HmacSha256::new_from_slice(secret_key)
			.map_err(|_| AuthError::TokenCreationError)?;

		mac.update(payload.as_bytes());

		let decoded_sig_bytes = URL_SAFE_NO_PAD.decode(provided_signature)
			.map_err(|_| AuthError::InvalidResetToken)?;

		if mac.verify_slice(&decoded_sig_bytes).is_err() {
			return Err(AuthError::InvalidResetToken);
		}

		let expires_at = expires_at_str.parse::<u64>()
			.map_err(|_| AuthError::InvalidResetToken)?;

		Ok((username.to_string(), expires_at))
	}

	/// Just get the role
    async fn get_user_role(
        pool: &Pool<Sqlite>,
        username: &str,
    ) -> Result<Role, AuthError> {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM users WHERE username = ?"
        )
            .bind(username)
            .fetch_optional(pool)
        .await?;

        match role {
            Some(r) => Role::try_from(r.as_str()).map_err(|_| AuthError::InvalidRole),
            None => Err(AuthError::UserNotFound),
        }
    }

    /*
      * Check if an Admin account exists.
    */
    pub async fn admin_exists(pool: &Pool<Sqlite>) -> Result<bool, sqlx::Error> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(pool)
            .await?;

        Ok(count.0 != 0)
    }
}
