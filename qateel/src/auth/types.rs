use std::fmt;
use serde::{Deserialize, Serialize};

pub struct AuthController;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "user")]
    User,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::User => write!(f, "user"),
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "admin" => Ok(Role::Admin),
            "user" => Ok(Role::User),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub role: Role,
    pub user_id: String,  // ulid string
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claims {
    pub exp: usize,
    pub sub: String,
    pub role: Role,
    pub user_id: String,  // ulid string
    pub token_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPassResetGenState {
    pub day_bucket: u64,
    pub count: u32,
}

/// For Auth operations
#[derive(Debug)]
pub enum AuthError {
	TimeError,
    InvalidRole,
    Unauthorized,
    UserNotFound,
	AdminNotFound,
    CannotDeleteSelf,
	InvalidResetToken,
    UserAlreadyExists,
	PasswordMinLength,
	ResetTokenExpired,
    PasswordHashError,
    InvalidCredentials,
    TokenCreationError,
	ResetTokenFileError,
    DbError(sqlx::Error),
    CannotDeleteLastAdmin,
    AdminAlreadyRegistered,
    ResetTokenRateLimited,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::InvalidRole => write!(f, "Invalid role"),
            AuthError::Unauthorized => write!(f, "Unauthorized"),
            AuthError::UserNotFound => write!(f, "User not found"),
			AuthError::TimeError => write!(f, "System time error"),
			AuthError::AdminNotFound => write!(f, "Admin not found"),
			AuthError::DbError(err) => write!(f, "Database error: {err}"),
            AuthError::CannotDeleteSelf => write!(f, "Can't delete self"),
			AuthError::InvalidResetToken => write!(f, "Invalid reset token"),
			AuthError::PasswordHashError => write!(f, "Password hash error"),
			AuthError::ResetTokenExpired => write!(f, "Reset token expired"),
			AuthError::UserAlreadyExists => write!(f, "User already exists"),
            AuthError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthError::TokenCreationError => write!(f, "Token creation error"),
			AuthError::ResetTokenFileError => write!(f, "Reset token file error"),
			AuthError::CannotDeleteLastAdmin => write!(f, "Cannot delete last admin"),
			AuthError::AdminAlreadyRegistered => write!(f, "Admin already registered"),
			AuthError::PasswordMinLength => write!(f, "Password should be atleast 8 characters"),
            AuthError::ResetTokenRateLimited => write!(f, "Reset token generation rate limit exceeded, try after sometimes"),
        }
    }
}

impl std::error::Error for AuthError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AuthError::DbError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for AuthError {
    fn from(err: sqlx::Error) -> Self {
        AuthError::DbError(err)
    }
}

#[derive(Deserialize)]
pub struct RegAdminReq {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserReq {
    pub username: String,
    pub password: String,
    pub requester_username: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserReq {
    pub requester_username: String,
    pub target_username: String,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

/// Change password via old password
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordReq {
    pub current_password: String,
    pub new_password: String,
}

/// Admin can change users' password
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminChangePasswordReq {
    pub target_username: String,
    pub new_password: String,
}

/// If admin forget its password
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetAdminPasswordReq {
	pub username: String,
    pub reset_token: String,
    pub new_password: String,
}

/// Token generation req
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenTokenReq {
    pub username: String,
}

