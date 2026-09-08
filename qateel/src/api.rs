use axum::{
    Router, middleware,
    routing::{get, post, delete},
    http::{header, HeaderName, HeaderValue, Method},
};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{
    app::AppState,
    web, aria2, auth, his, status
};

fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    let origins: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect();

    if origins.is_empty() {
        return None;
    }

    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
				Method::GET,
				Method::PUT,
				Method::POST,
				Method::PATCH,
				Method::DELETE,
				Method::OPTIONS
			])
            .allow_headers([
                header::CONTENT_TYPE,
                header::ACCEPT,
                header::AUTHORIZATION,
                HeaderName::from_static("x-csrf-token"),
            ])
            .allow_credentials(true),
    )
}

/// Api routes
#[allow(deprecated)]
pub fn routes(state: AppState, cors_origins: Vec<String>) -> Router {
    let router = Router::new()
        .nest("/api/auth", Router::new()
            .route("/me", get(auth::handler::get_me))
            .route("/login", post(auth::handler::login))
            .route("/logout", post(auth::handler::logout))
			.route("/reg/admin", post(auth::handler::reg_admin))
			.route("/create/user", post(auth::handler::create_user))
			.route("/delete/user", delete(auth::handler::delete_user))
			.route("/password/change", post(auth::handler::change_password))
			.route("/admin/reset_password", post(auth::handler::reset_admin_password))
			.route("/admin/change_password/user", post(auth::handler::admin_change_user_password))
			.route("/admin/gen_password_reset_token", post(auth::handler::gen_admin_pass_reset_token))
        )

        .nest("/api/qateel", Router::new()
			.route("/probe/uris", post(aria2::proxy::probe_uris_info))
			.route("/probe/torrents", post(aria2::proxy::probe_torrents))
            .route("/add/uris", post(aria2::proxy::add_uris))
            .route("/add/torrents", post(aria2::proxy::add_torrents))
            .route("/user/history", get(aria2::proxy::get_history))
            .route("/user/items/count", get(aria2::proxy::get_item_stats))
            .route("/gid/stop", post(aria2::proxy::stop))
            .route("/gid/retry", post(aria2::proxy::retry))
            .route("/gid/delete", delete(aria2::proxy::delete))
            .route("/gid/pause", post(aria2::proxy::pause))
            .route("/gid/resume", post(aria2::proxy::resume))
            .route("/gid/details", post(aria2::proxy::details))
            .route("/gid/changeuri", post(aria2::proxy::change_uri))
            .route("/gid/get/options", post(aria2::proxy::get_option))
            .route("/gid/move", post(aria2::proxy::move_position))
            .route("/gid/set/options", post(aria2::proxy::set_option))
            .route("/global/get/options", get(aria2::proxy::get_global_option))
            .route("/global/set/options", post(aria2::proxy::set_global_option))
            .route("/cmd/set/options", post(aria2::proxy::set_cmd_option))
            .route("/file/conflict", post(aria2::proxy::check_name_conflict))
            .route("/search", get(aria2::proxy::search))
            .route("/purge", post(aria2::proxy::purge_results))
            .route("/info", get(aria2::proxy::get_aria2_info))
            .layer(middleware::from_fn_with_state(state.clone(), auth::middleware::csrf_guard))
            .layer(middleware::from_fn_with_state(state.clone(), auth::middleware::auth_guard))
        )
        /* Websockets */
        .route("/api/ws/ddl", get(his::ws::ddl_ws))
        .route("/api/ws/status", get(status::status_ws))
        .route("/api/ws/global/stat", get(aria2::ws::global_stat_ws));

	let router = if let Some(layer) = cors_layer(&cors_origins) {
		router.layer(layer)
	} else {
		router
	};

    router
        .fallback(web::static_handler)
        .with_state(state)
}

// 'ᝰ' what is this!?
