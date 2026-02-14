use crate::config::ApiAuthMode;
use axum::{
    Router,
    routing::{get, post},
};

use crate::api::{
    ApiState,
    handlers::{
        auth_bootstrap, create_session, debug_lookup_once, debug_probe_peer, downloads, events,
        health, kad_keyword_results, kad_peers, kad_publish_keyword, kad_publish_source,
        kad_search_keyword, kad_search_sources, kad_sources, search_delete, search_details,
        search_stop, searches, session_check, session_logout, settings_get, settings_patch, status,
        token_rotate,
    },
    ui::{root_index_redirect, ui_asset, ui_auth, ui_fallback, ui_index, ui_page},
};

pub(crate) fn build_app(state: ApiState) -> Router<()> {
    let mut v1 = Router::new()
        .route("/health", get(health))
        .route("/token/rotate", post(token_rotate))
        .route("/session", post(create_session))
        .route("/session/check", get(session_check))
        .route("/session/logout", post(session_logout))
        .route("/status", get(status))
        .route("/downloads", get(downloads))
        .route("/events", get(events))
        .route("/settings", get(settings_get).patch(settings_patch))
        .route("/searches", get(searches))
        .route(
            "/searches/:search_id",
            get(search_details).delete(search_delete),
        )
        .route("/searches/:search_id/stop", post(search_stop))
        .route("/kad/peers", get(kad_peers))
        .route("/kad/sources/:file_id_hex", get(kad_sources))
        .route(
            "/kad/keyword_results/:keyword_id_hex",
            get(kad_keyword_results),
        )
        .route("/kad/search_sources", post(kad_search_sources))
        .route("/kad/search_keyword", post(kad_search_keyword))
        .route("/kad/publish_source", post(kad_publish_source))
        .route("/kad/publish_keyword", post(kad_publish_keyword));
    if matches!(state.auth_mode, ApiAuthMode::LocalUi) {
        v1 = v1.route("/auth/bootstrap", get(auth_bootstrap));
    }
    if state.enable_debug_endpoints {
        v1 = v1
            .route(
                "/debug/routing/summary",
                get(crate::api::handlers::debug_routing_summary),
            )
            .route(
                "/debug/routing/buckets",
                get(crate::api::handlers::debug_routing_buckets),
            )
            .route(
                "/debug/routing/nodes",
                get(crate::api::handlers::debug_routing_nodes),
            )
            .route("/debug/lookup_once", post(debug_lookup_once))
            .route("/debug/probe_peer", post(debug_probe_peer));
    }

    Router::new()
        .route("/", get(root_index_redirect))
        .route("/auth", get(ui_auth))
        .route("/index.html", get(ui_index))
        .route("/ui", get(ui_index))
        .route("/ui/", get(ui_index))
        .route("/ui/:page", get(ui_page))
        .route("/ui/assets/*path", get(ui_asset))
        .fallback(get(ui_fallback))
        .nest("/api/v1", v1)
        .with_state(state)
}
