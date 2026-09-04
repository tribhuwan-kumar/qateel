use serde_json::json;
use axum::{
	http::StatusCode,
	response::IntoResponse,
    extract::{Json, State},
};
use axum_extra::extract::cookie::{
    Cookie,
    SameSite,
    CookieJar
};
use jsonwebtoken::{
    decode,
    Validation,
    DecodingKey
};

use crate::{
    app::AppState,
    auth::{
        hmac,
        types::{
            Role,
            Claims,
            LoginReq,
            AuthError,
			GenTokenReq,
            RegAdminReq,
            CreateUserReq,
            AuthController,
            DeleteUserReq,
            ChangePasswordReq,
            ResetAdminPasswordReq,
			AdminChangePasswordReq,
			User as AuthenticatedUser,
        },
		auth::MIN_PASSWORD_LENGTH,
        auth::COOKIE_VAILDITY_DURATION,
    },
};

pub const MIN_USERNAME_LENGTH: usize = 6;

pub fn validate_creds(cred: &str, len: usize) -> bool {
    cred.len() >= len && cred.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Make an initial admin
pub async fn reg_admin(
    State(state): State<AppState>,
    Json(payload): Json<RegAdminReq>,
) -> impl IntoResponse {
    let username = payload.username.clone();
    if !validate_creds(&username, MIN_USERNAME_LENGTH) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Username must be longer than 6 characters and contain only letters, numbers, and underscores"
            })),
        );
    }
    if payload.password.clone().len() <= MIN_PASSWORD_LENGTH {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Password must be longer than 8 characters"
            })),
        );
    }

    match AuthController::init_admin(&state.db, &payload.username, &payload.password).await {
        Ok(user) => {
            state.status_tx.send_modify(|status| {
                status.admin_exists = true;
            });
            (StatusCode::OK, Json(json!({"status": "registered", "username": user.username })))
        },
        Err(AuthError::AdminAlreadyRegistered) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Admin already registered" }))
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{:?}", e) }))
        ),
    }
}


/// Create user
pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<CreateUserReq>,
) -> impl IntoResponse {
    let username = payload.username.clone();
    if !validate_creds(&username, MIN_USERNAME_LENGTH) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Username must be longer than 6 characters and contain only letters, numbers, and underscores"
            })),
        );
    }
    if payload.password.clone().len() <= MIN_PASSWORD_LENGTH {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Password must be longer than 8 characters"
            })),
        );
    }
    if auth.role != Role::Admin {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "User can only be created by admins"})));
    }

    match AuthController::create_user(
        &state.db,
        &payload.requester_username,
        &payload.username,
        &payload.password,
    )
    .await
    {
        Ok(user) => (StatusCode::OK, Json(json!({
                "status": "userCreted",
                "username": user.username,
                "role": user.role
        }))),
        Err(AuthError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Unauthorized" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Change by giving current password
pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<ChangePasswordReq>,
) -> impl IntoResponse {
	if &payload.current_password == &payload.new_password {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "New pasword should be different from current password"
            })),
        );
	}

    match AuthController::change_password(
        &state.db,
        &auth.username,
        &payload.current_password,
        &payload.new_password,
    )
    .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "status": "passwordChanged" })),
        ),
        Err(AuthError::InvalidCredentials) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid current password" })),
        ),
        Err(AuthError::UserNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Admin can change any user's password.
pub async fn admin_change_user_password(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<AdminChangePasswordReq>,
) -> impl IntoResponse {
    if auth.role != Role::Admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Users password can only be changed by admin" })),
        );
    }

    match AuthController::admin_change_user_password(
        &state.db,
        &auth.username,
        &payload.target_username,
        &payload.new_password,
    )
    .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "status": "passwordChanged" })),
        ),
        Err(AuthError::InvalidRole) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Can only change user password" })),
        ),
        Err(AuthError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Unauthorized" })),
        ),
        Err(AuthError::UserNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Generate a short lived admin password reset token file.
pub async fn gen_admin_pass_reset_token(
    State(state): State<AppState>,
    Json(payload): Json<GenTokenReq>,
) -> impl IntoResponse {
    match AuthController::gen_admin_pass_reset_token(
        &state.db,
		&state.config.data_dir.as_path(),
		&payload.username
	).await
	{
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "resetPasswordTokenGenerated",
            })),
        ),
        Err(AuthError::InvalidRole) => (
            StatusCode::FORBIDDEN,
            Json(json!({
				"username": &payload.username,
				"error": "Invalid role for generating reset password token"
			})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Reset the admin password with a token from a file.
pub async fn reset_admin_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetAdminPasswordReq>,
) -> impl IntoResponse {
    match AuthController::reset_admin_password(
        &state.db,
		&state.config.data_dir.as_path(),
        &payload.reset_token,
		&payload.username,
        &payload.new_password,
    )
    .await
    {
        Ok(user) => (
            StatusCode::OK,
            Json(json!({
                "status": "passwordReset",
                "username": user.username
            })),
        ),
        Err(AuthError::InvalidResetToken) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Invalid reset token" })),
        ),
        Err(AuthError::AdminNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Admin not found" })),
        ),
        Err(AuthError::ResetTokenFileError) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Reset token file not found" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// delete delete delete!!
pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(payload): Json<DeleteUserReq>,
) -> impl IntoResponse {
    if auth.role != Role::Admin {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "User can only be deleted by admins"})));
    }
    match AuthController::delete_user(
        &state.db,
        &payload.requester_username,
        &payload.target_username,
    ).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "deleted",
                "targetUsername": &payload.target_username
            })),
        ),
		Err(AuthError::Unauthorized) => (
			StatusCode::UNAUTHORIZED,
			Json(json!({ "error": "Failed to delete the username" }))
		),
		Err(AuthError::CannotDeleteSelf) => (
			StatusCode::FORBIDDEN,
			Json(json!({ "error": "Can't delete itself", "targetUsername": &payload.target_username }))
		),
		Err(AuthError::UserNotFound) => (
			StatusCode::NOT_FOUND,
			Json(json!({ "error": "User not found" })),
		),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{:?}", e) }))
        ),
    }
}


/// Cookie based login
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<LoginReq>,
) -> impl IntoResponse {
    match AuthController::login(
        &state.db,
        &payload.username,
        &payload.password,
        &state.jwt_secret
    ).await {
        Ok(token) => {
            let username = payload.username.clone();
            let username_ref = username.as_str();
            let row = sqlx::query!(
                "SELECT user_id FROM users WHERE username = ?",
                    username_ref
                )
                .fetch_optional(&state.db)
                .await;

            let row = match row {
                Ok(Some(r)) => r,
                Ok(None) => {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(json!({
                            "error": format!("username not found {}", username)
                        })),
                    ));
                }
                Err(e) => {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": format!("Database error: {}", e)
                        })),
                    ));
                }
            };

            let user_id = row.user_id;
            let bound_csrf = hmac::generate_bound_csrf(&user_id, &state.jwt_secret);

            let csrf_cookie = Cookie::build(("csrf_token", bound_csrf))
                .path("/")
                .http_only(false)
                .same_site(SameSite::Strict)
                .build();

			let is_https = match (&state.config.ssl_cert, &state.config.ssl_key) {
				(Some(cert), Some(key)) => cert.exists() && key.exists(),
				_ => false,
			};

            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .secure(is_https)
                .http_only(true)
                .same_site(SameSite::Strict)
                .max_age(time::Duration::days(COOKIE_VAILDITY_DURATION as i64))
                .build();

            Ok((
                StatusCode::OK,
                jar.add(cookie).add(csrf_cookie),
                Json(json!({
                    "status": "loggedIn",
                    "username": username
                })),
            ))
        },
        Err(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Invalid username or password"
            })),
        )),
    }
}

/// Just remove the cookie
pub async fn logout(
    State(state): State<AppState>,
	jar: CookieJar,
    auth: AuthenticatedUser,
) -> impl IntoResponse {
	let mut auth_cookie = Cookie::build(("auth_token", "")).path("/").build();
    auth_cookie.make_removal();
	let mut csrf_cookie = Cookie::build(("csrf_token", "")).path("/").build();
    csrf_cookie.make_removal();

	let updated_jar = jar.add(auth_cookie).add(csrf_cookie);

	match AuthController::logout(&state.db, &auth.user_id).await {
		Ok(_) => {
            (
                StatusCode::OK,
                updated_jar,
                Json(json!({
                    "status": "loggedOut",
                    "username": auth.username
                }))
            ).into_response()
        },
		Err(e) => {
            // If the database operation fails, still drop cookies on the client side!!
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                updated_jar,
                Json(json!({ "error": format!("{:?}", e) }))
            ).into_response()
        }
	}
}

/// Just checking the current logged in
pub async fn get_me(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if let Some(token_cookie) = jar.get("auth_token") {
        let token = token_cookie.value();
        /* Verify manually */
        let validation = Validation::default();
        let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());
        match decode::<Claims>(token, &key, &validation) {
            Ok(token_data) => {
                match AuthController::verify_token_version(
                    &state.db,
                    &token_data.claims.user_id,
                    token_data.claims.token_version,
                )
                .await
                {
                    Ok(()) => {
                        return (
                            StatusCode::OK,
                            Json(json!({
                                "authenticated": true,
                                "role": token_data.claims.role,
                                "username": token_data.claims.sub,
                                "userId": token_data.claims.user_id,
                            }))
                        );
                    }
                    Err(_) => {}
                }
            }
            // token invalid or expired
            Err(_) => {}
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "authenticated": false }))
    )
}

