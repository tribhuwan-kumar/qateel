use tracing::debug;
use serde_json::json;
use axum_extra::extract::CookieJar;
use axum::{
    Json,
    middleware::Next,
    response::Response,
    http::{StatusCode, Method, request::Parts},
    extract::{State, Request,
        FromRef, FromRequestParts,
	},
};
use jsonwebtoken::{decode, Validation, DecodingKey};

use crate::{
    AppState,
    auth::hmac,
    auth::types::{
        Claims,
		AuthController,
        User as AuthenticatedUser,
    },
};

/// HMAC based
pub async fn csrf_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let method = req.method();
    // keep read only method without `csrf_token`
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return Ok(next.run(req).await);
    }
    let header_token = req.headers()
        .get("x-csrf-token")
        .and_then(|h| h.to_str().ok())
        .ok_or((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Missing CSRF token" }))
        ))?;

    let user = req.extensions().get::<AuthenticatedUser>()
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Not authenticated" }))
        ))?;
    debug!("authenticated user: {:?}", user);

    let expected_token = hmac::generate_bound_csrf(&user.user_id, &state.jwt_secret);
    if header_token != expected_token {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Invalid CSRF token for this user" }))
        ));
    }

    Ok(next.run(req).await)
}

/// Just check the cookie, gaurd ☕︎
pub async fn auth_guard(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let tok = jar.get("auth_token")
        .map(|c| c.value())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Missing authentication cookie" }))
        ))?;

    let val = Validation::default();
    let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());

    match decode::<Claims>(tok, &key, &val) {
        Ok(t) => {
            if AuthController::verify_token_version(
                &state.db,
                &t.claims.user_id,
                t.claims.token_version,
            )
            .await
            .is_err()
            {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Invalid or expired token" }))
                ));
            }

            let user = AuthenticatedUser {
                user_id: t.claims.user_id,
                username: t.claims.sub,
                role: t.claims.role,
            };
            req.extensions_mut().insert(user);
            Ok(next.run(req).await)
        },
        Err(_) => {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid or expired token" }))
            ))
        }
    }
}


/// Extract the authenticated infos
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let jar = CookieJar::from_request_parts(parts, &state).await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "Cookie error"})))
        })?;

        let token = jar.get("auth_token")
            .map(|c| c.value())
            .ok_or((StatusCode::UNAUTHORIZED, Json(json!({ "error": "Missing auth token"}))))?;

        let validation = Validation::default();
        let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());

        let token_data = decode::<Claims>(token, &key, &validation)
            .map_err(|_| (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Invalid auth token"}))))?;

        AuthController::verify_token_version(
            &state.db,
            &token_data.claims.user_id,
            token_data.claims.token_version,
        )
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Invalid auth token"}))))?;

        Ok(AuthenticatedUser {
            user_id: token_data.claims.user_id,
            role: token_data.claims.role,
            username: token_data.claims.sub,
        })
    }
}
